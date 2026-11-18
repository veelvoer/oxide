//! What this PC remembers: its id and name, the paired phones (with their long-term keys) and a few options.
//! Windows: %APPDATA%\OxideRemote\host.json; elsewhere ~/.config/oxide-remote-host/host.json.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Phone {
    pub id: String,
    pub name: String,
    pub ltk: String,
    pub paired: u64,
    pub last_seen: u64,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub pc_id: String,
    pub pc_name: String,
    pub phones: Vec<Phone>,
    /// reachable from the internet (router forwarding, encrypted notes)
    pub anywhere: bool,
    /// leave encrypted notes with this PC's addresses so phones find it on any network
    pub notes: bool,
    pub rendezvous: String,
}

impl Default for Store {
    fn default() -> Self {
        Self { pc_id: String::new(), pc_name: String::new(), phones: vec![], anywhere: false, notes: true, rendezvous: "https://ntfy.sh".into() }
    }
}

pub fn path() -> PathBuf {
    let dir = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("OxideRemote")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"))
            .join("oxide-remote-host")
    };
    dir.join("host.json")
}

pub fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "pc".into())
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Store {
    pub fn load() -> Self {
        let mut s: Store = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut changed = false;
        if s.pc_id.is_empty() {
            s.pc_id = oxide_remote_proto::hex(&oxide_remote_proto::random::<8>());
            changed = true;
        }
        if s.pc_name.is_empty() {
            s.pc_name = hostname();
            changed = true;
        }
        if changed {
            s.save();
        }
        s
    }

    pub fn save(&self) {
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    pub fn phone(&self, id: &str) -> Option<&Phone> {
        self.phones.iter().find(|p| p.id == id)
    }
}

/// MAC addresses of the real network cards (for Wake-on-LAN), without virtual adapters.
pub fn macs() -> Vec<String> {
    let Ok(nets) = sysinfo::Networks::new_with_refreshed_list() else { return vec![] };
    let mut v: Vec<String> = nets
        .iter()
        .filter(|(name, _)| {
            let n = name.to_string_lossy().to_lowercase();
            !["loopback", "virtual", "vmware", "vethernet", "bluetooth", "docker", "veth", "br-", "lo", "tailscale", "wsl", "pseudo", "tap", "tun"].iter().any(|b| n.contains(b))
        })
        .map(|(_, d)| d.mac_address().to_string())
        .filter(|m| m != "00:00:00:00:00:00")
        .collect();
    v.sort();
    v.dedup();
    v
}
