//! `oxide-ctl phone`: your phone's screen in a window on the PC, with very little delay, also while the phone is
//! locked in your pocket. scrcpy does the streaming (the phone's hardware video encoder, about 35-70 ms); this finds
//! the phone and starts it with the right settings.
//!
//!   oxide-ctl phone          show the phone (again: hide it)
//!   oxide-ctl phone setup    with the phone on USB once: from then on it works over Wi-Fi
//!
//! The phone is found over USB, or over Wi-Fi through Android's wireless debugging (Android 11+: encrypted, only
//! for computers the phone trusts, off again on another network), which announces itself on the network (mDNS).
//! The Oxide Remote app switches it on and off ("Show this phone on my PCs"). The old unencrypted `adb tcpip` mode
//! is never used.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

fn runtime() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
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
    // the announcement can take a moment after the app switched wireless debugging on
    if devices().is_empty() {
        std::thread::sleep(Duration::from_millis(1500));
        if let Some(list) = adb(&["mdns", "services"], 4) {
            for l in list.lines().filter(|l| l.contains("_adb-tls-connect")) {
                if let Some(addr) = l.split_whitespace().find(|w| w.contains(':') && w.chars().next().is_some_and(|c| c.is_ascii_digit())) {
                    let _ = adb(&["connect", addr], 5);
                }
            }
        }
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
        // a real (virtual) keyboard: PIN pads and password fields take it; the phone's lock screen too
        "--keyboard=uhid".into(),
        // low delay: hardware H.264, no extra buffering, a size that is sharp on a monitor
        "--video-codec=h264".into(),
        // Constrained Baseline: every H.264 decoder can show it (openh264, which Fedora's scrcpy uses, only that)
        "--video-codec-options=profile:int=1,level:int=4096".into(),
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

/// Ask for the PIN in a small password prompt; None when cancelled or no prompt tool exists.
fn ask_pin() -> Option<String> {
    let tries: [(&str, Vec<&str>); 3] = [
        ("zenity", vec!["--password", "--title=Unlock phone"]),
        ("kdialog", vec!["--password", "PIN or password of the phone", "--title", "Unlock phone"]),
        ("systemd-ask-password", vec!["--timeout=60", "Phone PIN:"]),
    ];
    for (bin, args) in tries {
        if !has(bin) {
            continue;
        }
        let out = Command::new(bin).args(&args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        if out.status.success() {
            let pin = String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']).to_string();
            return (!pin.is_empty()).then_some(pin);
        }
        return None;
    }
    None
}

/// Unlock the phone from the PC. Android hides its PIN pad from screen mirrors (the picture is black there), so this
/// asks for the PIN here and types it on the phone. The PIN is never stored and goes to the phone through standard
/// input, not through a command line.
pub fn unlock() -> ! {
    let Some(serial) = find() else {
        notify("Phone not found", "Connect the phone (USB, or Wi-Fi pairing in Settings > Phone) first.");
        std::process::exit(1)
    };
    let sh = |cmd: &str| {
        let _ = adb(&["-s", &serial, "shell", cmd], 6);
    };
    // wake it and pull the PIN pad up from the bottom edge
    sh("input keyevent KEYCODE_WAKEUP");
    std::thread::sleep(Duration::from_millis(400));
    sh("wm dismiss-keyguard");
    sh("input swipe 540 2300 540 900 220");
    std::thread::sleep(Duration::from_millis(500));
    let Some(pin) = ask_pin() else {
        notify("Phone unlock", "No PIN entered. The phone's PIN pad is blacked out in the mirror: type your PIN on the keyboard.");
        std::process::exit(1)
    };
    let mut child = match Command::new("adb")
        .args(["-s", &serial, "shell", "input text \"$(cat)\" && input keyevent KEYCODE_ENTER"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            notify("Phone unlock", &format!("adb did not start: {e}"));
            std::process::exit(1)
        }
    };
    if let Some(mut i) = child.stdin.take() {
        use std::io::Write;
        let _ = i.write_all(pin.as_bytes());
    }
    let _ = child.wait();
    std::process::exit(0)
}

/// With the phone on USB, once: the Oxide Remote app may switch Android's wireless debugging on and off from then on
/// (its "Show this phone on my PCs" switch). Nothing else on the phone changes.
pub fn setup() -> ! {
    let usb: Vec<String> = devices().into_iter().filter(|(_, usb)| *usb).map(|(s, _)| s).collect();
    let Some(serial) = usb.first() else {
        notify("Phone screen setup", "Connect the phone with a USB cable, allow USB debugging on it, then try again.");
        std::process::exit(1)
    };
    let out = adb(&["-s", serial, "shell", "pm", "grant", "dev.oxide.remote", "android.permission.WRITE_SECURE_SETTINGS"], 8).unwrap_or_default();
    if out.to_lowercase().contains("unknown package") || out.to_lowercase().contains("exception") {
        notify("Phone screen setup", "Install the Oxide Remote app on the phone first.");
        std::process::exit(1)
    }
    notify(
        "Phone screen ready",
        "In the Oxide Remote app, switch on \"Show this phone on my PCs\" (its settings). Then Super+P shows the phone, also over Wi-Fi.",
    );
    std::process::exit(0)
}

/// Pair this PC with the phone's wireless debugging, once (Android asks for it on top of the USB trust).
/// The phone scans a QR code `WIFI:T:ADB;S:<name>;P:<password>;;` (Wireless debugging > Pair device with QR code); it
/// then announces a pairing service under `name`, and this pairs with it. Progress goes to stdout, one line each.
pub fn pair(name: &str, password: &str) -> ! {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(120) {
        let list = adb(&["mdns", "services"], 4).unwrap_or_default();
        let found = list.lines().filter(|l| l.contains("_adb-tls-pairing") && l.contains(name)).find_map(|l| {
            l.split_whitespace().find(|w| w.contains(':') && w.chars().next().is_some_and(|c| c.is_ascii_digit())).map(str::to_string)
        });
        if let Some(addr) = found {
            println!("pairing {addr}");
            let out = adb(&["pair", &addr, password], 20).unwrap_or_default();
            if out.to_lowercase().contains("success") {
                println!("paired");
                // connect right away: the phone announces its connect service a moment later
                std::thread::sleep(Duration::from_millis(1500));
                let _ = find();
                notify("Phone paired", "Super+P shows your phone over Wi-Fi now.");
                std::process::exit(0)
            }
            println!("failed {}", out.trim());
            std::process::exit(1)
        }
        std::thread::sleep(Duration::from_millis(700));
    }
    println!("timeout");
    std::process::exit(1)
}

pub fn cmd(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        Some("setup") => setup(),
        Some("unlock") => unlock(),
        Some("pair") if args.len() >= 3 => pair(&args[1], &args[2]),
        _ => show(),
    }
}
