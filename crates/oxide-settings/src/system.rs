//! Thin wrappers around the system tools used by the settings pages:
//! NetworkManager (`nmcli`), BlueZ (`bluetoothctl`), PipeWire (`wpctl`, `pactl`), `brightnessctl`.
//! Everything blocking runs on a worker thread through `Bg` or `Jobs`.
use std::{
    process::Command,
    sync::mpsc::{channel, Receiver},
};

use eframe::egui::Context;

/// Run a command; stdout on success, stderr (or a generic message) on failure.
pub fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    match Command::new(cmd).args(args).output() {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            Err(if !err.is_empty() { err } else if !out.is_empty() { out } else { format!("{cmd} failed") })
        }
        Err(_) => Err(format!("{cmd} is not installed")),
    }
}

pub fn have(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

/// Background value: fetch on a thread, poll every frame.
pub struct Bg<T> {
    rx: Option<Receiver<T>>,
    pub value: Option<T>,
}

impl<T> Default for Bg<T> {
    fn default() -> Self {
        Self { rx: None, value: None }
    }
}

impl<T: Send + 'static> Bg<T> {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    pub fn fetch(&mut self, ctx: &Context, f: impl FnOnce() -> T + Send + 'static) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(f());
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(v) = rx.try_recv() {
                self.value = Some(v);
                self.rx = None;
            }
        }
    }
}

/// Fire-and-forget actions whose result is shown as a message.
#[derive(Default)]
pub struct Jobs {
    pending: Vec<Receiver<(String, Result<String, String>)>>,
}

impl Jobs {
    pub fn spawn(&mut self, ctx: &Context, label: &str, f: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let (tx, rx) = channel();
        self.pending.push(rx);
        let (ctx, label) = (ctx.clone(), label.to_string());
        std::thread::spawn(move || {
            let _ = tx.send((label, f()));
            ctx.request_repaint();
        });
    }

    /// Finished jobs since the last call.
    pub fn finished(&mut self) -> Vec<(String, Result<String, String>)> {
        let mut done = vec![];
        self.pending.retain(|rx| match rx.try_recv() {
            Ok(r) => {
                done.push(r);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        done
    }

    pub fn busy(&self) -> bool {
        !self.pending.is_empty()
    }
}

// ---------------------------------------------------------------- Wi-Fi

#[derive(Clone, Debug, Default)]
pub struct WifiNet {
    pub ssid: String,
    pub signal: u8,
    pub secured: bool,
    pub active: bool,
    pub saved: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WifiDetails {
    pub ip: String,
    pub gateway: String,
    pub dns: String,
    pub mac: String,
}

#[derive(Clone, Debug, Default)]
pub struct WifiState {
    pub available: bool,
    pub enabled: bool,
    pub device: String,
    pub nets: Vec<WifiNet>,
    /// Saved wifi profiles: (name, autoconnect)
    pub saved: Vec<(String, bool)>,
    pub details: Option<WifiDetails>,
}

/// Split one line of `nmcli -t` output (fields separated by ':', escaped as '\:').
pub fn split_terse(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.last_mut().unwrap().push(n);
                }
            }
            ':' => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out
}

pub fn wifi_state() -> WifiState {
    let Ok(radio) = run("nmcli", &["radio", "wifi"]) else { return WifiState::default() };
    let mut st = WifiState { available: true, enabled: radio.trim() == "enabled", ..Default::default() };

    if let Ok(devs) = run("nmcli", &["-t", "-f", "DEVICE,TYPE", "dev"]) {
        st.device = devs.lines().map(split_terse).find(|f| f.get(1).is_some_and(|t| t == "wifi")).map(|f| f[0].clone()).unwrap_or_default();
    }
    if let Ok(conns) = run("nmcli", &["-t", "-f", "NAME,TYPE,AUTOCONNECT", "connection", "show"]) {
        for f in conns.lines().map(split_terse) {
            if f.len() >= 3 && f[1] == "802-11-wireless" {
                st.saved.push((f[0].clone(), f[2] == "yes"));
            }
        }
    }
    if st.enabled {
        if let Ok(list) = run("nmcli", &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"]) {
            for f in list.lines().map(split_terse) {
                if f.len() < 4 || f[1].is_empty() {
                    continue;
                }
                let net = WifiNet {
                    saved: st.saved.iter().any(|(n, _)| *n == f[1]),
                    ssid: f[1].clone(),
                    signal: f[2].parse().unwrap_or(0),
                    secured: !f[3].is_empty() && f[3] != "--",
                    active: f[0] == "*",
                };
                // the same network often shows up once per access point: keep the strongest
                match st.nets.iter_mut().find(|n| n.ssid == net.ssid) {
                    Some(old) => {
                        old.active |= net.active;
                        if net.signal > old.signal {
                            old.signal = net.signal;
                        }
                    }
                    None => st.nets.push(net),
                }
            }
            st.nets.sort_by(|a, b| b.active.cmp(&a.active).then(b.signal.cmp(&a.signal)));
        }
        if !st.device.is_empty() && st.nets.iter().any(|n| n.active) {
            if let Ok(d) = run("nmcli", &["-t", "-f", "IP4.ADDRESS,IP4.GATEWAY,IP4.DNS,GENERAL.HWADDR", "dev", "show", &st.device]) {
                let mut det = WifiDetails::default();
                for f in d.lines().map(split_terse) {
                    let val = f.get(1).cloned().unwrap_or_default();
                    match f[0].split('[').next().unwrap_or("") {
                        "IP4.ADDRESS" if det.ip.is_empty() => det.ip = val,
                        "IP4.GATEWAY" => det.gateway = val,
                        "IP4.DNS" => {
                            if !det.dns.is_empty() {
                                det.dns.push_str(", ");
                            }
                            det.dns.push_str(&val)
                        }
                        "GENERAL.HWADDR" => det.mac = f[1..].join(":"),
                        _ => {}
                    }
                }
                st.details = Some(det);
            }
        }
    }
    st
}

// ---------------------------------------------------------------- Bluetooth

#[derive(Clone, Debug, Default)]
pub struct BtDevice {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub icon: String,
}

#[derive(Clone, Debug, Default)]
pub struct BtState {
    pub available: bool,
    pub powered: bool,
    pub discoverable: bool,
    pub pairable: bool,
    pub alias: String,
    pub devices: Vec<BtDevice>,
}

pub fn bt_state() -> BtState {
    let Ok(show) = run("bluetoothctl", &["show"]) else { return BtState::default() };
    let field = |name: &str| show.lines().find_map(|l| l.trim().strip_prefix(&format!("{name}:")).map(|v| v.trim().to_string()));
    let mut st = BtState {
        available: show.contains("Controller"),
        powered: field("Powered").as_deref() == Some("yes"),
        discoverable: field("Discoverable").as_deref() == Some("yes"),
        pairable: field("Pairable").as_deref() == Some("yes"),
        alias: field("Alias").unwrap_or_default(),
        devices: vec![],
    };
    if let Ok(list) = run("bluetoothctl", &["devices"]) {
        for l in list.lines() {
            let mut p = l.splitn(3, ' ');
            if p.next() != Some("Device") {
                continue;
            }
            let (Some(mac), name) = (p.next(), p.next().unwrap_or("")) else { continue };
            let info = run("bluetoothctl", &["info", mac]).unwrap_or_default();
            let yes = |k: &str| info.lines().any(|l| l.trim().strip_prefix(&format!("{k}:")).is_some_and(|v| v.trim() == "yes"));
            st.devices.push(BtDevice {
                mac: mac.to_string(),
                name: name.to_string(),
                paired: yes("Paired"),
                connected: yes("Connected"),
                trusted: yes("Trusted"),
                icon: info.lines().find_map(|l| l.trim().strip_prefix("Icon:").map(|v| v.trim().to_string())).unwrap_or_default(),
            });
        }
        st.devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)).then(a.name.cmp(&b.name)));
    }
    st
}

// ---------------------------------------------------------------- Sound

#[derive(Clone, Debug, Default)]
pub struct AudioDev {
    pub id: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
    pub default: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AudioStream {
    pub index: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AudioCard {
    pub name: String,
    pub description: String,
    pub active: String,
    pub profiles: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct AudioState {
    pub available: bool,
    pub sinks: Vec<AudioDev>,
    pub sources: Vec<AudioDev>,
    pub streams: Vec<AudioStream>,
    pub cards: Vec<AudioCard>,
}

fn parse_wpctl(status: &str, section: &str) -> Vec<AudioDev> {
    let mut out = vec![];
    let mut inside = false;
    for line in status.lines() {
        let t = line.trim_start_matches(|c: char| c == '│' || c == ' ' || c == '├' || c == '└' || c == '─');
        if t.starts_with(section) {
            inside = true;
            continue;
        }
        if inside {
            // a new heading ends the section
            if t.trim().is_empty() || (t.ends_with(':') && !t.contains('[')) {
                if out.is_empty() && t.trim().is_empty() {
                    continue;
                }
                break;
            }
            let default = t.starts_with('*');
            let t = t.trim_start_matches('*').trim();
            let Some((id, rest)) = t.split_once('.') else { continue };
            let Ok(id) = id.trim().parse::<u32>() else { continue };
            let (name, vol) = match rest.rsplit_once('[') {
                Some((n, v)) => (n.trim().to_string(), v.trim_end_matches(']').to_string()),
                None => (rest.trim().to_string(), String::new()),
            };
            let volume = vol.split_whitespace().skip_while(|w| *w != "vol:").nth(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            out.push(AudioDev { id, name, volume, muted: vol.contains("MUTED()"), default });
        }
    }
    out
}

pub fn audio_state() -> AudioState {
    let Ok(status) = run("wpctl", &["status"]) else { return AudioState::default() };
    let mut st = AudioState { available: true, ..Default::default() };
    st.sinks = parse_wpctl(&status, "Sinks:");
    st.sources = parse_wpctl(&status, "Sources:");

    if let Ok(j) = run("pactl", &["-f", "json", "list", "sink-inputs"]) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) {
            for s in v.as_array().into_iter().flatten() {
                let pct = s["volume"].as_object().and_then(|m| m.values().next()).and_then(|c| c["value_percent"].as_str()).and_then(|p| p.trim_end_matches('%').parse::<f32>().ok()).unwrap_or(100.0);
                st.streams.push(AudioStream {
                    index: s["index"].as_u64().unwrap_or(0) as u32,
                    name: s["properties"]["application.name"].as_str().or(s["properties"]["media.name"].as_str()).unwrap_or("Unknown app").to_string(),
                    volume: pct / 100.0,
                    muted: s["mute"].as_bool().unwrap_or(false),
                });
            }
        }
    }
    if let Ok(j) = run("pactl", &["-f", "json", "list", "cards"]) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) {
            for c in v.as_array().into_iter().flatten() {
                let mut profiles: Vec<(String, String)> = c["profiles"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .filter(|(_, p)| p["available"].as_bool().unwrap_or(true))
                            .map(|(k, p)| (k.clone(), p["description"].as_str().unwrap_or(k).to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                profiles.sort();
                st.cards.push(AudioCard {
                    name: c["name"].as_str().unwrap_or("").to_string(),
                    description: c["properties"]["device.description"].as_str().unwrap_or("Sound card").to_string(),
                    active: c["active_profile"].as_str().unwrap_or("").to_string(),
                    profiles,
                });
            }
        }
    }
    st
}

// ---------------------------------------------------------------- Brightness and power

#[derive(Clone, Debug, Default)]
pub struct PowerState {
    pub brightness: Option<f32>,
    pub battery: Option<(u32, String)>,
    pub profile: Option<(String, Vec<String>)>,
}

pub fn power_state() -> PowerState {
    let mut st = PowerState::default();
    if let Ok(b) = run("brightnessctl", &["-m"]) {
        // device,class,current,percent,max
        if let Some(p) = b.lines().next().and_then(|l| l.split(',').nth(3)) {
            st.brightness = p.trim_end_matches('%').parse::<f32>().ok().map(|v| v / 100.0);
        }
    }
    let bat = std::fs::read_dir("/sys/class/power_supply")
        .ok()
        .and_then(|d| d.flatten().find(|e| e.file_name().to_string_lossy().starts_with("BAT")).map(|e| e.path()));
    if let Some(p) = bat {
        let cap = std::fs::read_to_string(p.join("capacity")).ok().and_then(|c| c.trim().parse().ok());
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default().trim().to_string();
        if let Some(c) = cap {
            st.battery = Some((c, status));
        }
    }
    if have("powerprofilesctl") {
        if let (Ok(cur), Ok(list)) = (run("powerprofilesctl", &["get"]), run("powerprofilesctl", &["list"])) {
            let names: Vec<String> = list
                .lines()
                .filter_map(|l| {
                    let t = l.trim().trim_start_matches('*').trim();
                    t.strip_suffix(':').map(str::to_string)
                })
                .collect();
            st.profile = Some((cur.trim().to_string(), names));
        }
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terse_split_handles_escapes() {
        assert_eq!(split_terse("*:My\\:Net:77:WPA2"), vec!["*", "My:Net", "77", "WPA2"]);
        assert_eq!(split_terse(":Open::"), vec!["", "Open", "", ""]);
    }

    #[test]
    fn wpctl_sections() {
        let s = "Audio\n ├─ Devices:\n │     110. Foo [alsa]\n │  \n ├─ Sinks:\n │      76. HDMI [vol: 1.00]\n │  *   97. Speaker [vol: 0.55 MUTED()]\n │  \n ├─ Sources:\n │  *   73. Mic [vol: 0.94]\n │  \n ├─ Filters:\n";
        let sinks = parse_wpctl(s, "Sinks:");
        assert_eq!(sinks.len(), 2);
        assert!(sinks[1].default && sinks[1].muted);
        assert!((sinks[1].volume - 0.55).abs() < 1e-6);
        assert_eq!(parse_wpctl(s, "Sources:").len(), 1);
    }
}
