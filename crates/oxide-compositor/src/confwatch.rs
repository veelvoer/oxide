//! The config file is watched: saving it applies it. A broken file never takes the session down: the last good
//! settings stay active and the shell shows the error (with the line) until the file is fixed.
use std::time::Duration;

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use tracing::warn;

use crate::state::Oxide;

fn mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(oxide_config::Config::path()).and_then(|m| m.modified()).ok()
}

impl Oxide {
    pub fn config_watch_start(&mut self) {
        self.config_mtime = mtime();
        // a config that was already broken at startup: report it (Oxide started with the defaults)
        if let Err(e) = oxide_config::Config::load() {
            self.set_config_error(&e);
        }
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(800)), |_, _, state| {
            let m = mtime();
            if m != state.config_mtime {
                state.config_mtime = m;
                if m.is_some() {
                    let _ = state.reload_config_from_disk();
                }
            }
            TimeoutAction::ToDuration(Duration::from_millis(800))
        });
    }

    fn set_config_error(&mut self, e: &oxide_config::Error) {
        warn!("config not applied: {e}");
        self.config_error = Some(oxide_ipc::ConfigProblem {
            message: e.short(),
            line: e.line(),
            path: oxide_config::Config::path().display().to_string(),
        });
    }

    /// Read oxide.toml again. On success apply it (when it differs); on a parse error keep everything as it is.
    pub fn reload_config_from_disk(&mut self) -> Result<(), String> {
        match oxide_config::Config::load() {
            Ok(c) => {
                self.config_error = None;
                if c != self.config {
                    self.apply_config(c);
                }
                Ok(())
            }
            Err(e) => {
                self.set_config_error(&e);
                Err(e.to_string())
            }
        }
    }
}
