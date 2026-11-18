//! Not Windows: a moving test picture and logged input, so the whole host can be tried (and tested) anywhere.
use super::{shrink_bgra, Image};

pub struct Capturer {
    n: u32,
}

impl Capturer {
    pub fn new() -> Result<Self, String> {
        Ok(Self { n: 0 })
    }

    /// A 1920x1080 test picture with a bar that moves; every call is a new frame.
    pub fn grab(&mut self, max_w: u32) -> Result<Option<Image>, String> {
        std::thread::sleep(std::time::Duration::from_millis(4));
        self.n = self.n.wrapping_add(1);
        let (w, h) = (1920usize, 1080usize);
        let mut px = vec![0u8; w * h * 4];
        let bar = (self.n as usize * 12) % w;
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let on = x >= bar && x < bar + 40;
                px[i] = if on { 255 } else { (x * 255 / w) as u8 };
                px[i + 1] = if on { 255 } else { (y * 255 / h) as u8 };
                px[i + 2] = if on { 255 } else { 90 };
                px[i + 3] = 255;
            }
        }
        Ok(Some(shrink_bgra(&px, w, h, w * 4, max_w as usize)))
    }
}

pub fn screen_size() -> (u32, u32) {
    (1920, 1080)
}
pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}
pub fn pointer_to(x: f64, y: f64) {
    eprintln!("[input] pointer to {x:.3},{y:.3}");
}
pub fn pointer_by(dx: f64, dy: f64) {
    eprintln!("[input] pointer by {dx:.1},{dy:.1}");
}
pub fn button(name: &str, pressed: bool) {
    eprintln!("[input] button {name} {}", if pressed { "down" } else { "up" });
}
pub fn scroll(dx: f64, dy: f64) {
    eprintln!("[input] scroll {dx:.1},{dy:.1}");
}
pub fn key(name: &str, mods: &[String]) -> Result<(), String> {
    eprintln!("[input] key {mods:?}+{name}");
    Ok(())
}
pub fn text(t: &str) -> Result<(), String> {
    eprintln!("[input] text {t:?}");
    Ok(())
}
pub fn action(name: &str, arg: Option<&str>) -> Result<(), String> {
    eprintln!("[action] {name} {arg:?}");
    Ok(())
}
pub fn os_name() -> String {
    "Linux (test host)".into()
}
pub fn battery() -> (Option<f32>, bool) {
    (None, false)
}
pub fn autostart() -> bool {
    false
}
pub fn set_autostart(_: bool) -> Result<(), String> {
    Err("only on Windows".into())
}
pub fn dpi_aware() {}
