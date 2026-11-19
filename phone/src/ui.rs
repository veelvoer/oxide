//! The app's building blocks, drawn to look the same on every phone: buttons, cards, bars, gauges.
use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::{platform, theme::*};

/// Phosphor icon glyphs (the subset bundled in assets/fonts).
pub mod ic {
    pub const MONITOR: &str = "\u{E32E}";
    pub const PLUS: &str = "\u{E3D4}";
    pub const ARROW_LEFT: &str = "\u{E058}";
    pub const CARET_RIGHT: &str = "\u{E13A}";
    pub const GEAR: &str = "\u{E272}";
    pub const DOTS: &str = "\u{E208}";
    pub const POWER: &str = "\u{E3DA}";
    pub const LIGHTNING: &str = "\u{E2DE}";
    pub const WIFI: &str = "\u{E4EA}";
    pub const GLOBE: &str = "\u{E288}";
    pub const EXPAND: &str = "\u{E1D0}";
    pub const SHRINK: &str = "\u{E1CE}";
    pub const KEYBOARD: &str = "\u{E2D8}";
    pub const MOUSE: &str = "\u{E33A}";
    pub const TAP: &str = "\u{EC90}";
    pub const CHART: &str = "\u{E156}";
    pub const CPU: &str = "\u{E610}";
    pub const MEMORY: &str = "\u{E9C4}";
    pub const DISKS: &str = "\u{E2A0}";
    pub const THERMOMETER: &str = "\u{E5C6}";
    pub const SLIDERS: &str = "\u{E434}";
    pub const PLAY: &str = "\u{E3D0}";
    pub const PAUSE: &str = "\u{E39E}";
    pub const PREV: &str = "\u{E5A4}";
    pub const NEXT: &str = "\u{E5A6}";
    pub const SPEAKER: &str = "\u{E44A}";
    pub const MUTE: &str = "\u{E45C}";
    pub const CLIPBOARD: &str = "\u{E196}";
    pub const COPY: &str = "\u{E1CA}";
    pub const SEND: &str = "\u{E396}";
    pub const SPARKLE: &str = "\u{E6A2}";
    pub const STOP: &str = "\u{E46C}";
    pub const LOCK: &str = "\u{E2FA}";
    pub const MOON: &str = "\u{E330}";
    pub const RESTART: &str = "\u{E036}";
    pub const TRASH: &str = "\u{E4A6}";
    pub const CHECK: &str = "\u{E182}";
    pub const CLOSE: &str = "\u{E4F6}";
    pub const DOWNLOAD: &str = "\u{E20C}";
    pub const BELL: &str = "\u{E0CE}";
    pub const WINDOW: &str = "\u{E5DA}";
    pub const GRID: &str = "\u{E464}";
    pub const WARNING: &str = "\u{E4E0}";
    pub const INFO: &str = "\u{E2CE}";
    pub const UP: &str = "\u{E08E}";
    pub const DOWN: &str = "\u{E03E}";
    pub const RIGHT: &str = "\u{E06C}";
    pub const LEFT_ARROW: &str = "\u{E058}";
    pub const BACKSPACE: &str = "\u{E0AE}";
    pub const ENTER: &str = "\u{E782}";
    pub const CLICK: &str = "\u{E7C8}";
    pub const BATTERY: &str = "\u{E0C2}";
    pub const PHONE: &str = "\u{E1E0}";
    pub const REFRESH: &str = "\u{E094}";
    pub const PENCIL: &str = "\u{E3B4}";
    pub const SEARCH: &str = "\u{E30C}";
    pub const OPEN: &str = "\u{E5DE}";
    pub const UPDATE: &str = "\u{E1AC}";
    pub const GAUGE: &str = "\u{E628}";
    pub const WAVE: &str = "\u{E802}";
    pub const UPDOWN: &str = "\u{E098}";
    pub const COMMAND: &str = "\u{E1C4}";
    pub const TABS: &str = "\u{E778}";
}

pub fn tap(r: &Response) -> bool {
    let c = r.clicked();
    if c {
        platform::haptic(0);
    }
    c
}

/// An icon glyph centered at `c`.
pub fn icon_at(ui: &Ui, c: Pos2, glyph: &str, size: f32, color: Color32, fill: bool) {
    ui.painter().text(c, Align2::CENTER_CENTER, glyph, icon_font(size, fill), color);
}

/// Just an icon, as a label.
pub fn icon(ui: &mut Ui, glyph: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    icon_at(ui, rect.center(), glyph, size, color, false);
    resp
}

/// A value that follows its target like a physical spring (mass 1). Interruptible: a new target keeps the speed.
/// Hearth's springs: fast (damping 0.6, stiffness 800), move (0.8, 380), enter (0.72, 320), release (0.52, 350).
pub fn spring(ctx: &egui::Context, id: egui::Id, target: f32, stiffness: f32, ratio: f32) -> f32 {
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.05);
    let (mut x, mut v) = ctx.data(|d| d.get_temp::<(f32, f32)>(id)).unwrap_or((target, 0.0));
    let c = 2.0 * ratio * stiffness.sqrt();
    for _ in 0..4 {
        let h = dt / 4.0;
        let a = -stiffness * (x - target) - c * v;
        v += a * h;
        x += v * h;
    }
    if (x - target).abs() < 0.0005 && v.abs() < 0.005 {
        x = target;
        v = 0.0;
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, (x, v)));
    x
}

/// The outline of a rounded rectangle, as points (for soft fills that stay inside the shape).
fn rounded_outline(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut pts = vec![];
    let corners = [
        (Pos2::new(rect.right() - r, rect.top() + r), -90.0f32),
        (Pos2::new(rect.right() - r, rect.bottom() - r), 0.0),
        (Pos2::new(rect.left() + r, rect.bottom() - r), 90.0),
        (Pos2::new(rect.left() + r, rect.top() + r), 180.0),
    ];
    for (c, start) in corners {
        for k in 0..=6 {
            let a = (start + 90.0 * k as f32 / 6.0).to_radians();
            pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    pts
}

/// The expressive press (as in Hearth): returns (offset, scale) for the pressed thing and paints the light under the
/// finger and the ring that spreads from the touch, inside `rect`'s rounded shape. Call it every frame.
pub fn elastic(ui: &Ui, resp: &Response, rect: Rect, radius: f32) -> (Vec2, f32) {
    let ctx = ui.ctx().clone();
    let id = resp.id.with("elastic");
    let down = resp.is_pointer_button_down_on();
    let pos = ctx.input(|i| i.pointer.interact_pos());
    let (mut tx, mut ty) = (0.0, 0.0);
    if let (true, Some(p)) = (down, pos) {
        tx = ((p.x - rect.center().x) / (rect.width() / 2.0)).clamp(-1.0, 1.0);
        ty = ((p.y - rect.center().y) / (rect.height() / 2.0)).clamp(-1.0, 1.0);
        ctx.data_mut(|d| d.insert_temp(id.with("at"), p));
    }
    // a new touch: the ring starts where the finger is
    let was = ctx.data(|d| d.get_temp::<bool>(id.with("down"))).unwrap_or(false);
    if down && !was {
        // read the clock first: egui's lock cannot be taken twice (input inside data_mut would hang)
        let now = ctx.input(|i| i.time);
        ctx.data_mut(|d| d.insert_temp(id.with("ring"), (pos.unwrap_or(rect.center()), now)));
    }
    ctx.data_mut(|d| d.insert_temp(id.with("down"), down));
    let (k, ratio) = if down { (550.0, 0.82) } else { (350.0, 0.52) };
    let dx = spring(&ctx, id.with("dx"), tx, k, ratio);
    let dy = spring(&ctx, id.with("dy"), ty, k, ratio);
    // a big card gives a little, a small button more
    let squeeze = (1.0 - 10.0 / rect.width().max(rect.height()).max(1.0)).clamp(0.94, 0.985);
    let s = spring(&ctx, id.with("s"), if down { squeeze } else { 1.0 }, if down { 1200.0 } else { 420.0 }, if down { 1.0 } else { 0.55 });
    let glow = ctx.animate_bool_with_time(id.with("glow"), down, if down { 0.12 } else { 0.28 });
    let p = pal();
    let light = if p.dark { Color32::WHITE } else { p.accent };
    let shown = Rect::from_center_size(rect.center() + vec2(dx, dy) * 2.0, rect.size() * s);
    if glow > 0.01 {
        // a fan from the finger to the outline: bright at the finger, nothing at the edge, never outside the shape
        let at = ctx.data(|d| d.get_temp::<Pos2>(id.with("at"))).unwrap_or(rect.center());
        let at = Pos2::new(at.x.clamp(shown.left() + 1.0, shown.right() - 1.0), at.y.clamp(shown.top() + 1.0, shown.bottom() - 1.0));
        let outline = rounded_outline(shown, radius * s);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(at, light.gamma_multiply((if p.dark { 0.2 } else { 0.16 }) * glow));
        for q in &outline {
            mesh.colored_vertex(*q, Color32::TRANSPARENT);
        }
        let n = outline.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
    }
    if let Some((o, t0)) = ctx.data(|d| d.get_temp::<(Pos2, f64)>(id.with("ring"))) {
        let t = ((ctx.input(|i| i.time) - t0) / 0.46) as f32;
        if t < 1.0 {
            let e = 1.0 - (1.0 - t).powi(3);
            let r = rect.width().max(rect.height()) * 1.0 * e;
            ui.painter_at(shown).circle_stroke(o, r, Stroke::new(r * 0.22 + 1.0, light.gamma_multiply(0.12 * (1.0 - t))));
            ctx.request_repaint();
        }
    }
    (vec2(dx, dy) * 2.0, s)
}

/// The expressive entrance (as in Hearth): the first time it is shown (for each `epoch`, e.g. each time a page
/// opens) the content rises a little and grows into place, one item after another (`order`, 38 ms apart).
pub fn arrive<R>(ui: &mut Ui, id: egui::Id, order: usize, f: impl FnOnce(&mut Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id.with("arrive"), || now + (order.min(8) as f64) * 0.038));
    let waiting = now < start;
    let target = if waiting { 0.0 } else { 1.0 };
    // starts at 0 the first time (the spring is created at its target, so set it to 0 first)
    if ctx.data(|d| d.get_temp::<(f32, f32)>(id.with("a"))).is_none() {
        ctx.data_mut(|d| d.insert_temp(id.with("a"), (0.0f32, 0.0f32)));
    }
    let a = spring(&ctx, id.with("a"), target, 320.0, 0.72);
    let o = ctx.animate_bool_with_time(id.with("o"), !waiting, 0.2);
    if waiting {
        ctx.request_repaint();
    }
    let rect = ui.available_rect_before_wrap();
    let scale = 0.94 + 0.06 * a;
    let origin = Pos2::new(rect.center().x, rect.top());
    let t = egui::emath::TSTransform::from_translation(vec2(0.0, (1.0 - a) * 26.0) + origin.to_vec2() * (1.0 - scale))
        * egui::emath::TSTransform::from_scaling(scale);
    ui.scope(|ui| {
        ui.multiply_opacity(o);
        ui.with_visual_transform(t, f).inner
    })
    .inner
}

/// Pressed look: a soft veil over the shape while the finger is on it.
fn veil(ui: &Ui, resp: &Response, rect: Rect, radius: f32, _color: Color32) {
    let _ = elastic(ui, resp, rect, radius);
}

/// Called at the start of every frame: cards count themselves, so they arrive in order; a new `epoch` (another page)
/// lets them arrive again.
pub fn begin_frame(ctx: &egui::Context, epoch: u64) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("card-order"), 0usize);
        d.insert_temp(egui::Id::new("card-epoch"), epoch);
    });
}

fn next_card(ctx: &egui::Context) -> (usize, u64) {
    ctx.data_mut(|d| {
        let n = *d.get_temp_mut_or_insert_with(egui::Id::new("card-order"), || 0usize);
        d.insert_temp(egui::Id::new("card-order"), n + 1);
        (n, d.get_temp::<u64>(egui::Id::new("card-epoch")).unwrap_or(0))
    })
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Filled,
    Tonal,
    Outlined,
    Text,
    Danger,
}

/// A pill-shaped button with an optional icon. `width` 0 = as wide as its content.
pub fn button(ui: &mut Ui, kind: Kind, glyph: Option<&str>, text: &str, width: f32) -> Response {
    button_sized(ui, kind, glyph, text, width, 44.0)
}

pub fn button_sized(ui: &mut Ui, kind: Kind, glyph: Option<&str>, text: &str, width: f32, height: f32) -> Response {
    let p = pal();
    let (fill, fg, stroke) = match kind {
        Kind::Filled => (p.accent, p.on_accent, Stroke::NONE),
        Kind::Tonal => (p.container, p.on_container, Stroke::NONE),
        Kind::Outlined => (Color32::TRANSPARENT, p.text, Stroke::new(1.0, p.line)),
        Kind::Text => (Color32::TRANSPARENT, p.accent, Stroke::NONE),
        Kind::Danger => (mix(p.card, p.bad, 0.16), p.bad, Stroke::NONE),
    };
    let font = medium(15.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let icon_w = if glyph.is_some() { 20.0 + if text.is_empty() { 0.0 } else { 8.0 } } else { 0.0 };
    let pad = if text.is_empty() { 12.0 } else if kind == Kind::Text { 12.0 } else { 20.0 };
    let content = icon_w + galley.size().x;
    let w = if width > 0.0 { width } else { content + pad * 2.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::click());
    let enabled = ui.is_enabled();
    let alpha = if enabled { 1.0 } else { 0.38 };
    // Expressive: pressing morphs the pill into a rounded square and back, and the button leans toward the finger
    let press = spring(ui.ctx(), resp.id.with("morph"), resp.is_pointer_button_down_on() as u8 as f32, 800.0, 0.6);
    let radius = egui::lerp(height / 2.0..=14.0, press.clamp(0.0, 1.2));
    let (off, s) = ui.ctx().data(|d| d.get_temp::<(Vec2, f32)>(resp.id.with("motion"))).unwrap_or((Vec2::ZERO, 1.0));
    let full = rect;
    let rect = Rect::from_center_size(rect.center() + off, rect.size() * s);
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, radius, fill.gamma_multiply(alpha));
    }
    if stroke != Stroke::NONE {
        ui.painter().rect_stroke(rect, radius, stroke, StrokeKind::Inside);
    }
    let m = elastic(ui, &resp, full, radius);
    ui.ctx().data_mut(|d| d.insert_temp(resp.id.with("motion"), m));
    let mut x = rect.center().x - content / 2.0;
    if let Some(g) = glyph {
        icon_at(ui, Pos2::new(x + 10.0, rect.center().y), g, 20.0, fg.gamma_multiply(alpha), false);
        x += icon_w;
    }
    ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, fg.gamma_multiply(alpha));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A round icon button (48 dp touch target).
pub fn icon_button(ui: &mut Ui, glyph: &str, color: Color32, bg: Option<Color32>) -> Response {
    icon_button_sized(ui, glyph, color, bg, 48.0, 22.0)
}

pub fn icon_button_sized(ui: &mut Ui, glyph: &str, color: Color32, bg: Option<Color32>, size: f32, glyph_size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if let Some(b) = bg {
        ui.painter().circle_filled(rect.center(), size / 2.0, b);
    }
    veil(ui, &resp, rect, size / 2.0, pal().text);
    icon_at(ui, rect.center(), glyph, glyph_size, color, false);
    resp
}

/// A flat card.
pub fn card<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    let (n, epoch) = next_card(ui.ctx());
    arrive(ui, egui::Id::new(("card", epoch, n)), n, |ui| plain_card(ui, f))
}

fn plain_card<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(pal().card)
        .corner_radius(r(24))
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            f(ui)
        })
        .inner
}

/// A card that is one big touch target. Its touch area lies under the card's own buttons: it is registered before
/// them, with the card's size from the last frame.
pub fn tap_card(ui: &mut Ui, id: egui::Id, f: impl FnOnce(&mut Ui)) -> Response {
    let (n, epoch) = next_card(ui.ctx());
    arrive(ui, egui::Id::new(("card", epoch, n)), n, |ui| tap_card_inner(ui, id, f))
}

fn tap_card_inner(ui: &mut Ui, id: egui::Id, f: impl FnOnce(&mut Ui)) -> Response {
    let last: Option<Rect> = ui.ctx().data(|d| d.get_temp(id));
    let resp = ui.interact(last.unwrap_or(Rect::NOTHING), id, Sense::click());
    // the card leans toward the finger and gives a little (its content with it), then springs back
    let (off, s) = ui.ctx().data(|d| d.get_temp::<(Vec2, f32)>(resp.id.with("motion"))).unwrap_or((Vec2::ZERO, 1.0));
    let center = last.map(|r| r.center()).unwrap_or_default();
    let t = egui::emath::TSTransform::from_translation(off + center.to_vec2() * (1.0 - s)) * egui::emath::TSTransform::from_scaling(s);
    let out = ui.with_visual_transform(t, |ui| {
        egui::Frame::new().fill(pal().card).corner_radius(r(24)).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            f(ui)
        })
    });
    let rect = out.inner.response.rect;
    ui.ctx().data_mut(|d| d.insert_temp(id, rect));
    if last != Some(rect) {
        ui.ctx().request_repaint();
    }
    let m = elastic(ui, &resp, rect, 24.0);
    ui.ctx().data_mut(|d| d.insert_temp(resp.id.with("motion"), m));
    resp
}

pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(text).font(medium(14.0)).color(pal().accent));
    });
    ui.add_space(2.0);
}

/// One row in a settings-like list: icon, title and line under it, something on the right.
pub fn row(ui: &mut Ui, glyph: &str, title: &str, sub: &str, right: impl FnOnce(&mut Ui)) {
    let p = pal();
    ui.horizontal(|ui| {
        ui.set_min_height(52.0);
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 20.0, p.raised);
        icon_at(ui, rect.center(), glyph, 20.0, p.text, false);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(if sub.is_empty() { 10.0 } else { 4.0 });
            ui.label(RichText::new(title).font(medium(16.0)).color(p.text));
            if !sub.is_empty() {
                ui.label(RichText::new(sub).size(13.5).color(p.muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
}

/// A top app bar: back arrow, title and a line under it, actions on the right.
pub fn top_bar(ui: &mut Ui, back: bool, title: &str, sub: Option<(&str, Color32)>, actions: impl FnOnce(&mut Ui)) -> bool {
    let p = pal();
    let mut go_back = false;
    ui.horizontal(|ui| {
        ui.set_min_height(56.0);
        if back {
            ui.add_space(-8.0);
            if tap(&icon_button(ui, ic::ARROW_LEFT, p.text, None)) {
                go_back = true;
            }
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(if sub.is_some() { 4.0 } else { 12.0 });
            ui.label(RichText::new(title).font(semibold(21.0)).color(p.text));
            if let Some((s, c)) = sub {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (d, _) = ui.allocate_exact_size(vec2(8.0, 16.0), Sense::hover());
                    ui.painter().circle_filled(d.center(), 4.0, c);
                    ui.label(RichText::new(s).size(13.5).color(p.muted));
                });
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
    });
    go_back
}

/// Material-style navigation bar at the bottom. Returns the chosen index.
pub fn nav_bar(ui: &mut Ui, items: &[(&str, &str)], selected: usize, bottom_inset: f32) -> Option<usize> {
    let p = pal();
    let h = 76.0;
    let full = ui.max_rect();
    let rect = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - h - bottom_inset), full.max);
    ui.painter().rect_filled(rect, 0.0, mix(p.bg, p.card, 0.85));
    let w = rect.width() / items.len() as f32;
    let mut picked = None;
    for (i, (glyph, label)) in items.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(rect.left() + w * i as f32, rect.top()), vec2(w, h));
        let resp = ui.interact(cell, ui.scope_id().with(("nav", i)), Sense::click());
        let on = i == selected;
        let t = ui.ctx().animate_bool_with_time(ui.scope_id().with(("nav-on", i)), on, 0.18);
        // overshoots a little on the way in (eased spring)
        let o = { let x = t; 1.0 + 2.2 * (x - 1.0).powi(3) + 1.2 * (x - 1.0).powi(2) };
        let pill = Rect::from_center_size(Pos2::new(cell.center().x, cell.top() + 26.0), vec2(36.0 + 28.0 * o.max(0.0), 32.0));
        if t > 0.0 {
            ui.painter().rect_filled(pill, 16.0, p.container.gamma_multiply(t));
        }
        icon_at(ui, pill.center(), glyph, 22.0, if on { p.on_container } else { p.muted }, on);
        ui.painter().text(Pos2::new(cell.center().x, cell.top() + 54.0), Align2::CENTER_CENTER, *label, medium(12.5), if on { p.text } else { p.muted });
        if tap(&resp) && !on {
            picked = Some(i);
        }
    }
    picked
}

/// An inset two-or-three-way switch with icons.
pub fn segmented(ui: &mut Ui, items: &[(&str, &str)], selected: &mut usize) -> bool {
    let p = pal();
    let h = 44.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(rect, h / 2.0, p.raised);
    let w = (rect.width() - 8.0) / items.len() as f32;
    // one animation per control: keyed by its labels
    let key = ui.scope_id().with(("seg", items.iter().map(|i| i.1).collect::<Vec<_>>()));
    let anim = ui.ctx().animate_value_with_time(key, *selected as f32, 0.18);
    let thumb = Rect::from_min_size(Pos2::new(rect.left() + 4.0 + w * anim, rect.top() + 4.0), vec2(w, h - 8.0));
    ui.painter().rect_filled(thumb, (h - 8.0) / 2.0, p.accent);
    let mut changed = false;
    for (i, (glyph, label)) in items.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(rect.left() + 4.0 + w * i as f32, rect.top() + 4.0), vec2(w, h - 8.0));
        let resp = ui.interact(cell, key.with(i), Sense::click());
        let on = i == *selected;
        let fg = if on { p.on_accent } else { p.muted };
        let galley = ui.painter().layout_no_wrap(label.to_string(), medium(14.0), fg);
        let content = 18.0 + 6.0 + galley.size().x;
        let x = cell.center().x - content / 2.0;
        icon_at(ui, Pos2::new(x + 9.0, cell.center().y), glyph, 18.0, fg, on);
        ui.painter().galley(Pos2::new(x + 24.0, cell.center().y - galley.size().y / 2.0), galley, fg);
        if tap(&resp) && !on {
            *selected = i;
            changed = true;
        }
    }
    changed
}

/// A small rounded chip (keys, workspaces).
pub fn chip(ui: &mut Ui, glyph: Option<&str>, text: &str, on: bool) -> Response {
    let p = pal();
    let font = if text.chars().count() <= 3 { medium(15.0) } else { medium(14.0) };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, if on { p.on_accent } else { p.text });
    let iw = if glyph.is_some() { if text.is_empty() { 20.0 } else { 26.0 } } else { 0.0 };
    let w = (galley.size().x + iw + 28.0).max(48.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
    ui.painter().rect_filled(rect, 14.0, if on { p.accent } else { p.raised });
    veil(ui, &resp, rect, 14.0, p.text);
    let content = iw + galley.size().x;
    let mut x = rect.center().x - content / 2.0;
    if let Some(g) = glyph {
        icon_at(ui, Pos2::new(x + 10.0, rect.center().y), g, 19.0, if on { p.on_accent } else { p.text }, false);
        x += iw;
    }
    ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, if on { p.on_accent } else { p.text });
    resp
}

/// A text field in a rounded, tonal box.
pub fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let p = pal();
    egui::Frame::new()
        .fill(p.raised)
        .corner_radius(r(16))
        .inner_margin(egui::Margin::symmetric(14, 2))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .vertical_align(egui::Align::Center)
                    .min_size(vec2(0.0, 48.0))
                    .hint_text(RichText::new(hint).color(p.muted))
                    .frame(egui::Frame::NONE)
                    .font(FontId::proportional(16.0))
                    .desired_width(width - 28.0),
            )
        })
        .inner
}

pub fn field_multi(ui: &mut Ui, text: &mut String, hint: &str, rows: usize) -> Response {
    let p = pal();
    egui::Frame::new()
        .fill(p.raised)
        .corner_radius(r(16))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(text)
                    .hint_text(RichText::new(hint).color(p.muted))
                    .frame(egui::Frame::NONE)
                    .font(FontId::proportional(16.0))
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner
}

/// Six boxes for the pairing code; typing goes into an invisible field over them. True when six digits are in.
pub fn code_boxes(ui: &mut Ui, code: &mut String) -> bool {
    let p = pal();
    let gap = 8.0;
    let w = ((ui.available_width() - gap * 5.0) / 6.0).min(52.0);
    let total = w * 6.0 + gap * 5.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
    let left = rect.center().x - total / 2.0;
    // the field that takes the keyboard, painted invisibly over the boxes
    let edit_rect = Rect::from_min_size(Pos2::new(left, rect.top()), vec2(total, 64.0));
    ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
    ui.visuals_mut().selection.bg_fill = Color32::TRANSPARENT;
    let resp = ui.put(
        edit_rect,
        egui::TextEdit::singleline(code).frame(egui::Frame::NONE).text_color(Color32::TRANSPARENT).font(FontId::monospace(1.0)).char_limit(6),
    );
    code.retain(|c| c.is_ascii_digit());
    code.truncate(6);
    if !resp.has_focus() && code.len() < 6 {
        resp.request_focus();
    }
    let focused = resp.has_focus();
    let blink = (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0;
    for k in 0..6 {
        let b = Rect::from_min_size(Pos2::new(left + k as f32 * (w + gap), rect.top() + 2.0), vec2(w, 60.0));
        let current = k == code.len() && focused;
        ui.painter().rect_filled(b, 14.0, p.raised);
        if current {
            ui.painter().rect_stroke(b, 14.0, Stroke::new(2.0, p.accent), StrokeKind::Inside);
            if blink {
                ui.painter().rect_filled(Rect::from_center_size(b.center(), vec2(2.0, 26.0)), 1.0, p.accent);
            }
        }
        if let Some(c) = code.chars().nth(k) {
            ui.painter().text(b.center(), Align2::CENTER_CENTER, c, FontId::monospace(28.0), p.text);
        }
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    code.len() == 6
}

/// A ring gauge with the number inside.
pub fn ring(ui: &mut Ui, frac: f32, color: Color32, size: f32, big: &str, small: &str) {
    let p = pal();
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let c = rect.center();
    let rad = size / 2.0 - 6.0;
    let stroke = 9.0;
    let n = 64;
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=n)
            .map(|i| {
                let a = from + (to - from) * i as f32 / n as f32;
                Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin())
            })
            .collect()
    };
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    ui.painter().add(egui::Shape::line(arc(start, start + sweep), Stroke::new(stroke, p.raised)));
    let f = ui.ctx().animate_value_with_time(ui.scope_id().with(("ring", small, rect.left() as i32, rect.top() as i32)), frac.clamp(0.0, 1.0), 0.4);
    if f > 0.005 {
        ui.painter().add(egui::Shape::line(arc(start, start + sweep * f), Stroke::new(stroke, color)));
        for pt in [arc(start, start)[0], *arc(start, start + sweep * f).last().unwrap()] {
            ui.painter().circle_filled(pt, stroke / 2.0, color);
        }
    }
    ui.painter().text(Pos2::new(c.x, c.y - 4.0), Align2::CENTER_CENTER, big, semibold(size * 0.2), p.text);
    ui.painter().text(Pos2::new(c.x, c.y + size * 0.16), Align2::CENTER_CENTER, small, FontId::proportional(12.5), p.muted);
}

/// A small line chart of the last minute.
pub fn spark(ui: &mut Ui, hist: &std::collections::VecDeque<f32>, color: Color32, max: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    if hist.len() < 2 {
        return;
    }
    let n = 60usize;
    let pts: Vec<Pos2> = hist
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.right() - (hist.len() - 1 - i) as f32 * rect.width() / (n - 1) as f32;
            let y = rect.bottom() - 2.0 - (v / max.max(1e-6)).clamp(0.0, 1.0) * (rect.height() - 6.0);
            Pos2::new(x, y)
        })
        .collect();
    // soft fill under the line, as thin vertical strips (works for any shape)
    for w in pts.windows(2) {
        let quad = vec![w[0], w[1], Pos2::new(w[1].x, rect.bottom()), Pos2::new(w[0].x, rect.bottom())];
        painter.add(egui::Shape::convex_polygon(quad, color.gamma_multiply(0.14), Stroke::NONE));
    }
    painter.add(egui::Shape::line(pts, Stroke::new(2.5, color)));
}

/// A thin progress bar.
pub fn bar(ui: &mut Ui, frac: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(rect, 4.0, pal().raised);
    let mut f = rect;
    f.set_width((rect.width() * frac.clamp(0.0, 1.0)).max(8.0));
    ui.painter().rect_filled(f, 4.0, color);
}

/// A centered message with a big icon (empty states, errors).
pub fn empty_state(ui: &mut Ui, glyph: &str, title: &str, text: &str, color: Color32) {
    let p = pal();
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        let (rect, _) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 48.0, mix(p.card, color, 0.18));
        icon_at(ui, rect.center(), glyph, 44.0, color, false);
        ui.add_space(14.0);
        ui.label(RichText::new(title).font(semibold(20.0)).color(p.text));
        ui.add_space(2.0);
        ui.label(RichText::new(text).size(15.0).color(p.muted));
        ui.add_space(10.0);
    });
}

/// A centered dialog over a dimmed screen. Returns Some(true) for the main button, Some(false) to cancel.
pub fn dialog(ctx: &egui::Context, glyph: &str, title: &str, text: &str, yes: &str, danger: bool) -> Option<bool> {
    let p = pal();
    let mut out = None;
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::unique("dialog-scrim")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(120));
        if resp.clicked() {
            out = Some(false);
        }
    });
    egui::Area::new(egui::Id::unique("dialog")).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
        egui::Frame::new().fill(mix(p.card, p.raised, 0.4)).corner_radius(r(28)).inner_margin(egui::Margin::same(24)).show(ui, |ui| {
            ui.set_width((screen.width() - 64.0).min(360.0));
            ui.vertical_centered(|ui| {
                icon(ui, glyph, 26.0, if danger { p.bad } else { p.accent });
                ui.add_space(4.0);
                ui.label(RichText::new(title).font(semibold(20.0)).color(p.text));
                ui.label(RichText::new(text).size(15.0).color(p.muted));
            });
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tap(&button(ui, if danger { Kind::Danger } else { Kind::Filled }, None, yes, 0.0)) {
                    out = Some(true);
                }
                if tap(&button(ui, Kind::Text, None, "Cancel", 0.0)) {
                    out = Some(false);
                }
            });
        });
    });
    out
}

/// A message at the bottom for a few seconds.
pub fn snackbar(ctx: &egui::Context, text: &str, bottom: f32) {
    let p = pal();
    egui::Area::new(egui::Id::unique("snackbar")).order(egui::Order::Tooltip).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -bottom - 16.0)).show(ctx, |ui| {
        egui::Frame::new().fill(if p.dark { mix(p.text, p.bg, 0.1) } else { mix(p.text, p.bg, 0.15) }).corner_radius(r(12)).inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
            ui.set_max_width(ctx.content_rect().width() - 48.0);
            ui.label(RichText::new(text).size(15.0).color(p.bg));
        });
    });
}

pub fn spinner(ui: &mut Ui, size: f32) {
    ui.add(egui::Spinner::new().size(size).color(pal().accent));
}
