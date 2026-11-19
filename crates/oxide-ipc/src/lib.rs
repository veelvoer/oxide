//! IPC protocol: newline-delimited JSON over a Unix socket.
use oxide_config::{Action, Config};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Fetch the live config.
    GetConfig,
    /// Replace the live config (not written to disk).
    SetConfig { config: Box<Config> },
    /// Re-read the config file from disk.
    Reload,
    /// Run a compositor action.
    Dispatch { action: Action },
    GetState,
    /// Keep the connection open; a `state` response is pushed on every change.
    Subscribe,
    /// Like `Subscribe`, from the bundled shell: it also receives `{"status":"call","call":"<function>"}` lines, so
    /// key binds open its panels without starting a process (much faster than `quickshell ipc call`).
    SubscribeShell,
    /// Synthesize keyboard or pointer input (used by automation and the AI assistant).
    Input { input: InputCmd },
    /// Stop and start the configured Quickshell shell again.
    RestartShell,
    /// Wait for the next key combination and hold it back from binds and apps (for the shortcut recorder).
    CaptureKey,
    /// Fetch (and clear) the combination captured after `CaptureKey`; `Captured { combo: None }` while waiting.
    TakeCapture,
    /// Stop waiting for a key.
    CancelCapture,
    /// Test aid: save the next `frames` rendered frames of every monitor as raw BGRX files in `dir`.
    Record { dir: String, frames: u32 },
    /// Change what "always" happens to windows of an app (saved to the config file and applied to open windows):
    /// the set fields of `rule` replace the stored ones, the field names in `clear` are removed.
    SetRule { app_id: String, rule: oxide_config::WindowRule, clear: Vec<String> },
    /// Tell Oxide about the state of a program, for the telemetry borders and context-aware layout:
    /// `state` is "busy", "error", "ok" or "clear"; `pid` is a process inside the window (e.g. your shell).
    Signal { pid: Option<u32>, state: String },
    /// The "Oxide was updated" note was read.
    WhatsNewSeen,
    /// Emergency: drop a screen lock whose locker is gone (also `oxide-ctl unlock` from another TTY).
    ForceUnlock,
}

/// Synthetic input, in global logical coordinates for the pointer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputCmd {
    Move { x: f64, y: f64 },
    /// Move the pointer by an offset (a touchpad on a phone).
    MoveBy { dx: f64, dy: f64 },
    /// button: "left", "right" or "middle"
    Button { button: String, pressed: bool },
    Click { x: f64, y: f64, button: String },
    DoubleClick { x: f64, y: f64 },
    Scroll { dx: f64, dy: f64 },
    /// An xkb key name such as "Return", "Escape" or "a"; `mods` e.g. ["ctrl", "shift"].
    Key { name: String, mods: Vec<String> },
    /// Press or release one key (to hold a modifier, e.g. Super for the HUD).
    KeyState { name: String, pressed: bool },
    Type { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Config { config: Box<Config> },
    State { state: State },
    Error { message: String },
    /// Result of `TakeCapture`: (modifiers like "SUPER+SHIFT", key name).
    Captured { combo: Option<(String, String)> },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub active_workspace: u32,
    pub workspaces: Vec<WorkspaceInfo>,
    /// Layout of the active workspace: "dwindle" or "master".
    pub layout: String,
    pub focused: Option<FocusedWindow>,
    pub windows: Vec<WindowInfo>,
    pub outputs: Vec<OutputInfo>,
    /// The config file has an error: Oxide keeps running with the last good settings until it is fixed.
    #[serde(default)]
    pub config_error: Option<ConfigProblem>,
    /// Oxide was updated since the last login: what changed (shown once).
    #[serde(default)]
    pub whats_new: Option<WhatsNew>,
    /// The Super key is being held on its own (the shell shows its ambient HUD while it is).
    #[serde(default)]
    pub super_held: bool,
    /// The performance profile in force: "eco", "normal" or "performance".
    #[serde(default)]
    pub profile: String,
    /// What the user chose (differs from `profile` while auto-eco is on).
    #[serde(default)]
    pub profile_choice: String,
    /// Running on mains power.
    #[serde(default)]
    pub on_ac: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhatsNew {
    pub version: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigProblem {
    pub message: String,
    pub line: Option<usize>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: u32,
    pub windows: u32,
    /// Shown on some monitor.
    pub visible: bool,
    /// The scratchpad workspace.
    pub special: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusedWindow {
    pub app_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub app_id: String,
    pub title: String,
    pub workspace: u32,
    pub floating: bool,
    pub focused: bool,
    pub fullscreen: bool,
    /// telemetry: "", "busy", "error", "ok", "call" or "audio"
    #[serde(default)]
    pub signal: String,
    /// What the config says should always happen to this app's windows (None = nothing).
    pub rule: Option<oxide_config::WindowRule>,
    /// Where the window is drawn right now (x, y, width, height); moves while animating.
    pub geometry: [i32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub refresh: f64,
    /// workspace shown on this output
    #[serde(default)]
    pub workspace: u32,
    /// the active output (new windows and popups go here)
    #[serde(default)]
    pub focused: bool,
}

pub fn socket_path() -> PathBuf {
    // tests run a second compositor next to the real one and must not touch its socket
    if let Some(p) = std::env::var_os("OXIDE_SOCKET") {
        return PathBuf::from(p);
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| "/tmp".into());
    dir.join("oxide.sock")
}

/// Send one request and wait for the reply.
pub fn call(req: &Request) -> std::io::Result<Response> {
    let mut s = UnixStream::connect(socket_path())?;
    let t = Some(std::time::Duration::from_secs(2));
    s.set_read_timeout(t)?;
    s.set_write_timeout(t)?;
    let mut line = serde_json::to_string(req).map_err(std::io::Error::other)?;
    line.push('\n');
    s.write_all(line.as_bytes())?;
    let mut buf = String::new();
    BufReader::new(s).read_line(&mut buf)?;
    serde_json::from_str(&buf).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_roundtrip() {
        let r = Request::Dispatch { action: Action::Workspace(3) };
        let s = serde_json::to_string(&r).unwrap();
        assert!(matches!(serde_json::from_str::<Request>(&s).unwrap(), Request::Dispatch { .. }));
    }
}
