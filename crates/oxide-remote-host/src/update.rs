//! Updates for this program: the newest `windows-vX.Y.Z` release on GitHub; one button downloads it and swaps it in
//! (a running exe can be renamed on Windows, so the old one becomes `.old` and is removed at the next start).
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const FEED: &str = "https://github.com/veelvoer/oxide/releases.atom";
const DOWNLOAD: &str = "https://github.com/veelvoer/oxide/releases/download";

#[derive(Default, Clone)]
pub struct Update {
    pub latest: Option<String>,
    pub busy: String,
    pub checked: bool,
}

fn semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim_start_matches('v').split('.').map(|n| n.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn curl() -> Command {
    let c = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = c;
        c.creation_flags(0x0800_0000);
        return c;
    }
    #[allow(unreachable_code)]
    c
}

/// The newest release newer than this program, as a version.
pub fn check(state: Arc<Mutex<Update>>) {
    std::thread::spawn(move || {
        let feed = curl().args(["-sL", "--max-time", "20", FEED]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        let mut best: Option<String> = None;
        for part in feed.split("/releases/tag/").skip(1) {
            let tag: String = part.chars().take_while(|c| !matches!(c, '"' | '<' | '&' | ' ')).collect();
            if let Some(v) = tag.strip_prefix("windows-v") {
                if best.as_ref().is_none_or(|b| semver(v) > semver(b)) {
                    best = Some(v.to_string());
                }
            }
        }
        let mut s = state.lock().unwrap();
        s.checked = true;
        s.latest = best.filter(|v| semver(v) > semver(VERSION));
        if s.latest.is_none() && feed.is_empty() {
            s.busy = "Could not reach GitHub.".into();
        }
    });
}

/// Remove the leftover of the last update.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("old"));
    }
}

fn swap(exe: &PathBuf, new: &PathBuf) -> std::io::Result<()> {
    let old = exe.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(new, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(e);
    }
    Ok(())
}

/// Download version `v`, swap it in and start it; this program quits.
pub fn apply(state: Arc<Mutex<Update>>, v: String) {
    std::thread::spawn(move || {
        let say = |t: &str| state.lock().unwrap().busy = t.to_string();
        let Ok(exe) = std::env::current_exe() else { return };
        let new = exe.with_extension("new");
        say("Downloading the update…");
        let url = format!("{DOWNLOAD}/windows-v{v}/oxide-remote-host-{v}-windows-x86_64.exe");
        let ok = curl().args(["-sfL", "--max-time", "300", "-o"]).arg(&new).arg(&url).status().is_ok_and(|s| s.success());
        if !ok || std::fs::metadata(&new).map_or(true, |m| m.len() < 1_000_000) {
            let _ = std::fs::remove_file(&new);
            say("The download failed.");
            return;
        }
        match swap(&exe, &new) {
            Ok(()) => {
                say("Restarting…");
                let _ = Command::new(&exe).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                std::process::exit(0);
            }
            Err(e) => say(&format!("Could not replace the program: {e}")),
        }
    });
}
