//! The PCs this phone is paired with (their keys included), kept in the app's private storage.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Pc {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub macs: Vec<String>,
    pub ltk: String,
    /// where the PC said it can be reached away from home ("host:port"), and the server with its notes
    pub remote: Vec<String>,
    /// its addresses on the network it was on last ("192.168.x.y:port")
    pub lan: Vec<String>,
    pub rendezvous: String,
    /// the PC's Oxide colors (for "Match my PC")
    pub palette: Vec<String>,
}

impl Pc {
    /// x.y.z.255 for the PC's address (Wake-on-LAN also goes there; routers often drop 255.255.255.255).
    pub fn subnet_broadcast(&self) -> Option<std::net::Ipv4Addr> {
        let ip: std::net::Ipv4Addr = self.host.parse().ok()?;
        let o = ip.octets();
        Some(std::net::Ipv4Addr::new(o[0], o[1], o[2], 255))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    pub phone_id: String,
    pub phone_name: String,
    pub pcs: Vec<Pc>,
    /// screen: quality and touch mode
    pub quality: u8,
    pub touchpad: bool,
    /// "phone" (Material You), "pc" (the last PC's Oxide theme), "dark", "light"
    pub theme: String,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let mut s: Store = std::fs::read_to_string(dir.join("pcs.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        s.dir = dir;
        if s.theme.is_empty() {
            s.theme = "phone".into();
        }
        if s.phone_id.is_empty() {
            s.phone_id = oxide_remote_proto::hex(&oxide_remote_proto::random::<8>());
            s.phone_name = "Phone".into();
            s.quality = 60;
            s.touchpad = true;
            s.save();
        }
        s
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join("pcs.json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, self.dir.join("pcs.json"));
        }
    }

    /// A newly paired PC (paired again: it keeps its place in the list).
    pub fn upsert(&mut self, pc: Pc) {
        match self.pcs.iter_mut().find(|p| p.id == pc.id) {
            Some(p) => *p = pc,
            None => self.pcs.push(pc),
        }
        self.save();
    }
}
