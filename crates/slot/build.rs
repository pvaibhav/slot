use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/logs/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
    println!("cargo:rerun-if-env-changed=SLOT_VERSION");
    if std::path::Path::new("../../.git/packed-refs").exists() {
        println!("cargo:rerun-if-changed=../../.git/packed-refs");
    }

    let hash = git(&["rev-parse", "--short=8", "HEAD"])
        .or_else(|| {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let root = root.canonicalize().ok()?;
            git(&[
                "config",
                "--global",
                "--add",
                "safe.directory",
                root.to_str()?,
            ])?;
            git(&["rev-parse", "--short=8", "HEAD"])
        })
        .unwrap_or_else(|| "unknown".into());
    let version = std::env::var("SLOT_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| git(&["describe", "--tags", "--abbrev=0"]))
        .map(|t| t.trim_start_matches('v').to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
    let date = Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=SLOT_GIT_HASH={hash}");
    println!("cargo:rustc-env=SLOT_VERSION={version}");
    println!("cargo:rustc-env=SLOT_BUILD_DATE={date}");
}
