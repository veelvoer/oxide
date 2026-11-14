//! What this PC remembers: its id and name, and the phones paired with it (with their long-term keys).
//! Kept in ~/.config/oxide/remote.json, readable only by you.
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    pub pc_id: String,
    pub pc_name: String,
    pub phones: Vec<Phone>,
}

pub fn path() -> PathBuf {
    oxide_config::Config::path().with_file_name("remote.json")
}

pub fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::fs::read_to_string("/proc/sys/kernel/hostname").ok().map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "linux".into())
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
        use std::os::unix::fs::PermissionsExt;
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).is_ok() {
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    pub fn phone(&self, id: &str) -> Option<&Phone> {
        self.phones.iter().find(|p| p.id == id)
    }
}

/// MAC addresses of the physical network interfaces (for Wake-on-LAN).
pub fn macs() -> Vec<String> {
    let mut v = vec![];
    let Ok(rd) = std::fs::read_dir("/sys/class/net") else { return v };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        // physical devices only (they have a "device" link); not lo, bridges, docker, VPNs
        if name == "lo" || !e.path().join("device").exists() {
            continue;
        }
        if let Ok(mac) = std::fs::read_to_string(e.path().join("address")) {
            let mac = mac.trim().to_string();
            if !mac.is_empty() && mac != "00:00:00:00:00:00" {
                v.push(mac);
            }
        }
    }
    v.sort();
    v
}
