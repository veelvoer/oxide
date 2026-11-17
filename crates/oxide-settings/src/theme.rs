//! Visual style: colors, spacing and the small animation helpers shared by all pages.
use eframe::egui::{self, Color32, Context, CornerRadius, FontId, Stroke, TextStyle, Visuals};

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::RwLock;

/// Colors in use; replaced when the theme changes (see `set_theme`).
#[derive(Clone, Copy)]
struct Pal {
    bg: Color32,
    sidebar: Color32,
    card: Color32,
    card_hover: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    on_accent: Color32,
    soft: Color32,
    good: Color32,
    warn: Color32,
    bad: Color32,
    blue: Color32,
    field: Color32,
    deep: Color32,
    light: bool,
}

static PAL: RwLock<Pal> = RwLock::new(Pal {
    bg: Color32::from_rgb(27, 25, 23),
    sidebar: Color32::from_rgb(33, 31, 29),
    card: Color32::from_rgb(35, 33, 32),
    card_hover: Color32::from_rgb(44, 41, 38),
    border: Color32::from_rgb(54, 50, 46),
    text: Color32::from_rgb(233, 228, 222),
    muted: Color32::from_rgb(154, 146, 138),
    accent: Color32::from_rgb(212, 87, 46),
    on_accent: Color32::from_rgb(29, 15, 9),
    soft: Color32::from_rgb(224, 138, 99),
    good: Color32::from_rgb(95, 163, 145),
    warn: Color32::from_rgb(217, 164, 65),
    bad: Color32::from_rgb(200, 80, 74),
    blue: Color32::from_rgb(111, 168, 220),
    field: Color32::from_rgb(44, 41, 38),
    deep: Color32::from_rgb(22, 20, 19),
    light: false,
});
static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn initialized() -> bool { STARTED.swap(true, Ordering::Relaxed) }
static ROUND: AtomicU8 = AtomicU8::new(8);
static EXPRESSIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CONTAINER_COLORS: RwLock<(Color32, Color32)> = RwLock::new((Color32::from_rgb(90, 50, 35), Color32::from_rgb(255, 220, 200)));

/// The Material Expressive style is on (pill controls, Material switches, springy presses).
pub fn expressive() -> bool {
    EXPRESSIVE.load(Ordering::Relaxed)
}

/// Tonal container color and the text on it (selected items, tonal buttons).
#[allow(non_snake_case)]
pub fn CONTAINER() -> Color32 {
    CONTAINER_COLORS.read().unwrap().0
}
#[allow(non_snake_case)]
pub fn ON_CONTAINER() -> Color32 {
    CONTAINER_COLORS.read().unwrap().1
}

/// A theme change fades from the old colors to the new ones: (colors before, start of the fade).
static FADE: RwLock<Option<(Pal, std::time::Instant)>> = RwLock::new(None);
const FADE_SECS: f32 = 0.4;

fn fade_t() -> Option<f32> {
    let f = FADE.read().unwrap();
    let (_, start) = (*f)?;
    let t = start.elapsed().as_secs_f32() / FADE_SECS;
    (t < 1.0).then(|| ease_out(t))
}

fn pal() -> Pal {
    let to = *PAL.read().unwrap();
    let from = FADE.read().unwrap().map(|f| f.0);
    match (from, fade_t()) {
        (Some(a), Some(t)) => {
            let l = |x: Color32, y: Color32| lerp_color(x, y, t);
            Pal {
                bg: l(a.bg, to.bg), sidebar: l(a.sidebar, to.sidebar), card: l(a.card, to.card), card_hover: l(a.card_hover, to.card_hover),
                border: l(a.border, to.border), text: l(a.text, to.text), muted: l(a.muted, to.muted), accent: l(a.accent, to.accent),
                on_accent: l(a.on_accent, to.on_accent), soft: l(a.soft, to.soft), good: l(a.good, to.good), warn: l(a.warn, to.warn),
                bad: l(a.bad, to.bad), blue: l(a.blue, to.blue), field: l(a.field, to.field), deep: l(a.deep, to.deep),
                light: if t < 0.5 { a.light } else { to.light },
            }
        }
        _ => to,
    }
}

/// Call once per frame: keeps the visuals following a running theme fade.
pub fn tick(ctx: &Context) {
    if fade_t().is_some() {
        apply(ctx);
        ctx.request_repaint();
    } else if FADE.read().unwrap().is_some() {
        *FADE.write().unwrap() = None;
        apply(ctx);
    }
}

// The color names read like constants at the call sites.
#[allow(non_snake_case)]
pub fn BG() -> Color32 { pal().bg }
#[allow(non_snake_case)]
pub fn SIDEBAR() -> Color32 { pal().sidebar }
#[allow(non_snake_case)]
pub fn CARD() -> Color32 { pal().card }
#[allow(non_snake_case)]
pub fn CARD_HOVER() -> Color32 { pal().card_hover }
#[allow(non_snake_case)]
pub fn BORDER() -> Color32 { pal().border }
#[allow(non_snake_case)]
pub fn TEXT() -> Color32 { pal().text }
#[allow(non_snake_case)]
pub fn MUTED() -> Color32 { pal().muted }
#[allow(non_snake_case)]
pub fn ACCENT() -> Color32 { pal().accent }
#[allow(non_snake_case)]
pub fn ON_ACCENT() -> Color32 { pal().on_accent }
#[allow(non_snake_case)]
pub fn COPPER() -> Color32 { pal().soft }
#[allow(non_snake_case)]
pub fn GOOD() -> Color32 { pal().good }
#[allow(non_snake_case)]
pub fn WARN() -> Color32 { pal().warn }
#[allow(non_snake_case)]
pub fn BAD() -> Color32 { pal().bad }
#[allow(non_snake_case)]
pub fn BLUE() -> Color32 { pal().blue }
#[allow(non_snake_case)]
pub fn PINK() -> Color32 { pal().bad }
#[allow(non_snake_case)]
pub fn TEAL() -> Color32 { pal().good }
#[allow(non_snake_case)]
pub fn VIOLET() -> Color32 { pal().muted }
/// Inset areas (previews, trays): a step away from the page color.
#[allow(non_snake_case)]
pub fn DEEP() -> Color32 { pal().deep }
#[allow(non_snake_case)]
pub fn FIELD() -> Color32 { pal().field }

/// Corner radius; `r` is the design value at the default roundness of 8.
pub fn radius(r: u8) -> CornerRadius {
    let k = ROUND.load(Ordering::Relaxed) as f32 / 8.0;
    CornerRadius::same(((r as f32 * k).round() as i32).clamp(0, 40) as u8)
}

fn hex(c: &str) -> Color32 {
    crate::widgets::parse_hex(c)
}

fn shade(c: Color32, t: f32, toward_white: bool) -> Color32 {
    let to = if toward_white { Color32::WHITE } else { Color32::BLACK };
    lerp_color(c, to, t)
}

/// Switch the whole app to a theme palette and corner roundness.
pub fn set_theme(ctx: &Context, p: &oxide_config::theme::Palette) {
    let bg = hex(&p.bg);
    let card = hex(&p.surface);
    let raised = hex(&p.raised);
    let before = pal();
    if FADE.read().unwrap().is_some() || initialized() {
        *FADE.write().unwrap() = Some((before, std::time::Instant::now()));
    }
    {
        let mut w = PAL.write().unwrap();
        *w = Pal {
            bg,
            sidebar: shade(bg, 0.0, false).lerp_to_gamma(card, 0.6),
            card,
            card_hover: raised,
            border: hex(&p.line),
            text: hex(&p.text),
            muted: hex(&p.muted),
            accent: hex(&p.accent),
            on_accent: hex(&p.ink),
            soft: hex(&p.accent_soft),
            good: hex(&p.good),
            warn: hex(&p.warn),
            bad: hex(&p.bad),
            blue: hex(&p.blue),
            field: raised,
            deep: lerp_color(bg, Color32::BLACK, if p.light { 0.07 } else { 0.22 }),
            light: p.light,
        };
    }
    ROUND.store(p.radius.min(24) as u8, Ordering::Relaxed);
    EXPRESSIVE.store(p.style == "expressive", Ordering::Relaxed);
    if !p.container.is_empty() {
        *CONTAINER_COLORS.write().unwrap() = (hex(&p.container), hex(&p.on_container));
    }
    apply(ctx);
}

pub fn medium() -> egui::FontFamily {
    egui::FontFamily::Name("medium".into())
}

fn install_fonts(ctx: &Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut f = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 4] = [
        ("plex-regular", include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf")),
        ("plex-medium", include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf")),
        ("plex-semibold", include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf")),
        ("plex-mono", include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")),
    ];
    for (name, bytes) in faces {
        f.font_data.insert(name.to_string(), std::sync::Arc::new(FontData::from_static(bytes)));
    }
    f.families.entry(FontFamily::Proportional).or_default().insert(0, "plex-regular".into());
    f.families.entry(FontFamily::Monospace).or_default().insert(0, "plex-mono".into());
    f.families.insert(medium(), vec!["plex-medium".into(), "plex-regular".into()]);
    f.families.insert(FontFamily::Name("semibold".into()), vec!["plex-semibold".into(), "plex-regular".into()]);
    ctx.set_fonts(f);
}

pub fn apply(ctx: &Context) {
    install_fonts(ctx);
    let pal = pal();
    let mut v = if pal.light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = BG();
    v.window_fill = CARD();
    v.extreme_bg_color = if pal.light { lerp_color(pal.bg, Color32::WHITE, 0.65) } else { shade(pal.bg, 0.25, false) };
    v.faint_bg_color = CARD();
    v.override_text_color = Some(TEXT());
    v.selection.bg_fill = ACCENT();
    v.selection.stroke = Stroke::new(1.0, ON_ACCENT());
    v.hyperlink_color = COPPER();
    v.window_corner_radius = radius(10);
    v.menu_corner_radius = radius(8);
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.45 };
    let w = &mut v.widgets;
    for s in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        // Expressive: every button and field is a pill
        s.corner_radius = if expressive() { egui::CornerRadius::same(255) } else { radius(4) };
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER());
    w.inactive.bg_fill = FIELD();
    w.inactive.weak_bg_fill = FIELD();
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER());
    w.hovered.bg_fill = shade(FIELD(), 0.08, !pal.light);
    w.hovered.weak_bg_fill = shade(FIELD(), 0.06, !pal.light);
    w.hovered.bg_stroke = Stroke::new(1.0, MUTED());
    w.active.bg_fill = shade(FIELD(), 0.14, !pal.light);
    w.active.weak_bg_fill = shade(FIELD(), 0.12, !pal.light);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT());
    ctx.set_visuals(v);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 5.0);
        style.spacing.interact_size.y = 28.0;
        style.spacing.slider_width = 200.0;
        style.spacing.slider_rail_height = 3.0;
        style.animation_time = 0.12;
        style.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(12.5));
        style.text_styles.insert(TextStyle::Heading, FontId::new(26.0, medium()));
        style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.5));
    });
}

/// Smoothly move a value toward `target` (ease-out), keyed by `id`.
pub fn tween(ctx: &Context, id: impl std::hash::Hash + std::fmt::Debug, target: f32, seconds: f32) -> f32 {
    ctx.animate_value_with_time(egui::Id::new(id), target, seconds)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// A value that follows its target like a physical spring (Material 3 spatial spring: stiffness 380, damping ratio
/// 0.8). Interruptible: a new target keeps the current speed. Returns the current value.
pub fn spring(ctx: &Context, id: egui::Id, target: f32) -> f32 {
    spring_with(ctx, id, target, 380.0, 0.8)
}

/// `spring` with its own stiffness and damping ratio (Hearth: fast 800/0.6, move 380/0.8, enter 320/0.72).
pub fn spring_with(ctx: &Context, id: egui::Id, target: f32, k: f32, ratio: f32) -> f32 {
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.05);
    let (mut x, mut v) = ctx.data(|d| d.get_temp::<(f32, f32)>(id)).unwrap_or((target, 0.0));
    let c = 2.0 * ratio * k.sqrt();
    // a few small steps per frame keep it stable
    for _ in 0..4 {
        let h = dt / 4.0;
        let a = -k * (x - target) - c * v;
        v += a * h;
        x += v * h;
    }
    if (x - target).abs() < 0.05 && v.abs() < 0.5 {
        x = target;
        v = 0.0;
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, (x, v)));
    x
}

/// The light theme is on.
pub fn is_light() -> bool {
    PAL.read().map(|p| p.light).unwrap_or(false)
}
