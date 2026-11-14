//! What windows are doing, for the telemetry borders and the context-aware layout.
//!
//! Signals per window:
//!   busy   a compiler or build tool runs inside it (seen in the process tree)
//!   error  a command failed (reported with `oxide-ctl signal error`, which Oxide's fish setup does for you)
//!   ok     a long command finished well (a short green flash)
//!   call   it records from the microphone (PipeWire), so it is most likely a call
//!   audio  it plays sound
//! A background thread looks at /proc and PipeWire every two seconds; nothing runs on the compositor thread.
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use smithay::reexports::calloop::channel::{channel, Event};

use crate::state::Oxide;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sig {
    Busy,
    Error,
    Ok,
    Call,
    Audio,
}

impl Sig {
    pub fn name(self) -> &'static str {
        match self {
            Sig::Busy => "busy",
            Sig::Error => "error",
            Sig::Ok => "ok",
            Sig::Call => "call",
            Sig::Audio => "audio",
        }
    }
    /// Signals reported by programs win over the ones Oxide guesses.
    fn rank(self) -> u8 {
        match self {
            Sig::Error => 5,
            Sig::Ok => 4,
            Sig::Busy => 3,
            Sig::Call => 2,
            Sig::Audio => 1,
        }
    }
}

const BUILD_TOOLS: &[&str] = &[
    "cargo", "rustc", "make", "gmake", "ninja", "cmake", "gcc", "g++", "cc1", "cc1plus", "clang", "clang++", "ld", "mold", "go", "javac",
    "gradle", "mvn", "npm", "pnpm", "yarn", "tsc", "webpack", "vite", "dotnet", "zig", "meson", "bazel", "swift", "ghc", "stack", "pytest",
];

/// What the thread found: the CPU load and, per window pid, what it is doing.
struct Report {
    cpu: f32,
    sigs: HashMap<u32, Sig>,
}

fn proc_table() -> (HashMap<u32, u32>, HashMap<u32, String>) {
    let mut parent = HashMap::new();
    let mut comm = HashMap::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return (parent, comm) };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
        // "pid (comm) state ppid ..."; comm may contain spaces, so split at the last ')'
        let Some(close) = stat.rfind(')') else { continue };
        let name = stat.get(stat.find('(').map_or(0, |i| i + 1)..close).unwrap_or("").to_string();
        let ppid = stat[close + 1..].split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        parent.insert(pid, ppid);
        comm.insert(pid, name);
    }
    (parent, comm)
}

/// The window pid a process belongs to (itself or the nearest ancestor that is a window).
fn owner(pid: u32, parent: &HashMap<u32, u32>, windows: &HashSet<u32>) -> Option<u32> {
    let mut p = pid;
    for _ in 0..64 {
        if windows.contains(&p) {
            return Some(p);
        }
        p = *parent.get(&p)?;
        if p <= 1 {
            return None;
        }
    }
    None
}

fn cpu_times() -> Option<(u64, u64)> {
    let t = std::fs::read_to_string("/proc/stat").ok()?;
    let v: Vec<u64> = t.lines().next()?.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    let idle = v.get(3)? + v.get(4).unwrap_or(&0);
    Some((v.iter().sum(), idle))
}

/// PipeWire streams that are running: (process id, recording?)
fn audio_streams() -> Vec<(u32, bool)> {
    let Ok(out) = std::process::Command::new("pw-dump").stderr(std::process::Stdio::null()).output() else { return vec![] };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else { return vec![] };
    let mut res = vec![];
    for o in v.as_array().into_iter().flatten() {
        let info = &o["info"];
        if info["state"].as_str() != Some("running") {
            continue;
        }
        let props = &info["props"];
        let class = props["media.class"].as_str().unwrap_or("");
        let rec = class == "Stream/Input/Audio";
        if !rec && class != "Stream/Output/Audio" {
            continue;
        }
        let pid = props["application.process.id"].as_u64().or_else(|| props["application.process.id"].as_str().and_then(|s| s.parse().ok()));
        if let Some(pid) = pid {
            res.push((pid as u32, rec));
        }
    }
    res
}

impl Oxide {
    pub fn telemetry_start(&mut self) {
        let (tx, rx) = channel::<Report>();
        let wanted = self.telemetry_pids.clone();
        std::thread::spawn(move || {
            let mut last = cpu_times();
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let windows: HashSet<u32> = wanted.lock().map(|w| w.clone()).unwrap_or_default();
                let now = cpu_times();
                let cpu = match (last, now) {
                    (Some((t0, i0)), Some((t1, i1))) if t1 > t0 => 1.0 - (i1.saturating_sub(i0)) as f32 / (t1 - t0) as f32,
                    _ => 0.0,
                };
                last = now;
                let mut sigs: HashMap<u32, Sig> = HashMap::new();
                if !windows.is_empty() {
                    let (parent, comm) = proc_table();
                    for (pid, name) in &comm {
                        if BUILD_TOOLS.contains(&name.as_str()) {
                            if let Some(w) = owner(*pid, &parent, &windows) {
                                sigs.insert(w, Sig::Busy);
                            }
                        }
                    }
                    for (pid, rec) in audio_streams() {
                        if let Some(w) = owner(pid, &parent, &windows) {
                            let s = if rec { Sig::Call } else { Sig::Audio };
                            let keep = sigs.get(&w).is_some_and(|o| o.rank() >= s.rank());
                            if !keep {
                                sigs.insert(w, s);
                            }
                        }
                    }
                }
                if tx.send(Report { cpu: cpu.clamp(0.0, 1.0), sigs }).is_err() {
                    break;
                }
            }
        });
        let _ = self.loop_handle.insert_source(rx, |ev, _, st| {
            if let Event::Msg(r) = ev {
                st.apply_report(r);
            }
        });
    }

    /// The process id of the client behind window `i`.
    pub fn window_pid(&self, i: usize) -> Option<u32> {
        let w = &self.wins.get(i)?.window;
        if let Some(x) = w.x11_surface() {
            return x.pid();
        }
        let surface = w.toplevel()?.wl_surface().clone();
        let client = self.display_handle.get_client(smithay::reexports::wayland_server::Resource::id(&surface)).ok()?;
        client.get_credentials(&self.display_handle).ok().map(|c| c.pid as u32)
    }

    fn apply_report(&mut self, r: Report) {
        self.cpu_load = r.cpu;
        let mut changed = false;
        for i in 0..self.wins.len() {
            let Some(pid) = self.window_pid(i) else { continue };
            let found = r.sigs.get(&pid).copied();
            let cur = self.wins[i].signal.map(|s| s.0);
            let next = match (cur, found) {
                // reported states (error, ok) stay until they expire or are cleared
                (Some(Sig::Error), _) => cur,
                (Some(Sig::Ok), f) if self.wins[i].signal.is_some_and(|s| s.1.elapsed() < Duration::from_secs(2)) => f.filter(|s| *s == Sig::Busy).or(cur),
                (_, f) => f,
            };
            if next != cur {
                self.wins[i].signal = next.map(|s| (s, Instant::now()));
                changed = true;
            }
        }
        // an error that you have looked at for a while is done
        for w in self.wins.iter_mut() {
            if let Some((Sig::Error, since)) = w.signal {
                if self.focus.as_ref() == Some(&w.window) && since.elapsed() > Duration::from_secs(6) {
                    w.signal = None;
                    changed = true;
                }
            }
        }
        // keep the pid list of the thread current
        let pids: HashSet<u32> = (0..self.wins.len()).filter_map(|i| self.window_pid(i)).collect();
        if let Ok(mut w) = self.telemetry_pids.lock() {
            *w = pids;
        }
        if changed {
            if self.config.layout.context_aware {
                self.arrange();
            }
            self.dirty = true;
        }
        if self.config.decoration.telemetry {
            self.dirty = true; // the focus border follows the CPU load
        }
    }

    /// `oxide-ctl signal STATE --pid PID`: a program reports its state.
    pub fn signal_from(&mut self, pid: Option<u32>, state: &str) -> Result<(), String> {
        let sig = match state {
            "busy" => Some(Sig::Busy),
            "error" | "fail" | "failed" => Some(Sig::Error),
            "ok" | "done" | "success" => Some(Sig::Ok),
            "clear" | "none" => None,
            other => return Err(format!("unknown state {other:?} (busy, error, ok, clear)")),
        };
        // the window the process belongs to (or the focused window when no pid is given)
        let idx = match pid {
            Some(p) => {
                let (parent, _) = proc_table();
                let windows: HashMap<u32, usize> = (0..self.wins.len()).filter_map(|i| self.window_pid(i).map(|w| (w, i))).collect();
                let set: HashSet<u32> = windows.keys().copied().collect();
                owner(p, &parent, &set).and_then(|w| windows.get(&w).copied())
            }
            None => self.focus.as_ref().and_then(|f| self.win_idx(f)),
        };
        let Some(i) = idx else { return Err("no window found for that process".into()) };
        self.wins[i].signal = sig.map(|s| (s, Instant::now()));
        if self.config.layout.context_aware {
            self.arrange();
        }
        self.dirty = true;
        self.kick_animations();
        Ok(())
    }

    /// For the layout: the tiled window of `ws` that needs attention (an error first, then a call).
    pub fn hot_window(&self, members: &[usize]) -> Option<usize> {
        if !self.config.layout.context_aware {
            return None;
        }
        let pick = |want: Sig| members.iter().position(|&i| self.wins[i].signal.is_some_and(|s| s.0 == want));
        pick(Sig::Error).or_else(|| pick(Sig::Call))
    }

    /// Whether any border is animating (so frames keep coming only while needed).
    pub fn telemetry_animating(&self) -> bool {
        self.config.decoration.telemetry
            && self.wins.iter().any(|w| w.visible && match w.signal {
                Some((Sig::Error | Sig::Busy | Sig::Call, _)) => true,
                Some((Sig::Ok, t)) => t.elapsed() < Duration::from_millis(1800),
                _ => false,
            })
    }
}

pub type PidSet = Arc<Mutex<HashSet<u32>>>;
