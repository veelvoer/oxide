//! Supervises the Quickshell shell (bar, panels, AI assistant) that Oxide starts for the user.
use std::{
    fs::OpenOptions,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use oxide_config::{active_shell_link, shells_dir};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use tracing::{info, warn};

use crate::state::Oxide;

pub struct ShellProc {
    pub child: Child,
    pub started: Instant,
    pub name: String,
    pub last_mem_check: Instant,
}

/// Private memory (anonymous pages) of a process in MB; shared libraries are not counted.
fn private_mb(pid: u32) -> Option<u64> {
    let t = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let kb: u64 = t.lines().find(|l| l.starts_with("RssAnon:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// A shell that grows beyond this is restarted (the whole session should stay far below 1 GB; the normal shell uses ~100 MB).
fn shell_limit_mb() -> u64 {
    std::env::var("OXIDE_SHELL_LIMIT_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(600)
}

impl Oxide {
    pub fn stop_shell(&mut self) {
        if let Some(mut p) = self.shell.take() {
            let _ = p.child.kill();
            let _ = p.child.wait();
        }
    }

    /// (Re)start the configured shell. Safe to call at any time.
    pub fn restart_shell(&mut self) {
        self.stop_shell();
        let mut cfg = self.config.shell.clone();
        // the standard Oxide shell, unless Advanced mode switched to a custom one
        if !self.config.own.shell {
            cfg.active = "oxide".into();
        }
        if !cfg.enabled || cfg.active.is_empty() {
            return;
        }
        oxide_config::ensure_bundled_shell(&cfg.active);
        let dir = shells_dir().join(&cfg.active);
        if !dir.join("shell.qml").is_file() {
            warn!("shell {:?} not found in {} (install it from Settings > Shell)", cfg.active, shells_dir().display());
            return;
        }
        // stable path other tools (key binds, the settings app) can use
        let link = active_shell_link();
        let _ = std::fs::remove_file(&link);
        if let Err(e) = std::os::unix::fs::symlink(&dir, &link) {
            warn!("cannot update {}: {e}", link.display());
        }
        let log = std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
            .map(|d| d.join("oxide"))
            .and_then(|d| {
                std::fs::create_dir_all(&d).ok()?;
                OpenOptions::new().create(true).append(true).open(d.join("shell.log")).ok()
            });
        // Oxide's own QML module (`import Oxide`) is found through the import path
        let mut imports: Vec<std::path::PathBuf> = oxide_config::qml_dirs().into_iter().filter(|d| d.is_dir()).collect();
        if let Some(old) = std::env::var_os("QML_IMPORT_PATH") {
            imports.extend(std::env::split_paths(&old));
        }
        let mut cmd = Command::new("quickshell");
        if let Ok(p) = std::env::join_paths(imports) {
            cmd.env("QML_IMPORT_PATH", p);
        }
        cmd.arg("-p").arg(&link).env("WAYLAND_DISPLAY", &self.socket_name).env("OXIDE_WALLPAPER", "native").stdin(Stdio::null());
        match log {
            Some(f) => {
                cmd.stdout(f.try_clone().map(Stdio::from).unwrap_or_else(|_| Stdio::null())).stderr(Stdio::from(f));
            }
            None => {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }
        match cmd.spawn() {
            Ok(child) => {
                info!("started shell {:?}", cfg.active);
                self.shell = Some(ShellProc { child, started: Instant::now(), name: cfg.active, last_mem_check: Instant::now() });
            }
            Err(e) => warn!("cannot start quickshell: {e}"),
        }
    }

    /// Restart the shell if it died (but give up when it crashes right away, over and over).
    pub fn watch_shell(&mut self) {
        let Some(p) = self.shell.as_mut() else { return };
        if !matches!(p.child.try_wait(), Ok(Some(_))) {
            // memory guard: a shell that leaks is restarted instead of eating the machine
            if p.last_mem_check.elapsed() > Duration::from_secs(30) {
                p.last_mem_check = Instant::now();
                let (pid, limit) = (p.child.id(), shell_limit_mb());
                if private_mb(pid).is_some_and(|mb| mb > limit) {
                    warn!("shell uses more than {limit} MB; restarting it");
                    self.restart_shell();
                }
            }
            return;
        }
        let lived = p.started.elapsed();
        self.shell = None;
        if lived > Duration::from_secs(10) {
            self.shell_crashes = 0;
        } else {
            self.shell_crashes += 1;
        }
        if self.shell_crashes >= 5 {
            warn!("shell keeps crashing; not restarting it (see ~/.cache/oxide/shell.log)");
            return;
        }
        warn!("shell exited; restarting");
        self.restart_shell();
    }

    pub fn start_shell_watchdog(&mut self) {
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(3)), |_, _, state| {
            state.watch_shell();
            TimeoutAction::ToDuration(Duration::from_secs(3))
        });
    }
}
