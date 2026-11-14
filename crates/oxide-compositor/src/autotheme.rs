//! The theme that follows the wallpaper (`theme.preset = "auto"`).
//!
//! With the bundled shell the wallpaper is a file named in wallpaper.json, so its colors are read from the file.
//! A shell of your own draws the wallpaper itself (or a tool such as swww does): then the compositor looks at
//! what the background layer shows, a moment after it changed. Both end in `auto-theme.json`, from which the
//! palette is built (see oxide_config::autotheme), and the theme files are rewritten, so the shell, the
//! terminal and GTK follow.
use std::time::Duration;

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use oxide_config::autotheme::{self, Seed};
use smithay::utils::{Physical, Rectangle};

use crate::state::Oxide;

/// Learn from the pixels (or the file) and, when the seed changed, store it and rewrite the theme files.
fn apply(seed: Seed, theme: oxide_config::theme::ThemeCfg) {
    if autotheme::load_seed().as_ref() == Some(&seed) {
        return;
    }
    if autotheme::save_seed(&seed).is_ok() {
        let _ = oxide_config::theme::write_all(&theme);
    }
}

fn wallpaper_json() -> String {
    std::fs::read_to_string(oxide_config::Config::path().with_file_name("wallpaper.json")).unwrap_or_default()
}

/// The picture file the bundled shell shows, if any.
fn wallpaper_file(v: &serde_json::Value) -> Option<std::path::PathBuf> {
    match v["mode"].as_str().unwrap_or("photo") {
        "image" => v["path"].as_str().filter(|p| !p.is_empty()).map(std::path::PathBuf::from),
        "photo" | "scene1" | "scene2" => {
            let name = v["photo"].as_str().unwrap_or("dolomites");
            oxide_config::wallpaper_dirs().into_iter().map(|d| d.join(format!("{name}.jpg"))).find(|p| p.exists())
        }
        _ => None,
    }
}

impl Oxide {
    pub fn auto_theme_start(&mut self) {
        let timer = Timer::from_duration(Duration::from_millis(1500));
        let _ = self.loop_handle.insert_source(timer, |_, _, state| {
            state.auto_theme_tick();
            TimeoutAction::ToDuration(Duration::from_millis(1500))
        });
    }

    fn auto_theme_tick(&mut self) {
        if self.config.theme.preset != "auto" {
            return;
        }
        let theme = self.config.theme.clone();
        if self.config.default_shell_active() {
            let text = wallpaper_json();
            let stamp = format!("{text}|{}", autotheme::load_seed().is_some());
            if stamp == self.auto_stamp {
                return;
            }
            self.auto_stamp = stamp;
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            std::thread::spawn(move || {
                let seed = match v["mode"].as_str().unwrap_or("photo") {
                    "color" => v["color"].as_str().and_then(autotheme::seed_from_color),
                    "theme" => Some(Seed::default()),
                    _ => wallpaper_file(&v).and_then(|f| autotheme::samples_from_file(&f)).map(|s| autotheme::seed_from_samples(&s)),
                };
                if let Some(seed) = seed {
                    apply(seed, theme);
                }
            });
            return;
        }
        // an own shell: look at the background layer once it has been still for a moment
        let ready = match self.bg_dirty {
            Some((first, last)) => last.elapsed() > Duration::from_millis(1200) || first.elapsed() > Duration::from_secs(8),
            None => autotheme::load_seed().is_none() && !self.monitors.is_empty() && self.auto_stamp.is_empty(),
        };
        if !ready {
            return;
        }
        self.bg_dirty = None;
        self.auto_stamp = "sampled".into();
        let Some(out) = self.monitors.first().map(|m| m.output.clone()) else { return };
        let Some(mode) = out.current_mode() else { return };
        let has_bg = smithay::desktop::layer_map_for_output(&out).layers().any(|l| l.layer() == smithay::wayland::shell::wlr_layer::Layer::Background);
        if !has_bg {
            return;
        }
        let region = Rectangle::<i32, Physical>::new((0, 0).into(), mode.size);
        self.sampling_bg = true;
        let px = self.capture(&out, region, false);
        self.sampling_bg = false;
        let Some(px) = px else { return };
        let (w, h) = (mode.size.w as usize, mode.size.h as usize);
        let mut samples: Vec<[u8; 3]> = vec![];
        let (sx, sy) = ((w / 80).max(1), (h / 48).max(1));
        for y in (0..h).step_by(sy) {
            for x in (0..w).step_by(sx) {
                let i = (y * w + x) * 4;
                if let Some(p) = px.get(i..i + 3) {
                    samples.push([p[2], p[1], p[0]]); // BGRA
                }
            }
        }
        std::thread::spawn(move || apply(autotheme::seed_from_samples(&samples), theme));
    }
}
