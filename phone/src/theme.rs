//! Colors and shapes, in the look of Oxide's settings (dark, flat layers, one accent).
use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x26);
pub const CARD: Color32 = Color32::from_rgb(0x22, 0x23, 0x34);
pub const RAISED: Color32 = Color32::from_rgb(0x2b, 0x2d, 0x42);
pub const DEEP: Color32 = Color32::from_rgb(0x15, 0x16, 0x1f);
pub const LINE: Color32 = Color32::from_rgb(0x34, 0x36, 0x4f);
pub const TEXT: Color32 = Color32::from_rgb(0xc8, 0xcf, 0xf1);
pub const MUTED: Color32 = Color32::from_rgb(0x82, 0x89, 0xb0);
pub const ACCENT: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);
pub const INK: Color32 = Color32::from_rgb(0x14, 0x16, 0x24);
pub const GOOD: Color32 = Color32::from_rgb(0x9e, 0xce, 0x6a);
pub const WARN: Color32 = Color32::from_rgb(0xe0, 0xaf, 0x68);
pub const BAD: Color32 = Color32::from_rgb(0xf7, 0x76, 0x8e);

pub fn r(v: u8) -> CornerRadius {
    CornerRadius::same(v)
}

pub fn apply(ctx: &egui::Context, scale: f32) {
    ctx.set_pixels_per_point(scale);
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = DEEP;
    v.faint_bg_color = CARD;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, INK);
    let w = &mut v.widgets;
    for s in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = r(10);
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.inactive.bg_fill = RAISED;
    w.inactive.weak_bg_fill = RAISED;
    w.inactive.bg_stroke = Stroke::new(1.0, LINE);
    w.hovered.weak_bg_fill = LINE;
    w.hovered.bg_fill = LINE;
    w.active.weak_bg_fill = LINE;
    w.active.bg_fill = LINE;
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(10.0, 10.0);
        s.spacing.button_padding = egui::vec2(14.0, 10.0);
        s.spacing.interact_size.y = 44.0;
        s.spacing.slider_width = 220.0;
        use egui::{FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::proportional(16.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(16.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(13.0));
        s.text_styles.insert(TextStyle::Heading, FontId::proportional(24.0));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(15.0));
    });
}
