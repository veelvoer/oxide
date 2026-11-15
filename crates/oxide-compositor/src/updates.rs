//! Updates: now and then `oxide-ctl update auto` looks for a new version and installs it (it builds at the lowest
//! priority). The note about what changed is read at startup, so it shows once, at the first login after an update.
use std::time::Duration;

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::state::Oxide;

fn notes_path() -> std::path::PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"))
        .join("oxide/whats-new.json")
}

impl Oxide {
    pub fn updates_start(&mut self) {
        let notes: Option<serde_json::Value> = std::fs::read_to_string(notes_path()).ok().and_then(|t| serde_json::from_str(&t).ok());
        if let Some(n) = notes.filter(|n| n["shown"] == serde_json::json!(false)) {
            let changes: Vec<String> = n["changes"].as_array().map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect()).unwrap_or_default();
            self.whats_new = Some(oxide_ipc::WhatsNew { version: n["version"].as_str().unwrap_or_default().to_string(), changes });
        }
        if std::env::var_os("OXIDE_TEST").is_some() {
            return;
        }
        // the first look a few minutes after login (the session settles first), then every six hours
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(240)), |_, _, state| {
            if state.config.update.auto {
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("oxide-ctl")
                        .args(["update", "auto"])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                });
            }
            TimeoutAction::ToDuration(Duration::from_secs(6 * 3600))
        });
    }

    pub fn whats_new_seen(&mut self) {
        self.whats_new = None;
        let p = notes_path();
        if let Some(mut v) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
            v["shown"] = serde_json::json!(true);
            let _ = std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        self.broadcast_state();
    }
}
