//! Calibration screens for `scripts/gen_icon_metrics.py`: every icon (and a line of capitals per font) drawn at a known
//! center, so the optical offsets of the glyphs can be measured from a screenshot. Not part of the app.
use egui::{vec2, Align2, Color32, Pos2, Rect, Ui};

use crate::theme::*;

/// The icons the watch draws, in the order of the generated table.
pub const ICONS: &[&str] = &[
    "\u{E32E}", "\u{E3D4}", "\u{E272}", "\u{E3DA}", "\u{E2FA}", "\u{E330}", "\u{E44A}", "\u{E45C}", "\u{E3D0}", "\u{E5A4}", "\u{E5A6}", "\u{E058}", "\u{E182}", "\u{E0AE}", "\u{E1AC}", "\u{E094}", "\u{E33A}",
    "\u{E464}", "\u{E434}", "\u{E156}", "\u{E13A}", "\u{E32A}", "\u{E1E0}", "\u{E396}", "\u{E4F6}",
];

pub fn ui(ui: &mut Ui, what: &str) {
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0, Color32::BLACK);
    if what == "text" {
        for (n, font) in [FontId::proportional(30.0), medium(30.0), semibold(30.0)].into_iter().enumerate() {
            let c = Pos2::new(full.center().x, full.top() + 45.0 + n as f32 * 60.0);
            ui.painter().text(c, Align2::CENTER_CENTER, "HHHH", font, Color32::WHITE);
        }
        return;
    }
    for (i, g) in ICONS.iter().enumerate() {
        let (cx, cy) = (i % 5, i / 5);
        let c = Pos2::new(full.left() + 22.5 + cx as f32 * 45.0, full.top() + 22.5 + cy as f32 * 45.0);
        if what == "fixed" {
            crate::round::icon(ui, c, g, 26.0, Color32::WHITE);
        } else {
            ui.painter().text(c, Align2::CENTER_CENTER, *g, icon_font(26.0, false), Color32::WHITE);
        }
    }
    let _ = (vec2(0.0, 0.0), Rect::NOTHING);
}

use egui::FontId;
