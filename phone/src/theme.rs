//! Colors, type and shapes. The palette follows the phone: dark or light, and its Material You colors when the
//! phone has them (Android 12+); otherwise Oxide's own colors. It can also follow the PC's Oxide theme.
use std::sync::RwLock;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pal {
    pub dark: bool,
    pub bg: Color32,
    /// cards
    pub card: Color32,
    /// raised parts on cards: fields, chips
    pub raised: Color32,
    /// sunken parts: tracks, the screen's frame
    pub deep: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    /// tonal buttons and selected chips
    pub container: Color32,
    pub on_container: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
}

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

impl Pal {
    /// Oxide's own colors.
    pub fn oxide(dark: bool) -> Pal {
        if dark {
            Pal {
                dark,
                bg: rgb(0x14151c),
                card: rgb(0x1d1f29),
                raised: rgb(0x272a37),
                deep: rgb(0x101117),
                line: rgb(0x2e3140),
                text: rgb(0xe6e8f2),
                muted: rgb(0x9097b0),
                accent: rgb(0x8fb0ff),
                on_accent: rgb(0x0e1a3a),
                container: rgb(0x2b3a63),
                on_container: rgb(0xd9e2ff),
                good: rgb(0x7fd18b),
                warn: rgb(0xe8b46a),
                bad: rgb(0xff8a8a),
            }
        } else {
            Pal {
                dark,
                bg: rgb(0xf7f7fb),
                card: rgb(0xffffff),
                raised: rgb(0xeef0f6),
                deep: rgb(0xe6e8ef),
                line: rgb(0xdfe2ea),
                text: rgb(0x1a1c24),
                muted: rgb(0x606577),
                accent: rgb(0x3d5fd6),
                on_accent: rgb(0xffffff),
                container: rgb(0xdce3ff),
                on_container: rgb(0x162a6b),
                good: rgb(0x2e8b45),
                warn: rgb(0xa86b10),
                bad: rgb(0xc93a3a),
            }
        }
    }

    /// From the phone's Material You tones (see OxideActivity.systemColors for the order).
    pub fn material(dark: bool, c: &[u32]) -> Option<Pal> {
        if c.len() < 14 {
            return None;
        }
        let t = |i: usize| rgb(c[i]);
        let (a100, a200, _a500, a600, a800) = (t(0), t(1), t(2), t(3), t(4));
        let (n10, n50, n100, n800, n900) = (t(5), t(6), t(7), t(8), t(9));
        let (s100, s700, nv200, nv700) = (t(10), t(11), t(12), t(13));
        let base = Pal::oxide(dark);
        Some(if dark {
            Pal {
                dark,
                bg: n900,
                card: mix(n900, n800, 0.55),
                raised: n800,
                deep: mix(n900, Color32::BLACK, 0.25),
                line: mix(n900, n100, 0.12),
                text: n100,
                muted: nv200,
                accent: a200,
                on_accent: a800,
                container: s700,
                on_container: s100,
                ..base
            }
        } else {
            Pal {
                dark,
                bg: n10,
                card: n50,
                raised: n100,
                deep: mix(n100, n800, 0.06),
                line: mix(n10, n900, 0.12),
                text: n900,
                muted: nv700,
                accent: a600,
                on_accent: n10,
                container: a100,
                on_container: a800,
                ..base
            }
        })
    }

    /// From the PC's Oxide theme (hex colors: bg, card, text, muted, accent, on_accent).
    pub fn from_pc(hex: &[String]) -> Option<Pal> {
        let p: Vec<Color32> = hex.iter().filter_map(|h| Color32::from_hex(h).ok()).collect();
        if p.len() < 6 {
            return None;
        }
        let (bg, card, text, muted, accent, on_accent) = (p[0], p[1], p[2], p[3], p[4], p[5]);
        let dark = (bg.r() as u32 + bg.g() as u32 + bg.b() as u32) < 384;
        let base = Pal::oxide(dark);
        Some(Pal {
            dark,
            bg,
            card,
            raised: mix(card, text, 0.07),
            deep: mix(bg, Color32::BLACK, if dark { 0.25 } else { 0.05 }),
            line: mix(card, text, 0.12),
            text,
            muted,
            accent,
            on_accent,
            container: mix(card, accent, 0.28),
            on_container: text,
            ..base
        })
    }
}

static PAL: RwLock<Option<Pal>> = RwLock::new(None);

pub fn pal() -> Pal {
    PAL.read().ok().and_then(|p| *p).unwrap_or_else(|| Pal::oxide(true))
}

pub fn r(v: u8) -> CornerRadius {
    CornerRadius::same(v)
}

// ------------------------------------------------------------- type

pub const ICONS: &str = "icons";
pub const ICONS_FILL: &str = "icons-fill";
pub const MEDIUM: &str = "medium";
pub const SEMIBOLD: &str = "semibold";

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn icon_font(size: f32, fill: bool) -> FontId {
    FontId::new(size, FontFamily::Name(if fill { ICONS_FILL } else { ICONS }.into()))
}

pub fn fonts(ctx: &egui::Context) {
    let mut f = FontDefinitions::default();
    let add = |f: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        f.font_data.insert(name.into(), std::sync::Arc::new(FontData::from_static(bytes)));
    };
    add(&mut f, "plex", include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"));
    add(&mut f, "plex-medium", include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf"));
    add(&mut f, "plex-semibold", include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"));
    add(&mut f, "plex-mono", include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"));
    add(&mut f, "phosphor", include_bytes!("../assets/fonts/Phosphor.ttf"));
    add(&mut f, "phosphor-fill", include_bytes!("../assets/fonts/Phosphor-Fill.ttf"));
    // Plex first, egui's own fonts after it for anything Plex lacks (emoji, symbols)
    let fallback: Vec<String> = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let with = |first: &str| std::iter::once(first.to_string()).chain(fallback.iter().cloned()).collect::<Vec<_>>();
    f.families.insert(FontFamily::Proportional, with("plex"));
    f.families.insert(FontFamily::Name(MEDIUM.into()), with("plex-medium"));
    f.families.insert(FontFamily::Name(SEMIBOLD.into()), with("plex-semibold"));
    let mono: Vec<String> = std::iter::once("plex-mono".to_string()).chain(f.families.get(&FontFamily::Monospace).cloned().unwrap_or_default()).collect();
    f.families.insert(FontFamily::Monospace, mono);
    f.families.insert(FontFamily::Name(ICONS.into()), vec!["phosphor".into()]);
    f.families.insert(FontFamily::Name(ICONS_FILL.into()), vec!["phosphor-fill".into()]);
    ctx.set_fonts(f);
}

/// Apply a palette to egui's widgets.
pub fn apply(ctx: &egui::Context, p: Pal) {
    let changed = PAL.read().ok().and_then(|x| *x) != Some(p);
    if !changed {
        return;
    }
    if let Ok(mut w) = PAL.write() {
        *w = Some(p);
    }
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.extreme_bg_color = p.raised;
    v.faint_bg_color = p.card;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = mix(p.accent, p.bg, 0.55);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    v.handle_shape = egui::style::HandleShape::Circle;
    v.slider_trailing_fill = true;
    v.hyperlink_color = p.accent;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = r(28);
    v.menu_corner_radius = r(16);
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(if p.dark { 90 } else { 40 }) };
    let w = &mut v.widgets;
    for s in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = r(14);
        s.fg_stroke = Stroke::new(1.5, p.text);
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    w.noninteractive.bg_fill = p.card;
    w.inactive.bg_fill = p.raised;
    w.inactive.weak_bg_fill = p.raised;
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.weak_bg_fill = mix(p.raised, p.text, 0.08);
    w.hovered.bg_fill = mix(p.raised, p.text, 0.08);
    w.hovered.bg_stroke = Stroke::NONE;
    w.active.weak_bg_fill = mix(p.raised, p.text, 0.14);
    w.active.bg_fill = mix(p.raised, p.text, 0.14);
    w.active.bg_stroke = Stroke::NONE;
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(10.0, 10.0);
        s.spacing.button_padding = egui::vec2(16.0, 10.0);
        s.spacing.interact_size.y = 44.0;
        s.spacing.slider_width = 200.0;
        s.spacing.slider_rail_height = 6.0;
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.interaction.tooltip_delay = 10.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(16.0));
        s.text_styles.insert(egui::TextStyle::Button, medium(15.0));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Heading, semibold(26.0));
        s.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(15.0));
    });
}
