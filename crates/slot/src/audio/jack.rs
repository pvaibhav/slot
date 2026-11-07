use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct Jack {
    plugged: AtomicBool,
    changed: AtomicBool,
}

impl Jack {
    pub fn set(&self, plugged: bool) {
        if self.plugged.swap(plugged, Ordering::Relaxed) != plugged {
            self.changed.store(true, Ordering::Relaxed);
        }
    }

    pub fn plugged(&self) -> bool {
        self.plugged.load(Ordering::Relaxed)
    }

    pub fn take_change(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }
}

pub fn follows_jack(is_dsos: Option<&str>) -> bool {
    is_dsos == Some("1")
}

type Seen = Option<(u64, std::time::SystemTime, u64)>;

pub struct RouteWatch {
    paths: Vec<std::path::PathBuf>,
    seen: Option<Vec<Seen>>,
}

impl RouteWatch {
    pub fn new(alsa_config_path: Option<&str>) -> Self {
        let paths = alsa_config_path
            .map(|p| p.split(':').filter(|s| !s.is_empty()).map(Into::into).collect())
            .unwrap_or_default();
        RouteWatch { paths, seen: None }
    }

    pub fn changed(&mut self) -> bool {
        use std::os::unix::fs::MetadataExt;
        let now: Vec<Seen> = self
            .paths
            .iter()
            .map(|p| {
                let m = std::fs::metadata(p).ok()?;
                Some((m.ino(), m.modified().ok()?, m.len()))
            })
            .collect();
        match self.seen.replace(now) {
            Some(before) => self.seen.as_ref() != Some(&before),
            None => false,
        }
    }
}
