//! Unix-socket IPC server (see oxide-ipc for the protocol).
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use oxide_ipc::{Request, Response};
use smithay::reexports::calloop::{generic::Generic, timer::{TimeoutAction, Timer}, Interest, LoopHandle, Mode, PostAction};
use tracing::{info, warn};

use crate::state::Oxide;

/// inode of the socket file this process created (0 = none)
static OUR_INO: AtomicU64 = AtomicU64::new(0);

pub fn init(handle: &LoopHandle<'static, Oxide>) {
    listen(handle);
    // if the socket file is deleted or replaced by a dead one (a cleanup script, a second Oxide that exited),
    // bind it again; a file that another running Oxide listens on is left alone
    let h = handle.clone();
    let _ = handle.insert_source(Timer::from_duration(Duration::from_secs(3)), move |_, _, _| {
        let path = oxide_ipc::socket_path();
        let cur = std::fs::metadata(&path).map(|m| m.ino()).unwrap_or(0);
        let ours = OUR_INO.load(Ordering::Relaxed);
        if ours != 0 && cur != ours {
            warn!("ipc socket was replaced or removed, listening again");
            listen(&h);
        }
        TimeoutAction::ToDuration(Duration::from_secs(3))
    });
}

fn listen(handle: &LoopHandle<'static, Oxide>) {
    let path = oxide_ipc::socket_path();
    // another Oxide (for example a nested test window) is already answering here: do not take its socket
    if path.exists() && UnixStream::connect(&path).is_ok() {
        OUR_INO.store(0, Ordering::Relaxed);
        return warn!("another Oxide is listening on {}; ipc is off for this instance", path.display());
    }
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => return warn!("ipc disabled, cannot bind {}: {e}", path.display()),
    };
    OUR_INO.store(std::fs::metadata(&path).map(|m| m.ino()).unwrap_or(0), Ordering::Relaxed);
    listener.set_nonblocking(true).ok();
    info!("ipc listening on {}", path.display());
    let res = handle.insert_source(Generic::new(listener, Interest::READ, Mode::Level), |_, listener, state| {
        while let Ok((stream, _)) = listener.accept() {
            stream.set_nonblocking(false).ok();
            stream.set_read_timeout(Some(Duration::from_millis(200))).ok();
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            let resp = match serde_json::from_str::<Request>(&line) {
                Ok(Request::Subscribe) => {
                    // keep the stream: it receives a `state` line now and on every change
                    let resp = Response::State { state: state.state_info() };
                    let mut out = serde_json::to_string(&resp).unwrap_or_default();
                    out.push('\n');
                    if stream.set_nonblocking(true).is_ok() && (&stream).write_all(out.as_bytes()).is_ok() {
                        state.ipc_subs.push(stream);
                    }
                    continue;
                }
                Ok(req) => state.handle_ipc(req),
                Err(e) => Response::Error { message: format!("bad request: {e}") },
            };
            let mut out = serde_json::to_string(&resp).unwrap_or_default();
            out.push('\n');
            let _ = (&stream).write_all(out.as_bytes());
        }
        Ok(PostAction::Continue)
    });
    if let Err(e) = res {
        warn!("ipc source: {e}");
    }
}

impl Oxide {
    fn handle_ipc(&mut self, req: Request) -> Response {
        match req {
            Request::GetConfig => Response::Config { config: Box::new(self.config.clone()) },
            Request::SetConfig { config } => {
                self.apply_config(*config);
                Response::Ok
            }
            Request::SetRule { app_id, rule, clear } => {
                let mut cfg = self.config.clone();
                let i = cfg.rules.iter().position(|r| r.title.is_none() && r.app_id.as_ref().is_some_and(|a| a.eq_ignore_ascii_case(&app_id)));
                let i = i.unwrap_or_else(|| {
                    cfg.rules.push(oxide_config::WindowRule { app_id: Some(app_id.clone()), ..Default::default() });
                    cfg.rules.len() - 1
                });
                let r = &mut cfg.rules[i];
                for f in &clear {
                    match f.as_str() {
                        "floating" => r.floating = None,
                        "workspace" => r.workspace = None,
                        "opacity" => r.opacity = None,
                        "blur" => r.blur = None,
                        _ => {}
                    }
                }
                r.floating = rule.floating.or(r.floating);
                r.workspace = rule.workspace.or(r.workspace);
                r.opacity = rule.opacity.or(r.opacity);
                r.blur = rule.blur.or(r.blur);
                if r.floating.is_none() && r.workspace.is_none() && r.opacity.is_none() && r.blur.is_none() {
                    cfg.rules.remove(i);
                }
                match cfg.save() {
                    Ok(()) => {
                        self.apply_config(cfg);
                        self.reapply_rules();
                        Response::Ok
                    }
                    Err(e) => Response::Error { message: e.to_string() },
                }
            }
            Request::Signal { pid, state } => match self.signal_from(pid, &state) {
                Ok(()) => Response::Ok,
                Err(message) => Response::Error { message },
            },
            Request::Reload => match self.reload_config_from_disk() {
                Ok(()) => Response::Ok,
                Err(message) => Response::Error { message },
            },
            Request::Dispatch { action } => {
                self.dispatch_action(&action);
                Response::Ok
            }
            Request::GetState => Response::State { state: self.state_info() },
            Request::Subscribe => Response::Ok,
            Request::Input { input } => match self.inject(input) {
                Ok(()) => Response::Ok,
                Err(message) => Response::Error { message },
            },
            Request::CaptureKey => {
                self.capture = Some(None);
                self.capture_deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
                Response::Ok
            }
            Request::Record { dir, frames } => {
                let _ = std::fs::create_dir_all(&dir);
                self.recorder = Some((dir, frames, 0));
                self.dirty = true;
                Response::Ok
            }
            Request::ForceUnlock => {
                use smithay::wayland::session_lock::SessionLockHandler;
                self.unlock();
                Response::Ok
            }
            Request::CancelCapture => {
                self.capture = None;
                Response::Ok
            }
            Request::TakeCapture => {
                match &self.capture {
                    Some(Some(c)) => {
                        let combo = Some(c.clone());
                        self.capture = None;
                        Response::Captured { combo }
                    }
                    Some(None) if std::time::Instant::now() > self.capture_deadline => {
                        self.capture = None;
                        Response::Error { message: "capture timed out".into() }
                    }
                    Some(None) => Response::Captured { combo: None },
                    // not waiting any more (cancelled with Escape)
                    None => Response::Error { message: "no capture".into() },
                }
            }
            Request::RestartShell => {
                self.restart_shell();
                Response::Ok
            }
        }
    }
}

impl Oxide {
    /// Push the current state to every subscriber if it changed since the last push.
    pub fn broadcast_state(&mut self) {
        if self.ipc_subs.is_empty() {
            return;
        }
        let resp = Response::State { state: self.state_info() };
        let mut json = serde_json::to_string(&resp).unwrap_or_default();
        if json == self.last_state_json {
            return;
        }
        self.last_state_json = json.clone();
        json.push('\n');
        // a subscriber that cannot take the line (closed or stuck) is dropped
        self.ipc_subs.retain(|s| (&*s).write_all(json.as_bytes()).is_ok());
    }
}
