//! The watch's own settings (the PCs and the phone id live in the shared store).
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "watch" (Oxide's dark colors on black), "custom" (your accent) or "pc" (the PC's own Oxide theme)
    pub theme: String,
    /// index into `ACCENTS`
    pub accent: usize,
    /// "pad" (touchpad) or "gyro" (move your hand, pinch to click)
    pub cursor: String,
    /// the gyro mouse: "move" (the hand moves the pointer like a mouse) or "point" (the pointer goes where the arm points)
    pub gyro_mode: String,
    /// worn on the right wrist (the arm points the other way along the watch)
    pub right_wrist: bool,
    /// pointer speed, 0.5 to 3
    pub speed: f32,
    pub invert_x: bool,
    pub invert_y: bool,
    /// the gyro mouse clicks on a pinch (1 pinch = left click, 3 = right click)
    pub pinch: bool,
    /// three quick pinches are a right click (a single one then waits a moment to be sure it is single)
    pub pinch_count: bool,
    /// how sharp a pinch must be (lower = more sensitive)
    pub pinch_level: f32,
    /// look for updates by itself
    pub auto_update: bool,
    /// what the phone gave this watch when it was paired
    pub token: String,
    /// the phone's Bluetooth address, to look there first
    pub phone_addr: String,
    #[serde(skip)]
    pub dir: PathBuf,
}

pub const ACCENTS: [(&str, u32); 8] = [
    ("Blue", 0x8fb0ff),
    ("Mint", 0x7fd9b5),
    ("Lime", 0xb6e36b),
    ("Amber", 0xf0c060),
    ("Orange", 0xff9a62),
    ("Rose", 0xff8fa8),
    ("Violet", 0xc3a3ff),
    ("Cyan", 0x6fd6e6),
];

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "watch".into(), accent: 0, cursor: "pad".into(), gyro_mode: "point".into(), right_wrist: false, speed: 1.0, invert_x: false, invert_y: false, pinch: true, pinch_count: false, pinch_level: 7.0, auto_update: true, token: String::new(), phone_addr: String::new(), dir: PathBuf::new() }
    }
}

impl Settings {
    pub fn load(dir: PathBuf) -> Self {
        let mut s: Settings = std::fs::read_to_string(dir.join("watch.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        s.dir = dir;
        // the sensitivity used to be a threshold in m/s2: now it is how many times louder than the noise a tap must be
        if s.pinch_level < 3.0 {
            s.pinch_level = 7.0;
        }
        s
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join("watch.json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, self.dir.join("watch.json"));
        }
    }
}
