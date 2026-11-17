//! `oxide-ctl phone`: your phone's screen in a window on the PC, with very little delay, also while the phone is
//! locked in your pocket. scrcpy does the streaming (the phone's hardware video encoder, about 35-70 ms); this finds
//! the phone and starts it with the right settings.
//!
//!   oxide-ctl phone          show the phone (again: hide it)
//!   oxide-ctl phone setup    with the phone on USB once: from then on it works over Wi-Fi
//!
//! The phone is found over USB, at the addresses it was last seen on (Wi-Fi debugging on port 5555 after `setup`),
//! and through Android's wireless debugging announcements on the network (mDNS).
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

fn runtime() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

fn state_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"))
        .join("oxide/phone.json")
}

fn has(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

fn notify(title: &str, body: &str) {
    eprintln!("{title}: {body}");
    let _ = Command::new("notify-send").args(["-a", "Oxide", "-i", "phone", title, body]).stdin(Stdio::null()).spawn();
}

/// `adb` with a time limit (a phone that went away makes some commands hang).
fn adb(args: &[&str], secs: u64) -> Option<String> {
    let mut c = Command::new("adb").args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match c.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < Duration::from_secs(secs) => std::thread::sleep(Duration::from_millis(30)),
            _ => {
                let _ = c.kill();
                return None;
            }
        }
    }
    let out = c.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
}

/// Phones adb can talk to now: (serial, on USB).
fn devices() -> Vec<(String, bool)> {
    adb(&["devices"], 5)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let serial = w.next()?;
            (w.next()? == "device").then(|| (serial.to_string(), !serial.contains(':') && !serial.contains("._adb")))
        })
        .collect()
}

/// Addresses the phone was seen on: after `setup`, and by Oxide Remote sessions.
fn known_ips() -> Vec<String> {
    let mut ips: Vec<String> = vec![];
    if let Some(v) = std::fs::read_to_string(state_file()).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        if let Some(ip) = v["ip"].as_str() {
            ips.push(ip.to_string());
        }
    }
    // the phones paired with oxide-remoted, at the address they last connected from
    let remote = oxide_config::Config::path().with_file_name("remote.json");
    if let Some(v) = std::fs::read_to_string(remote).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        for p in v["phones"].as_array().into_iter().flatten() {
            if let Some(ip) = p["ip"].as_str().filter(|s| !s.is_empty()) {
                if !ips.iter().any(|x| x == ip) {
                    ips.push(ip.to_string());
                }
            }
        }
    }
    ips
}

/// Find the phone and connect to it; the serial to use.
fn find() -> Option<String> {
    let now = devices();
    if let Some((s, _)) = now.iter().find(|(_, usb)| *usb).or(now.first()) {
        return Some(s.clone());
    }
    // wireless debugging announces itself on the network
    if let Some(list) = adb(&["mdns", "services"], 4) {
        for l in list.lines().filter(|l| l.contains("_adb-tls-connect")) {
            if let Some(addr) = l.split_whitespace().find(|w| w.contains(':') && w.chars().next().is_some_and(|c| c.is_ascii_digit())) {
                let _ = adb(&["connect", addr], 5);
            }
        }
    }
    // Wi-Fi debugging after `setup` (port 5555), at the addresses the phone was seen on
    for ip in known_ips() {
        let _ = adb(&["connect", &format!("{ip}:5555")], 4);
    }
    devices().first().map(|(s, _)| s.clone())
}

fn pid_file() -> PathBuf {
    runtime().join("oxide-phone.pid")
}

/// Show the phone (or hide it when it is showing).
pub fn show() -> ! {
    if let Some(pid) = std::fs::read_to_string(pid_file()).ok().and_then(|p| p.trim().parse::<i32>().ok()) {
        if std::path::Path::new(&format!("/proc/{pid}")).exists() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
            let _ = std::fs::remove_file(pid_file());
            std::process::exit(0)
        }
    }
    if !has("scrcpy") || !has("adb") {
        notify("Phone screen", "Needs scrcpy and adb: install the scrcpy and android-tools packages (oxide-ctl doctor shows how).");
        std::process::exit(1)
    }
    let Some(serial) = find() else {
        notify(
            "Phone not found",
            "Plug it in once and choose Settings > Phone > Set up phone screen, or switch on \"Show this phone on my PCs\" in the Oxide Remote app.",
        );
        std::process::exit(1)
    };
    let cfg = oxide_config::Config::load().unwrap_or_default().remote;
    let mut args: Vec<String> = vec![
        "-s".into(),
        serial,
        "--window-title=Phone".into(),
        // the phone's own screen stays dark (it can stay in your pocket); closing the window turns it off again
        "--turn-screen-off".into(),
        "--power-off-on-close".into(),
        // low delay: hardware H.264, no extra buffering, a size that is sharp on a monitor
        "--video-codec=h264".into(),
        format!("--max-size={}", cfg.phone_max_size.clamp(480, 4096)),
        format!("--video-bit-rate={}M", cfg.phone_bitrate.clamp(2, 40)),
        "--max-fps=60".into(),
        "--video-buffer=0".into(),
    ];
    if !cfg.phone_audio {
        args.push("--no-audio".into());
    }
    match Command::new("scrcpy").args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(child) => {
            let _ = std::fs::write(pid_file(), child.id().to_string());
            std::process::exit(0)
        }
        Err(e) => {
            notify("Phone screen", &format!("scrcpy did not start: {e}"));
            std::process::exit(1)
        }
    }
}

/// With the phone on USB: allow Wi-Fi from now on (until the phone restarts; the Oxide Remote app keeps wireless
/// debugging on after that, with the permission granted here).
pub fn setup() -> ! {
    let usb: Vec<String> = devices().into_iter().filter(|(_, usb)| *usb).map(|(s, _)| s).collect();
    let Some(serial) = usb.first() else {
        notify("Phone screen setup", "Connect the phone with a USB cable, allow USB debugging on it, then try again.");
        std::process::exit(1)
    };
    // the app may switch wireless debugging on by itself from now on
    let _ = adb(&["-s", serial, "shell", "pm", "grant", "dev.oxide.remote", "android.permission.WRITE_SECURE_SETTINGS"], 8);
    // its Wi-Fi address
    let route = adb(&["-s", serial, "shell", "ip", "route"], 6).unwrap_or_default();
    let ip = route
        .lines()
        .filter(|l| l.contains("wlan"))
        .find_map(|l| l.split_whitespace().skip_while(|w| *w != "src").nth(1).map(str::to_string));
    let _ = adb(&["-s", serial, "tcpip", "5555"], 10);
    if let Some(ip) = &ip {
        let _ = std::fs::create_dir_all(state_file().parent().unwrap_or(&runtime()));
        let _ = std::fs::write(state_file(), serde_json::json!({ "ip": ip }).to_string());
        std::thread::sleep(Duration::from_millis(1500));
        let _ = adb(&["connect", &format!("{ip}:5555")], 6);
    }
    notify(
        "Phone screen ready",
        &match ip {
            Some(ip) => format!("You can unplug the phone. Super+P shows its screen over Wi-Fi ({ip})."),
            None => "Super+P shows its screen. The phone is not on Wi-Fi now, so for the moment only over USB.".into(),
        },
    );
    std::process::exit(0)
}

pub fn cmd(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        Some("setup") => setup(),
        _ => show(),
    }
}
