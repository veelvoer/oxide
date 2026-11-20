//! Drawing for a round screen: everything is laid out on the circle. Lists are cut to the circle (rows narrow toward the top
//! and bottom edge and fade out), they scroll with a finger or the rotary bezel and glide on after a flick; arcs run along
//! the rim. Nothing here uses egui's scroll area: one small scroller drives it, so the feel is the same everywhere.
use std::time::{Duration, Instant};

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui};

use crate::theme::*;

/// The circle of the screen.
#[derive(Clone, Copy)]
pub struct Geo {
    pub c: Pos2,
    pub r: f32,
}

impl Geo {
    pub fn new(full: Rect) -> Self {
        Self { c: full.center(), r: full.width().min(full.height()) / 2.0 }
    }

    /// Half the width of the circle at height `y`, with `margin` kept free at the rim.
    pub fn half(&self, y: f32, margin: f32) -> f32 {
        let dy = (y - self.c.y).abs();
        ((self.r - margin).powi(2) - dy * dy).max(0.0).sqrt()
    }

    pub fn at(&self, radius: f32, angle: f32) -> Pos2 {
        self.c + vec2(angle.cos(), angle.sin()) * radius
    }
}

/// A line along an arc (angles in radians, 0 = right, clockwise on screen).
pub fn arc(ui: &Ui, center: Pos2, radius: f32, from: f32, to: f32, stroke: Stroke) {
    let steps = (((to - from).abs() * radius) / 2.5).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            center + vec2(a.cos(), a.sin()) * radius
        })
        .collect();
    ui.painter().add(egui::Shape::line(pts, stroke));
}

/// An icon whose ink (not its box) is centered on `center`: each glyph has its own optical offset (see icon_metrics.rs).
pub fn icon(ui: &Ui, center: Pos2, glyph: &str, size: f32, color: Color32) {
    let cp = glyph.chars().next().map_or(0, |c| c as u32);
    let (ox, oy) = crate::icon_metrics::ICON_OFFSETS.iter().find(|(c, _, _)| *c == cp).map_or((0.0, 0.0), |(_, x, y)| (*x, *y));
    ui.painter().text(center - vec2(ox, oy) * size, Align2::CENTER_CENTER, glyph, icon_font(size, false), color);
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ------------------------------------------------------------------ cells

/// One entry of a list.
#[derive(Clone)]
pub enum Cell {
    Title(String),
    /// small muted text, wrapped
    Note(String),
    Row { icon: &'static str, title: String, sub: String, caret: bool },
    Toggle { title: String, on: bool },
    /// `-  value  +`: zone 1 is minus, zone 2 plus
    Stepper { title: String, value: String },
    /// side by side: the zone is the option
    Choice { options: Vec<String>, selected: usize },
    Button { icon: Option<&'static str>, text: String, filled: bool },
    /// round color dots: the zone is the dot
    Swatches { colors: Vec<Color32>, selected: Option<usize> },
    /// a PC: zone 0 opens it, zone 3 is its wake button
    Pc { name: String, sub: String, dot: Color32, wake_accent: bool },
    Back,
    Spacer(f32),
}

impl Cell {
    fn height(&self, ui: &Ui, width: f32) -> f32 {
        match self {
            Cell::Title(_) => 26.0,
            Cell::Note(t) => ui.painter().layout(t.clone(), FontId::proportional(11.0), Color32::WHITE, width - 10.0).size().y + 10.0,
            Cell::Row { sub, .. } => {
                if sub.is_empty() {
                    44.0
                } else {
                    52.0
                }
            }
            Cell::Toggle { .. } => 44.0,
            Cell::Stepper { .. } => 56.0,
            Cell::Choice { .. } => 40.0,
            Cell::Button { .. } => 42.0,
            Cell::Swatches { .. } => 42.0,
            Cell::Pc { .. } => 56.0,
            Cell::Back => 36.0,
            Cell::Spacer(h) => *h,
        }
    }

    fn tappable(&self) -> bool {
        !matches!(self, Cell::Title(_) | Cell::Note(_) | Cell::Spacer(_))
    }
}

/// What was tapped: the cell and the part of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub index: usize,
    pub zone: usize,
}

// ------------------------------------------------------------------ the scroller

/// A finger-driven scroll position with momentum and a soft stop at the ends.
#[derive(Default)]
pub struct Scroller {
    pub off: f32,
    vel: f32,
    press: Option<Pos2>,
    pressed_at: Option<Instant>,
    claimed: bool,
    last_y: f32,
    moved: f32,
}

impl Scroller {
    pub fn reset(&mut self) {
        *self = Scroller::default();
    }
}

fn zone_of(cell: &Cell, rect: Rect, p: Pos2, r: f32) -> usize {
    match cell {
        Cell::Stepper { .. } => {
            if p.x < rect.center().x - rect.width() * 0.14 {
                1
            } else if p.x > rect.center().x + rect.width() * 0.14 {
                2
            } else {
                0
            }
        }
        Cell::Choice { options, .. } => (((p.x - rect.left()) / rect.width() * options.len() as f32) as usize).min(options.len().saturating_sub(1)),
        Cell::Swatches { colors, .. } => (((p.x - rect.left()) / rect.width() * colors.len() as f32) as usize).min(colors.len().saturating_sub(1)),
        Cell::Pc { .. } => {
            if rect.width() > r * 1.5 && p.x > rect.right() - 46.0 {
                3
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Lay the cells out on the circle inside `area`, scroll them, draw them and return what was tapped.
pub fn list(ui: &mut Ui, geo: Geo, area: Rect, sc: &mut Scroller, cells: &[Cell]) -> Option<Hit> {
    let p = pal();
    let ctx = ui.ctx().clone();
    // heights first (they depend on the width a little: notes wrap)
    let base_w = geo.half(geo.c.y - geo.r * 0.3, 8.0) * 2.0;
    let heights: Vec<f32> = cells.iter().map(|c| c.height(ui, base_w)).collect();
    let gap = 6.0;
    let top_pad = geo.r * 0.24;
    let bottom_pad = geo.r * 0.30;
    let content = top_pad + heights.iter().sum::<f32>() + gap * cells.len().saturating_sub(1) as f32 + bottom_pad;
    let max_off = (content - area.height()).max(0.0);

    // ---- input
    let (pressed, down, released, pos, scroll) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.latest_pos(), i.smooth_scroll_delta.y));
    let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
    let mut tapped = None;
    if pressed {
        if let Some(pt) = pos.filter(|q| area.contains(*q)) {
            sc.press = Some(pt);
            sc.pressed_at = Some(Instant::now());
            sc.claimed = false;
            sc.moved = 0.0;
            sc.last_y = pt.y;
            sc.vel = 0.0;
        }
    }
    if down {
        if let (Some(start), Some(pt)) = (sc.press, pos) {
            let dy = pt.y - sc.last_y;
            sc.last_y = pt.y;
            sc.moved = sc.moved.max((pt - start).length());
            if !sc.claimed && (pt - start).y.abs() > 7.0 && (pt - start).y.abs() > (pt - start).x.abs() * 0.8 {
                sc.claimed = true;
            }
            if sc.claimed {
                // beyond the ends the content follows only partly: it is held back
                let over = if sc.off < 0.0 { -sc.off } else if sc.off > max_off { sc.off - max_off } else { 0.0 };
                let k = 1.0 / (1.0 + over / 40.0);
                sc.off -= dy * k;
                sc.vel = sc.vel * 0.6 + (-dy / dt) * 0.4;
                ctx.request_repaint();
            }
        }
    }
    if released {
        if let Some(start) = sc.press.take() {
            if !sc.claimed && sc.moved < 9.0 {
                tapped = Some(start);
            }
        }
        sc.claimed = false;
    }
    // the rotary bezel (and a mouse wheel)
    if scroll.abs() > 0.1 {
        sc.off -= scroll;
        sc.vel = 0.0;
        ctx.request_repaint();
    }
    // momentum and the soft stop
    if !down {
        if sc.off < 0.0 || sc.off > max_off {
            let target = sc.off.clamp(0.0, max_off);
            sc.off += (target - sc.off) * (1.0 - (-dt * 14.0).exp());
            sc.vel = 0.0;
            if (sc.off - target).abs() < 0.2 {
                sc.off = target;
            } else {
                ctx.request_repaint();
            }
        } else if sc.vel.abs() > 8.0 {
            sc.off += sc.vel * dt;
            sc.vel *= (-dt * 4.2).exp();
            ctx.request_repaint();
        } else {
            sc.vel = 0.0;
        }
    }

    // ---- draw
    let mut y = area.top() + top_pad - sc.off;
    let mut hit = None;
    let held = if down && !sc.claimed { sc.press } else { None };
    for (i, cell) in cells.iter().enumerate() {
        let h = heights[i];
        let (yc, far) = (y + h / 2.0, (y + h / 2.0 - geo.c.y).abs() + h / 2.0);
        let half = geo.half(geo.c.y + (far * (yc - geo.c.y).signum()), 5.0).max(0.0);
        // cells follow the circle, but never get too narrow for what is in them; notes keep the width they were measured at
        let width = if matches!(cell, Cell::Note(_)) { base_w } else { (half * 2.0).max(base_w * 0.8).min(base_w + 40.0) };
        let rect = Rect::from_center_size(Pos2::new(geo.c.x, yc), vec2(width, h));
        let d = (yc - geo.c.y).abs() / geo.r;
        let alpha = 1.0 - smooth((d - 0.62) / 0.38);
        if alpha > 0.02 {
            let pressed_here = held.is_some_and(|hp| rect.contains(hp));
            draw_cell(ui, cell, rect, alpha, pressed_here, p, geo.r);
            if let Some(t) = tapped {
                if cell.tappable() && alpha > 0.35 && rect.contains(t) {
                    hit = Some(Hit { index: i, zone: zone_of(cell, rect, t, geo.r) });
                }
            }
        }
        y += h + gap;
    }
    hit
}

fn a(c: Color32, alpha: f32) -> Color32 {
    c.gamma_multiply(alpha)
}

/// Text that is shrunk (not cut) to fit.
fn fit_text(ui: &Ui, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32, max_w: f32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), color);
    if galley.size().x <= max_w || max_w < 20.0 {
        ui.painter().text(pos, align, text, font, color);
    } else {
        let scale = (max_w / galley.size().x).max(0.6);
        ui.painter().text(pos, align, text, FontId::new(font.size * scale, font.family), color);
    }
}

fn draw_cell(ui: &Ui, cell: &Cell, rect: Rect, alpha: f32, pressed: bool, p: Pal, radius: f32) {
    let sink = if pressed { 2.0 } else { 0.0 };
    let r = rect.shrink(sink);
    let pill = (r.height() / 2.0).min(30.0) as u8;
    let card = a(mix(p.card, p.text, if pressed { 0.1 } else { 0.0 }), alpha);
    let ink = a(p.text, alpha);
    let muted = a(p.muted, alpha);
    let accent = a(p.accent, alpha);
    match cell {
        Cell::Title(t) => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, t, semibold(14.0), ink);
        }
        Cell::Note(t) => {
            let g = ui.painter().layout(t.clone(), FontId::proportional(11.0), muted, rect.width() - 10.0);
            let pos = Pos2::new(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0);
            ui.painter().galley(pos, g, muted);
        }
        Cell::Row { icon, title, sub, caret } => {
            ui.painter().rect_filled(r, pill, card);
            self::icon(ui, Pos2::new(r.left() + 22.0, r.center().y), icon, 18.0, accent);
            let ty = if sub.is_empty() { r.center().y } else { r.center().y - 8.0 };
            let max_w = r.width() - 44.0 - if *caret { 36.0 } else { 14.0 };
            fit_text(ui, Pos2::new(r.left() + 42.0, ty), Align2::LEFT_CENTER, title, medium(13.0), ink, max_w);
            if !sub.is_empty() {
                fit_text(ui, Pos2::new(r.left() + 42.0, r.center().y + 9.0), Align2::LEFT_CENTER, sub, FontId::proportional(10.5), muted, max_w);
            }
            if *caret {
                self::icon(ui, Pos2::new(r.right() - 16.0, r.center().y), "\u{E13A}", 14.0, muted);
            }
        }
        Cell::Toggle { title, on } => {
            ui.painter().rect_filled(r, pill, card);
            fit_text(ui, Pos2::new(r.left() + 18.0, r.center().y), Align2::LEFT_CENTER, title, medium(12.5), ink, r.width() - 80.0);
            let sw = Rect::from_center_size(Pos2::new(r.right() - 34.0, r.center().y), vec2(38.0, 22.0));
            ui.painter().rect_filled(sw, 11, mix(a(p.raised, alpha), accent, if *on { 1.0 } else { 0.0 }));
            let kx = if *on { sw.right() - 11.0 } else { sw.left() + 11.0 };
            ui.painter().circle_filled(Pos2::new(kx, sw.center().y), 8.0, if *on { a(p.on_accent, alpha) } else { muted });
        }
        Cell::Stepper { title, value } => {
            ui.painter().rect_filled(r, pill, card);
            ui.painter().text(Pos2::new(r.center().x, r.top() + 13.0), Align2::CENTER_CENTER, title, FontId::proportional(10.5), muted);
            ui.painter().text(Pos2::new(r.center().x, r.bottom() - 18.0), Align2::CENTER_CENTER, value, semibold(15.0), ink);
            for (dx, g) in [(-1.0, "\u{E32A}"), (1.0, "\u{E3D4}")] {
                let c = Pos2::new(r.center().x + dx * (r.width() / 2.0 - 30.0), r.bottom() - 18.0);
                ui.painter().circle_filled(c, 14.0, a(p.raised, alpha));
                self::icon(ui, c, g, 14.0, ink);
            }
        }
        Cell::Choice { options, selected } => {
            ui.painter().rect_filled(r, pill, card);
            let w = r.width() / options.len() as f32;
            for (k, o) in options.iter().enumerate() {
                let seg = Rect::from_min_size(Pos2::new(r.left() + w * k as f32, r.top()), vec2(w, r.height())).shrink(3.0);
                let on = k == *selected;
                if on {
                    ui.painter().rect_filled(seg, (seg.height() / 2.0) as u8, accent);
                }
                fit_text(ui, seg.center(), Align2::CENTER_CENTER, o, medium(12.0), if on { a(p.on_accent, alpha) } else { ink }, seg.width() - 4.0);
            }
        }
        Cell::Button { icon, text, filled } => {
            let (fill, fg) = if *filled { (accent, a(p.on_accent, alpha)) } else { (a(mix(p.raised, p.text, if pressed { 0.1 } else { 0.0 }), alpha), ink) };
            ui.painter().rect_filled(r, pill, fill);
            match icon {
                Some(g) => {
                    // icon and text as one group, centered together
                    let tw = ui.painter().layout_no_wrap(text.clone(), medium(13.0), fg).size().x;
                    let (iw, gap) = (16.0, 8.0);
                    let left = r.center().x - (iw + gap + tw) / 2.0;
                    self::icon(ui, Pos2::new(left + iw / 2.0, r.center().y), g, 16.0, fg);
                    ui.painter().text(Pos2::new(left + iw + gap, r.center().y), Align2::LEFT_CENTER, text, medium(13.0), fg);
                }
                None => {
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, medium(13.0), fg);
                }
            }
        }
        Cell::Swatches { colors, selected } => {
            let w = r.width() / colors.len() as f32;
            for (k, c) in colors.iter().enumerate() {
                let ctr = Pos2::new(r.left() + w * (k as f32 + 0.5), r.center().y);
                ui.painter().circle_filled(ctr, 12.0, a(*c, alpha));
                if *selected == Some(k) {
                    ui.painter().circle_stroke(ctr, 16.0, Stroke::new(2.0, ink));
                }
            }
        }
        Cell::Pc { name, sub, dot, wake_accent } => {
            ui.painter().rect_filled(r, pill, card);
            self::icon(ui, Pos2::new(r.left() + 24.0, r.center().y), "\u{E32E}", 20.0, accent);
            ui.painter().circle_filled(Pos2::new(r.left() + 35.0, r.center().y + 9.0), 4.0, a(*dot, alpha));
            // a narrow cell (near the rim) has no room for the wake button
            let wide = r.width() > radius * 1.5;
            let max_w = r.width() - 46.0 - if wide { 52.0 } else { 14.0 };
            fit_text(ui, Pos2::new(r.left() + 46.0, r.center().y - 8.0), Align2::LEFT_CENTER, name, medium(13.5), ink, max_w);
            fit_text(ui, Pos2::new(r.left() + 46.0, r.center().y + 9.0), Align2::LEFT_CENTER, sub, FontId::proportional(10.5), muted, max_w);
            if wide {
                let wc = Pos2::new(r.right() - 26.0, r.center().y);
                ui.painter().circle_filled(wc, 18.0, a(p.raised, alpha));
                self::icon(ui, wc, "\u{E3DA}", 16.0, if *wake_accent { accent } else { muted });
            }
        }
        Cell::Back => {
            ui.painter().rect_filled(Rect::from_center_size(rect.center(), vec2(64.0, r.height())), pill, card);
            self::icon(ui, rect.center(), "\u{E058}", 16.0, muted);
        }
        Cell::Spacer(_) => {}
    }
}

// ------------------------------------------------------------------ small things

/// A round button with an icon (and an optional ring), for the controls page. Returns true when tapped.
pub fn round_button(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, center: Pos2, radius: f32, icon: &str, accent: bool) -> bool {
    let p = pal();
    let rect = Rect::from_center_size(center, vec2(radius * 2.0 + 6.0, radius * 2.0 + 6.0));
    let resp = ui.interact(rect, ui.make_persistent_id(id), Sense::click());
    let down = ui.ctx().animate_bool_with_time(resp.id.with("down"), resp.is_pointer_button_down_on(), 0.08);
    let rr = radius - down * 2.0;
    let (fill, fg) = if accent { (p.accent, p.on_accent) } else { (mix(p.card, p.text, 0.04 + down * 0.1), p.accent) };
    ui.painter().circle_filled(center, rr, mix(fill, p.text, if accent { down * 0.12 } else { 0.0 }));
    self::icon(ui, center, icon, rr * 0.9, fg);
    resp.clicked()
}

/// How often the screen is redrawn while something moves: 60 per second.
pub const FRAME: Duration = Duration::from_millis(16);
