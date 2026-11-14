//! What the phone can make the PC do. Everything goes through ordinary tools (oxide-ctl, systemctl, playerctl,
//! wpctl, wl-clipboard), so the PC behaves exactly as if you did it yourself.
use std::process::{Command, Stdio};

use oxide_ipc::{call, InputCmd, Request, Response};

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let st = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    match st {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("{cmd} failed ({s})")),
        Err(e) => Err(format!("{cmd}: {e}")),
    }
}

fn spawn(cmd: &str, args: &[&str]) -> Result<(), String> {
    Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| ()).map_err(|e| format!("{cmd}: {e}"))
}

fn oxide(req: Request) -> Result<(), String> {
    match call(&req) {
        Ok(Response::Error { message }) => Err(message),
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Oxide is not running here ({e})")),
    }
}

pub fn input(cmd: InputCmd) -> Result<(), String> {
    oxide(Request::Input { input: cmd })
}

/// The geometry of an output (global position and size), to map phone touches onto it.
pub fn output_geometry(name: Option<&str>) -> Option<(f64, f64, f64, f64)> {
    let Ok(Response::State { state }) = call(&Request::GetState) else { return None };
    let o = state.outputs.iter().find(|o| name.is_some_and(|n| n == o.name)).or_else(|| state.outputs.first())?;
    Some((o.x as f64, o.y as f64, o.width as f64, o.height as f64))
}

pub fn action(name: &str, arg: Option<&str>) -> Result<(), String> {
    match name {
        "lock" => run("oxide-ctl", &["lock"]).or_else(|_| run("loginctl", &["lock-session"])),
        "suspend" => run("systemctl", &["suspend"]),
        "reboot" => run("systemctl", &["reboot"]),
        "shutdown" => run("systemctl", &["poweroff"]),
        "logout" => run("oxide-ctl", &["quit"]),
        "media_play_pause" => run("playerctl", &["play-pause"]),
        "media_next" => run("playerctl", &["next"]),
        "media_prev" => run("playerctl", &["previous"]),
        "mute" => run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).or_else(|_| run("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"])),
        "volume" => {
            let v: u32 = arg.and_then(|a| a.parse().ok()).ok_or("volume needs a number")?;
            let pct = format!("{}%", v.min(150));
            run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &pct]).or_else(|_| run("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &pct]))
        }
        "workspace" => {
            let n: u32 = arg.and_then(|a| a.parse().ok()).ok_or("workspace needs a number")?;
            oxide(Request::Dispatch { action: oxide_config::Action::Workspace(n) })
        }
        "run" => {
            let c = arg.filter(|a| !a.trim().is_empty()).ok_or("nothing to run")?;
            oxide(Request::Dispatch { action: oxide_config::Action::Exec(c.to_string()) }).or_else(|_| spawn("sh", &["-c", c]))
        }
        "open_url" => spawn("xdg-open", &[arg.ok_or("no address")?]),
        "notify" => spawn("notify-send", &["-a", "Oxide Remote", "From your phone", arg.unwrap_or("")]),
        "clipboard_set" => {
            use std::io::Write;
            let mut c = Command::new("wl-copy").stdin(Stdio::piped()).spawn().map_err(|e| format!("wl-copy: {e}"))?;
            c.stdin.take().map(|mut s| s.write_all(arg.unwrap_or("").as_bytes()));
            c.wait().map(|_| ()).map_err(|e| e.to_string())
        }
        other => Err(format!("unknown action {other}")),
    }
}

pub fn clipboard() -> String {
    Command::new("wl-paste").args(["-n", "-t", "text"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
}
