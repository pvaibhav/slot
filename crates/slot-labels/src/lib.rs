//! Missing cartridge labels, prepared on a caller's worker thread. No ROM data leaves the
//! device: LaunchBox receives only a cleaned filename and requests for public artwork.
mod artwork;
mod source;

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use slot_store::Cart;

#[derive(Debug)]
pub enum Error {
    Network(String),
    Unavailable(String),
    Image(String),
    Storage(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(s) | Self::Unavailable(s) | Self::Image(s) => f.write_str(s),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Storage(e)
    }
}

trait Transport {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, Error>;
}

struct Http(ureq::Agent);
impl Transport for Http {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, Error> {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut url = url.to_owned();
        // Gallery IDs redirect to canonical titles. Follow a bounded number of redirects,
        // validating every destination and sharing one timeout across the whole transfer.
        for _ in 0..3 {
            if !source::allowed_url(&url) {
                return Err(Error::Unavailable("unsupported artwork URL".into()));
            }
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or_else(|| Error::Network("artwork request timed out".into()))?;
            let mut response = self
                .0
                .get(&url)
                .config()
                .timeout_global(Some(remaining))
                .build()
                .call()
                .map_err(|e| match e {
                    ureq::Error::StatusCode(code)
                        if (400..500).contains(&code) && code != 408 && code != 429 =>
                    {
                        Error::Unavailable(e.to_string())
                    }
                    _ => Error::Network(e.to_string()),
                })?;
            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| Error::Unavailable("artwork redirect has no location".into()))?;
                url = if location.starts_with('/') && !location.starts_with("//") {
                    let host = if url.starts_with("https://images.launchbox-app.com/") {
                        "https://images.launchbox-app.com"
                    } else {
                        "https://gamesdb.launchbox-app.com"
                    };
                    format!("{host}{location}")
                } else {
                    location.to_owned()
                };
                continue;
            }
            return response
                .body_mut()
                .with_config()
                .limit(16 * 1024 * 1024)
                .read_to_vec()
                .map_err(|e| Error::Network(e.to_string()));
        }
        Err(Error::Unavailable("too many artwork redirects".into()))
    }
}

pub struct Downloader {
    http: Http,
    // Cache resolved source URLs, not decoded images. A failed image transfer or SD write
    // should retry that image without repeating title searches and gallery downloads.
    sources: HashMap<String, String>,
}

impl Default for Downloader {
    fn default() -> Self {
        Self {
            http: Http(
                ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(15)))
                    .max_redirects(0)
                    .user_agent(concat!(
                        "slot/",
                        env!("CARGO_PKG_VERSION"),
                        " cartridge-labels"
                    ))
                    .build()
                    .into(),
            ),
            sources: HashMap::new(),
        }
    }
}

impl Downloader {
    /// Blocking; call only on a background worker. Existing files are never replaced.
    pub fn prepare(&mut self, root: &Path, cart: &Cart) -> Result<PathBuf, Error> {
        self.prepare_identified(root, cart, None)
    }

    pub fn prepare_identified(
        &mut self,
        root: &Path,
        cart: &Cart,
        canonical_title: Option<&str>,
    ) -> Result<PathBuf, Error> {
        prepare(
            &mut self.http,
            &mut self.sources,
            root,
            cart,
            canonical_title,
        )
    }
}

fn prepare(
    http: &mut impl Transport,
    sources: &mut HashMap<String, String>,
    root: &Path,
    cart: &Cart,
    canonical_title: Option<&str>,
) -> Result<PathBuf, Error> {
    // Use the actual filename, retaining every tag and punctuation mark.
    let filename = cart
        .rom
        .file_name()
        .ok_or_else(|| Error::Unavailable("missing ROM filename".into()))?;
    let target = root
        .join("Labels")
        .join(cart.platform.dir_name())
        .join(Path::new(filename).with_extension("png"));
    if target.try_exists()? {
        return Ok(target);
    }
    let url = match sources.get(&cart.stem) {
        Some(url) => url.clone(),
        None => {
            let url = source::resolve(http, cart, canonical_title)?;
            sources.insert(cart.stem.clone(), url.clone());
            url
        }
    };
    let bytes = http.get(&url)?;
    let png = artwork::prepare(&bytes)?;
    publish(&target, &png)?;
    Ok(target)
}

fn publish(target: &Path, bytes: &[u8]) -> Result<(), Error> {
    let dir = target
        .parent()
        .ok_or_else(|| Error::Unavailable("missing label directory".into()))?;
    std::fs::create_dir_all(dir)?;
    // The complete file appears in one operation. A custom label copied in during the
    // download wins, including on FAT/exFAT where hard-link based installation cannot work.
    let mut tmp = tempfile::Builder::new()
        .prefix(".slot-label-")
        .tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    match tmp.persist_noclobber(target) {
        Ok(_) => {
            if let Ok(dir) = std::fs::File::open(dir) {
                let _ = dir.sync_all();
            }
            Ok(())
        }
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        // Older vendor exFAT drivers reject RENAME_NOREPLACE and hard links. Their
        // ordinary rename is atomic; the single label worker checks again immediately
        // before publishing so an already present custom label is preserved.
        Err(e) if matches!(e.error.raw_os_error(), Some(1 | 22 | 38 | 95)) => {
            publish_legacy(e.file, target)
        }
        Err(e) => Err(Error::Storage(e.error)),
    }
}

fn publish_legacy(tmp: tempfile::NamedTempFile, target: &Path) -> Result<(), Error> {
    if target.try_exists()? {
        return Ok(());
    }
    tmp.persist(target).map_err(|e| Error::Storage(e.error))?;
    if let Some(dir) = target
        .parent()
        .and_then(|dir| std::fs::File::open(dir).ok())
    {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests;
