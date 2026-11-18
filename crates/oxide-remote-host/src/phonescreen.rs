//! Your phone's screen on this PC (scrcpy + adb), like `oxide-ctl phone` on Linux. On Windows the tools are
//! downloaded once from the official scrcpy release into the app's folder; Android's encrypted wireless debugging
//! is used for Wi-Fi (never the old unencrypted `adb tcpip`), paired once with a QR code.
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub type Status = Arc<Mutex<String>>;

fn cmd(program: &std::path::Path) -> Command {
    let c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = c;
        c.creation_flags(0x0800_0000); // no console window
        return c;
    }
    #[allow(unreachable_code)]
    c
}

fn tools_dir() -> PathBuf {
    crate::store::path().with_file_name("scrcpy")
}

/// A tool from the app's own folder, else from the PATH.
pub fn tool(name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Ok(rd) = std::fs::read_dir(tools_dir()) {
        for e in rd.flatten() {
            let p = e.path().join(&file);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(&file)).find(|f| f.is_file()))
}

pub fn ready() -> bool {
    tool("scrcpy").is_some() && tool("adb").is_some()
}

fn curl() -> Command {
    cmd(std::path::Path::new(if cfg!(windows) { "curl.exe" } else { "curl" }))
}

/// Download scrcpy (with adb) from its official GitHub release (Windows only; elsewhere: the package manager).
pub fn install_tools(status: Status) {
    std::thread::spawn(move || {
        let say = |t: &str| *status.lock().unwrap() = t.to_string();
        if !cfg!(windows) {
            say("Install scrcpy and adb with your package manager.");
            return;
        }
        say("Looking for the newest scrcpy…");
        let feed = curl().args(["-sL", "--max-time", "20", "https://github.com/Genymobile/scrcpy/releases.atom"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        let Some(tag) = feed.split("/releases/tag/").nth(1).map(|t| t.chars().take_while(|c| !matches!(c, '"' | '<' | '&' | ' ')).collect::<String>()) else {
            say("Could not reach GitHub. Check the connection.");
            return;
        };
        let dir = tools_dir();
        let _ = std::fs::create_dir_all(&dir);
        let zip = dir.join("scrcpy.zip");
        say(&format!("Downloading scrcpy {tag}…"));
        let url = format!("https://github.com/Genymobile/scrcpy/releases/download/{tag}/scrcpy-win64-{tag}.zip");
        let ok = curl().args(["-sfL", "--max-time", "300", "-o"]).arg(&zip).arg(&url).status().is_ok_and(|s| s.success());
        if !ok {
            say("The download failed.");
            return;
        }
        say("Unpacking…");
        // tar.exe ships with Windows 10 and later and reads zip files
        let ok = cmd(std::path::Path::new("tar.exe")).arg("-xf").arg(&zip).arg("-C").arg(&dir).status().is_ok_and(|s| s.success());
        let _ = std::fs::remove_file(&zip);
        say(if ok && ready() { "Ready." } else { "Unpacking failed." });
    });
}

fn adb(args: &[&str], secs: u64) -> Option<String> {
    let mut c = cmd(&tool("adb")?).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let start = Instant::now();
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
    let o = c.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr))
}

fn devices() -> Vec<String> {
    adb(&["devices"], 6)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let s = w.next()?;
            (w.next()? == "device").then(|| s.to_string())
        })
        .collect()
}

fn connect_mdns() {
    if let Some(list) = adb(&["mdns", "services"], 5) {
        for l in list.lines().filter(|l| l.contains("_adb-tls-connect")) {
            if let Some(a) = l.split_whitespace().find(|w| w.contains(':') && w.chars().next().is_some_and(|c| c.is_ascii_digit())) {
                let _ = adb(&["connect", a], 6);
            }
        }
    }
}

/// The phone: on USB, or over Wi-Fi through Android's wireless debugging.
fn find() -> Option<String> {
    let d = devices();
    if let Some(s) = d.iter().find(|s| !s.contains("._adb")).or(d.first()) {
        return Some(s.clone());
    }
    // the phone announces itself a moment after wireless debugging is on: ask for up to ~10 s
    for _ in 0..8 {
        connect_mdns();
        if let Some(s) = devices().into_iter().next() {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(1300));
    }
    None
}

/// Open the phone's screen. The phone's own display goes dark (it can stay in your pocket); a locked phone shows its
/// lock screen: Android hides its PIN pad from mirrors, so unlock with `unlock` below.
pub fn show(status: Status, child: Arc<Mutex<Option<Child>>>) {
    std::thread::spawn(move || {
        let say = |t: &str| *status.lock().unwrap() = t.to_string();
        let running = {
            let mut g = child.lock().unwrap();
            let alive = g.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_none());
            if alive {
                if let Some(c) = g.as_mut() {
                    let _ = c.kill();
                }
            }
            if alive || g.is_some() {
                *g = None;
            }
            alive
        };
        if running {
            say("Closed.");
            return;
        }
        let (Some(scrcpy), true) = (tool("scrcpy"), tool("adb").is_some()) else {
            say("Get the tools first (button above).");
            return;
        };
        say("Looking for the phone…");
        let Some(serial) = find() else {
            say("Phone not found: connect it with USB (USB debugging on), or pair over Wi-Fi below.");
            return;
        };
        let mut args = vec!["-s".to_string(), serial, "--window-title=Phone".into(), "--turn-screen-off".into(), "--power-off-on-close".into(), "--max-size=1600".into(), "--video-bit-rate=12M".into(), "--max-fps=60".into(), "--video-buffer=0".into(), "--no-audio".into()];
        if cfg!(not(windows)) {
            // Linux's openh264 only decodes Constrained Baseline; a virtual keyboard works on lock screens
            args.push("--video-codec-options=profile:int=1,level:int=4096".into());
            args.push("--keyboard=uhid".into());
        }
        let log = tools_dir().join("scrcpy.log");
        let _ = std::fs::create_dir_all(tools_dir());
        let errf = std::fs::File::create(&log).map(Stdio::from).unwrap_or_else(|_| Stdio::null());
        let mut c = cmd(&scrcpy);
        c.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(errf);
        // scrcpy must talk to the same adb that found the phone (two adb versions fight over the server)
        if let Some(a) = tool("adb") {
            c.env("ADB", a);
        }
        match c.spawn() {
            Ok(ch) => {
                *child.lock().unwrap() = Some(ch);
                std::thread::sleep(Duration::from_millis(3500));
                let exited = child.lock().unwrap().as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_some());
                if exited {
                    *child.lock().unwrap() = None;
                    let text = std::fs::read_to_string(&log).unwrap_or_default();
                    let tail: Vec<&str> = text.lines().rev().filter(|l| !l.trim().is_empty()).take(3).collect();
                    say(&format!("scrcpy stopped: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
                } else {
                    say("Showing the phone. Press the button again to close it.");
                }
            }
            Err(e) => say(&format!("scrcpy did not start: {e}")),
        }
    });
}

/// Unlock: wake the phone, pull the PIN pad up and type the PIN (never stored; sent through standard input).
pub fn unlock(status: Status, pin: String) {
    std::thread::spawn(move || {
        let say = |t: &str| *status.lock().unwrap() = t.to_string();
        let Some(serial) = find() else {
            say("Phone not found.");
            return;
        };
        let _ = adb(&["-s", &serial, "shell", "input keyevent KEYCODE_WAKEUP"], 6);
        std::thread::sleep(Duration::from_millis(400));
        let _ = adb(&["-s", &serial, "shell", "wm dismiss-keyguard"], 6);
        let _ = adb(&["-s", &serial, "shell", "input swipe 540 2300 540 900 220"], 6);
        std::thread::sleep(Duration::from_millis(500));
        let Some(adb_path) = tool("adb") else { return };
        let Ok(mut ch) = cmd(&adb_path).args(["-s", &serial, "shell", "input text \"$(cat)\" && input keyevent KEYCODE_ENTER"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
            say("adb did not start.");
            return;
        };
        if let Some(mut i) = ch.stdin.take() {
            use std::io::Write;
            let _ = i.write_all(pin.as_bytes());
        }
        let _ = ch.wait();
        say("PIN sent.");
    });
}

/// With the phone on USB, once: the Oxide Remote app may switch wireless debugging on and off.
pub fn setup(status: Status) {
    std::thread::spawn(move || {
        let say = |t: &str| *status.lock().unwrap() = t.to_string();
        let usb: Vec<String> = devices().into_iter().filter(|s| !s.contains("._adb") && !s.contains(':')).collect();
        let Some(serial) = usb.first() else {
            say("Connect the phone with USB and allow USB debugging, then try again.");
            return;
        };
        let out = adb(&["-s", serial, "shell", "pm", "grant", "dev.oxide.remote", "android.permission.WRITE_SECURE_SETTINGS"], 10).unwrap_or_default().to_lowercase();
        if out.contains("unknown package") || out.contains("exception") {
            say("Install the Oxide Remote app on the phone first.");
        } else {
            say("Done. In the app's settings switch on \"Show this phone on my PCs\", then pair over Wi-Fi below.");
        }
    });
}

/// Pair with the phone's wireless debugging: it scans a QR code `WIFI:T:ADB;S:<name>;P:<password>;;`, then announces
/// a pairing service under `name`.
pub fn pair(status: Status, name: String, password: String) {
    std::thread::spawn(move || {
        let say = |t: &str| *status.lock().unwrap() = t.to_string();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(120) {
            let list = adb(&["mdns", "services"], 5).unwrap_or_default();
            let found = list.lines().filter(|l| l.contains("_adb-tls-pairing") && l.contains(&name)).find_map(|l| l.split_whitespace().find(|w| w.contains(':') && w.chars().next().is_some_and(|c| c.is_ascii_digit())).map(str::to_string));
            if let Some(addr) = found {
                say("Pairing…");
                let out = adb(&["pair", &addr, &password], 20).unwrap_or_default().to_lowercase();
                if out.contains("success") {
                    say("Paired. Connecting…");
                    std::thread::sleep(Duration::from_millis(1500));
                    if find().is_some() {
                        say("Paired and connected. Press Show my phone.");
                    } else {
                        say("Paired, but the phone does not answer yet: switch Wireless debugging off and on on the phone, then press Show my phone.");
                    }
                } else {
                    say("Pairing failed. Try again.");
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(700));
        }
        say("No phone scanned the code in time.");
    });
}

pub fn random_word(n: usize) -> String {
    oxide_remote_proto::random::<16>().iter().take(n).map(|x| (b'a' + x % 26) as char).collect()
}
