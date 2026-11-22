//! Building blocks: cards, setting rows, animated switch, key chips.
use eframe::egui::{self, vec2, Align, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui};

use crate::theme::*;

pub fn parse_hex(s: &str) -> Color32 {
    match oxide_config::parse_color(s) {
        Some([r, g, b, a]) => Color32::from_rgba_unmultiplied(
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
            (a * 255.0).round() as u8,
        ),
        None => Color32::WHITE,
    }
}

pub fn to_hex(c: Color32, alpha: bool) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if alpha {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

/// Icons are one color (see DESIGN.md): no rainbow.
pub fn tint_for(_icon: &str) -> Color32 {
    MUTED()
}

/// A plain icon in the first column of a row (no tile, no tint).
pub fn icon_tile(ui: &mut Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    crate::icons::paint(ui, name, Rect::from_center_size(rect.center(), vec2(size * 0.62, size * 0.62)), MUTED());
}

/// Pick an icon from the words in a setting's name.
pub fn auto_icon(name: &str) -> &'static str {
    let n = name.to_lowercase();
    let table: &[(&str, &str)] = &[
        ("wi-fi", "wifi"), ("network", "wifi"), ("hidden", "eye-off-outline"), ("password", "key-variant"),
        ("bluetooth", "bluetooth"), ("visible", "eye-outline"), ("pairing", "bluetooth-connect"),
        ("microphone", "microphone"), ("volume", "volume-high"), ("mute", "volume-off"), ("sound", "volume-high"), ("output", "speaker"), ("input", "microphone"), ("profile", "tune-variant"),
        ("brightness", "brightness-6"), ("scale", "magnify"), ("refresh", "refresh"), ("width", "arrow-expand-horizontal"), ("height", "arrow-expand-horizontal"), ("position", "monitor-multiple"), ("screen", "monitor"),
        ("battery", "battery-high"), ("power", "lightning-bolt"),
        ("gap", "vector-arrange-below"), ("border", "border-all-variant"), ("rounded", "rounded-corner"), ("opacity", "opacity"), ("dim", "weather-night"), ("blur", "blur"), ("shadow", "box-shadow"),
        ("color", "palette-outline"), ("gradient", "gradient-horizontal"), ("angle", "format-rotate-90"),
        ("layout", "view-dashboard-outline"), ("dwindle", "view-dashboard-outline"), ("master", "view-dashboard-outline"), ("split", "view-dashboard-outline"), ("floating", "application-outline"),
        ("focus", "cursor-default-click-outline"), ("scroll", "arrow-up-down"), ("workspace", "view-grid-outline"),
        ("animate", "animation-play-outline"), ("window", "application-outline"), ("fade", "opacity"), ("curve", "chart-bell-curve"), ("duration", "timer-outline"), ("style", "shape-outline"),
        ("keyboard", "keyboard-outline"), ("layout", "keyboard-outline"), ("key", "keyboard-outline"), ("repeat", "refresh"), ("variant", "keyboard-variant"), ("extra", "tune-variant"),
        ("touchpad", "gesture-tap"), ("natural", "gesture-swipe-horizontal"), ("tap", "gesture-tap"), ("typing", "keyboard-outline"), ("pointer", "cursor-default"), ("accel", "speedometer"),
        ("xwayland", "console-line"), ("commands", "console-line"), ("settings", "tune-variant"), ("permission", "shield-lock-outline"), ("thinking", "brain"), ("shell", "puzzle-outline"), ("mouse", "mouse"), ("screen", "monitor"), ("look at", "monitor-screenshot"), ("retry", "refresh"), ("instructions", "text-box-edit-outline"), ("address", "web"), ("startup", "play"), ("enabled", "check"), ("name", "text-box-outline"), ("location", "folder-open-outline"), ("reload", "refresh"), ("hyprland", "import"), ("defaults", "restore"),
    ];
    for (k, v) in table {
        if n.contains(k) {
            return v;
        }
    }
    // every row has an icon: the words start at the same place everywhere
    "tune-variant"
}

/// Page title: plain text, left aligned. The icon argument is kept for callers but not drawn.
pub fn page_header(ui: &mut Ui, _icon: &str, title: &str, subtitle: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).heading());
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).color(MUTED()));
    }
    ui.add_space(12.0);
}

/// Time since the current page appeared, in seconds (set by the app when the page changes).
fn page_age(ctx: &egui::Context) -> f32 {
    let t0: f64 = ctx.data(|d| d.get_temp(egui::Id::new("page_t0"))).unwrap_or(0.0);
    (ctx.input(|i| i.time) - t0) as f32
}

pub fn start_page(ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("page_t0"), now));
}

/// Called before a page is drawn: its cards count themselves, so they can arrive in order.
pub fn begin_page(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("card_n"), 0usize));
}

/// The expressive entrance (Hearth's springIn): when a page opens its cards rise a little and grow into place, one
/// after another (38 ms apart). Only in the Material Expressive style.
fn arrive<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    if !expressive() {
        return f(ui);
    }
    let ctx = ui.ctx().clone();
    let n = ctx.data_mut(|d| {
        let n = *d.get_temp_mut_or_insert_with(egui::Id::new("card_n"), || 0usize);
        d.insert_temp(egui::Id::new("card_n"), n + 1);
        n
    });
    let t0: f64 = ctx.data(|d| d.get_temp(egui::Id::new("page_t0"))).unwrap_or(0.0);
    let id = egui::Id::new(("arrive", (t0 * 1000.0) as i64, n));
    let waiting = page_age(&ctx) < n.min(8) as f32 * 0.038;
    if ctx.data(|d| d.get_temp::<(f32, f32)>(id)).is_none() {
        ctx.data_mut(|d| d.insert_temp(id, (0.0f32, 0.0f32)));
    }
    let a = spring_with(&ctx, id, if waiting { 0.0 } else { 1.0 }, 320.0, 0.72);
    let o = ctx.animate_bool_with_time(id.with("o"), !waiting, 0.2);
    if waiting {
        ctx.request_repaint();
    }
    let rect = ui.available_rect_before_wrap();
    let scale = 0.94 + 0.06 * a;
    let origin = egui::pos2(rect.center().x, rect.top());
    let t = egui::emath::TSTransform::from_translation(vec2(0.0, (1.0 - a) * 26.0) + origin.to_vec2() * (1.0 - scale))
        * egui::emath::TSTransform::from_scaling(scale);
    ui.scope(|ui| {
        ui.multiply_opacity(o);
        ui.with_visual_transform(t, f).inner
    })
    .inner
}

/// A heading above picture cards: Hearth's colored sentence-case title in Expressive, small capitals otherwise.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(if expressive() { 16.0 } else { 10.0 });
    if expressive() {
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(RichText::new(text).family(medium()).color(ACCENT()).size(14.5));
        });
    } else {
        ui.label(RichText::new(text.to_uppercase()).family(medium()).color(MUTED()).size(11.5).extra_letter_spacing(1.1));
    }
    ui.add_space(if expressive() { 6.0 } else { 4.0 });
}

/// A group of rows: one flat container with hairline dividers. The title is plain sentence case above it.
pub fn card<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    arrive(ui, |ui| plain_card(ui, title, body))
}

fn plain_card<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    let ex = expressive();
    if !title.is_empty() {
        ui.add_space(if ex { 16.0 } else { 10.0 });
        if ex {
            // Hearth's section title: the primary color, sentence case, a little larger
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new(title).family(medium()).color(ACCENT()).size(14.5));
            });
        } else {
            ui.label(RichText::new(title.to_uppercase()).family(medium()).color(MUTED()).size(11.5).extra_letter_spacing(1.1));
        }
        ui.add_space(if ex { 6.0 } else { 4.0 });
    } else {
        ui.add_space(6.0);
    }
    egui::Frame::new()
        .fill(if ex { CARD_HOVER() } else { CARD() })
        .stroke(if ex { Stroke::NONE } else { Stroke::new(1.0, BORDER()) })
        .corner_radius(if ex { egui::CornerRadius::same(20) } else { radius(12) })
        .inner_margin(egui::Margin::symmetric(if ex { 18 } else { 14 }, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            body(ui)
        })
        .inner
}

/// One setting row: icon tile, name (and optional status line) on the left, control on the right.
/// `tip` is shown when hovering the row, so the page stays clean.
pub fn row_full(ui: &mut Ui, icon: Option<&str>, name: &str, sub: &str, tip: &str, last: bool, control: impl FnOnce(&mut Ui)) {
    // every row has the same height and everything in it is centered on one line: icon, words and control
    let h = if sub.is_empty() { ROW_H } else { ROW_SUB_H };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    if !name.is_empty() {
        let icon = icon.unwrap_or_else(|| auto_icon(name));
        // the icon is centered in the row; the words are centered on it too (one line, or two with the status line under)
        if !icon.is_empty() {
            let mut left = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
            icon_tile(&mut left, icon, 22.0);
        }
        let body = egui::TextStyle::Body.resolve(ui.style());
        let small = egui::TextStyle::Small.resolve(ui.style());
        let g_name = ui.painter().layout_no_wrap(name.to_string(), body, TEXT());
        let g_sub = (!sub.is_empty()).then(|| ui.painter().layout_no_wrap(sub.to_string(), small, MUTED()));
        let total = g_name.size().y + g_sub.as_ref().map_or(0.0, |g| 2.0 + g.size().y);
        let x = rect.left() + 34.0;
        let mut y = rect.center().y - total / 2.0;
        let name_h = g_name.size().y;
        ui.painter().galley(Pos2::new(x, y), g_name, TEXT());
        y += name_h + 2.0;
        if let Some(g) = g_sub {
            ui.painter().galley(Pos2::new(x, y), g, MUTED());
        }
    }
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    right.spacing_mut().item_spacing.x = 10.0;
    right.spacing_mut().interact_size.y = CONTROL_H;
    control(&mut right);
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    if !last {
        let y = rect.bottom();
        ui.painter().hline(rect.x_range(), y, Stroke::new(1.0, BORDER()));
    }
}

/// The height of a row, of a row with a second line, and of the controls in it (fields, buttons, swatches).
pub const ROW_H: f32 = 46.0;
pub const ROW_SUB_H: f32 = 56.0;
pub const CONTROL_H: f32 = 30.0;
/// The number next to a slider: always this wide, so all of them end at the same line.
const NUMBER_W: f32 = 68.0;

/// Setting row; `help` appears as a tooltip.
pub fn row(ui: &mut Ui, name: &str, help: &str, last: bool, control: impl FnOnce(&mut Ui)) {
    row_full(ui, None, name, "", help, last, control);
}

/// Row with a visible status line (device state, signal...).
pub fn row_sub(ui: &mut Ui, icon: &str, name: &str, sub: &str, last: bool, control: impl FnOnce(&mut Ui)) {
    row_full(ui, Some(icon), name, sub, "", last, control);
}

/// Animated on/off switch.
pub fn switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    if expressive() {
        return m3_switch(ui, on);
    }
    // a full pill with a round knob, whatever the theme's roundness
    let size = vec2(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = tween(ui.ctx(), resp.id, if *on { 1.0 } else { 0.0 }, 0.14);
    let hover = if resp.hovered() { 0.12 } else { 0.0 };
    let bg = lerp_color(lerp_color(FIELD(), MUTED(), 0.35 + hover), ACCENT(), t);
    let pill = rect.height() / 2.0;
    ui.painter().rect_filled(rect, pill, bg);
    if t < 1.0 {
        ui.painter().rect_stroke(rect, pill, Stroke::new(1.0, lerp_color(BORDER(), ACCENT(), t)), egui::StrokeKind::Inside);
    }
    let r = 8.0;
    let x = egui::lerp((rect.left() + 3.0 + r)..=(rect.right() - 3.0 - r), ease_out(t));
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), r, lerp_color(TEXT(), ON_ACCENT(), t));
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), pill + 2.0, Stroke::new(1.5, ACCENT()), egui::StrokeKind::Outside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The Material 3 switch: an outlined track with a small knob when off; on, the knob grows and shows a check;
/// pressing makes it bigger still. The knob springs a little past its place.
fn m3_switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = tween(ui.ctx(), resp.id, if *on { 1.0 } else { 0.0 }, 0.16);
    let pressed = resp.is_pointer_button_down_on();
    let target = if pressed { 28.0 } else if *on { 24.0 } else { 16.0 };
    let size = ui.ctx().animate_value_with_time(resp.id.with("knob"), target, 0.14);
    let p = ui.painter();
    let pill = rect.height() / 2.0;
    let track = lerp_color(lerp_color(FIELD(), TEXT(), if resp.hovered() { 0.06 } else { 0.0 }), ACCENT(), t);
    p.rect_filled(rect, pill, track);
    if t < 0.99 {
        p.rect_stroke(rect, pill, Stroke::new(2.0, lerp_color(lerp_color(BORDER(), MUTED(), 0.6), ACCENT(), t)), egui::StrokeKind::Inside);
    }
    // a slight overshoot on the way: an eased spring
    let s = {
        let x = t;
        let c = 1.6;
        1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
    };
    let left = rect.left() + pill;
    let right = rect.right() - pill;
    let cx = left + (right - left) * s;
    let c = Pos2::new(cx, rect.center().y);
    p.circle_filled(c, size / 2.0, lerp_color(lerp_color(BORDER(), MUTED(), 0.8), ON_ACCENT(), t));
    if t > 0.5 {
        // the check, drawn
        let a = (t - 0.5) * 2.0;
        let k = 4.0 * a;
        let pts = vec![Pos2::new(c.x - k, c.y), Pos2::new(c.x - k * 0.3, c.y + k * 0.7), Pos2::new(c.x + k, c.y - k * 0.7)];
        p.add(egui::Shape::line(pts, Stroke::new(2.0, ACCENT())));
    }
    if resp.has_focus() {
        p.rect_stroke(rect.expand(2.0), pill + 2.0, Stroke::new(1.5, ACCENT()), egui::StrokeKind::Outside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn toggle_row(ui: &mut Ui, name: &str, help: &str, last: bool, value: &mut bool) {
    row(ui, name, help, last, |ui| {
        switch(ui, value);
    });
}

pub fn slider_row<N: egui::emath::Numeric>(
    ui: &mut Ui,
    name: &str,
    help: &str,
    last: bool,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    suffix: &str,
) {
    row(ui, name, help, last, |ui| {
        // the number sits in a dark inset box, in the mono font
        ui.visuals_mut().widgets.inactive.weak_bg_fill = DEEP();
        ui.visuals_mut().widgets.inactive.bg_fill = DEEP();
        ui.visuals_mut().widgets.hovered.weak_bg_fill = DEEP();
        ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::monospace(13.0));
        // right to left: the track, then the number before it
        track(ui, value, range.clone(), 200.0);
        number(ui, value, range, suffix);
    });
}

/// The number next to a slider: drag it or type into it.
fn number<N: egui::emath::Numeric>(ui: &mut Ui, value: &mut N, range: std::ops::RangeInclusive<N>, suffix: &str) -> egui::Response {
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    let mut d = egui::DragValue::new(value).range(range).suffix(suffix.to_string()).speed((hi - lo) / 300.0);
    if !N::INTEGRAL && hi - lo <= 2.0 {
        d = d.fixed_decimals(2);
    }
    ui.add_sized([NUMBER_W, CONTROL_H], d)
}

/// A slider track in the Material 3 expressive style: a thick rounded track that stops with a small gap on both
/// sides of a solid vertical handle, so nothing shows past or through the handle.
pub fn track<N: egui::emath::Numeric>(ui: &mut Ui, value: &mut N, range: std::ops::RangeInclusive<N>, width: f32) -> egui::Response {
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click_and_drag());
    // the track runs to both ends of its rectangle (so it ends on the same line as every other control); the handle's centre
    // stays 2 px inside
    let inner = rect.shrink2(vec2(2.0, 0.0));
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on()) {
        let t = ((p.x - inner.left()) / inner.width()).clamp(0.0, 1.0) as f64;
        let mut v = lo + t * (hi - lo);
        if N::INTEGRAL {
            v = v.round();
        }
        if (v - value.to_f64()).abs() > f64::EPSILON {
            *value = N::from_f64(v);
            resp.mark_changed();
        }
    }
    if resp.has_focus() {
        let step = if N::INTEGRAL { 1.0 } else { (hi - lo) / 100.0 };
        let delta = ui.input(|i| (i.key_pressed(egui::Key::ArrowRight) as i32 - i.key_pressed(egui::Key::ArrowLeft) as i32) as f64);
        if delta != 0.0 {
            *value = N::from_f64((value.to_f64() + delta * step).clamp(lo, hi));
            resp.mark_changed();
        }
    }
    let t = if hi > lo { ((value.to_f64() - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let active = resp.dragged() || resp.is_pointer_button_down_on();
    let grow = tween(ui.ctx(), resp.id.with("grip"), if active { 1.0 } else if resp.hovered() { 0.5 } else { 0.0 }, 0.12);
    let x = inner.left() + inner.width() * t;
    let cy = rect.center().y;
    let th = 8.0;
    let gap = 5.0;
    let p = ui.painter();
    // filled part up to the handle, the rest after it
    if x - gap > inner.left() + 1.0 {
        let a = Rect::from_min_max(Pos2::new(rect.left(), cy - th / 2.0), Pos2::new(x - gap, cy + th / 2.0));
        p.rect_filled(a, egui::CornerRadius { nw: 4, sw: 4, ne: 2, se: 2 }, ACCENT());
    }
    if x + gap < inner.right() - 1.0 {
        let b = Rect::from_min_max(Pos2::new(x + gap, cy - th / 2.0), Pos2::new(rect.right(), cy + th / 2.0));
        p.rect_filled(b, egui::CornerRadius { nw: 2, sw: 2, ne: 4, se: 4 }, lerp_color(FIELD(), MUTED(), 0.35));
    }
    let hw = 4.0 - grow * 1.0;
    let hh = 24.0 + grow * 2.0;
    p.rect_filled(Rect::from_center_size(Pos2::new(x, cy), vec2(hw, hh)), hw / 2.0, ACCENT());
    if resp.has_focus() {
        p.rect_stroke(rect.expand(2.0), 6.0, Stroke::new(1.5, ACCENT().gamma_multiply(0.6)), egui::StrokeKind::Outside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn text_row(ui: &mut Ui, name: &str, help: &str, last: bool, value: &mut String, hint: &str) {
    row(ui, name, help, last, |ui| {
        ui.add_sized([200.0, CONTROL_H], egui::TextEdit::singleline(value).vertical_align(Align::Center).hint_text(hint.to_string()));
    });
}

pub fn choice_row<T: PartialEq + Clone>(
    ui: &mut Ui,
    name: &str,
    help: &str,
    last: bool,
    value: &mut T,
    options: &[(T, &str)],
) {
    row(ui, name, help, last, |ui| {
        if expressive() {
            // Hearth's filter chips: 8 px corners that round into a pill when chosen, a tonal fill and a check, a pop
            ui.spacing_mut().item_spacing.x = 8.0;
            for (opt, label) in options.iter().rev() {
                let selected = value == opt;
                if m3_chip(ui, label, selected).clicked() {
                    *value = opt.clone();
                }
            }
            return;
        }
        // segmented control in a dark inset; laid out right to left, so reversed
        egui::Frame::new().fill(DEEP()).stroke(Stroke::new(1.0, BORDER())).corner_radius(radius(8)).inner_margin(egui::Margin::same(2)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().interact_size.y = CONTROL_H - 6.0;
            for (opt, label) in options.iter().rev() {
                let selected = value == opt;
                let btn = egui::Button::new(RichText::new(*label).color(if selected { ON_ACCENT() } else { MUTED() }))
                    .fill(if selected { ACCENT() } else { egui::Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .min_size(vec2(0.0, CONTROL_H - 6.0))
                    .corner_radius(radius(8));
                if ui.add(btn).clicked() {
                    *value = opt.clone();
                }
            }
        });
    });
}

/// Color picker row editing a "#rrggbb[aa]" string.
pub fn color_row(ui: &mut Ui, name: &str, help: &str, last: bool, value: &mut String, alpha: bool) {
    row(ui, name, help, last, |ui| {
        let mut text = value.clone();
        if ui.add_sized([if alpha { 112.0 } else { 96.0 }, CONTROL_H], egui::TextEdit::singleline(&mut text).font(egui::TextStyle::Monospace).vertical_align(Align::Center).clip_text(false)).changed() {
            *value = text;
        }
        let mut c = parse_hex(value);
        ui.spacing_mut().interact_size = vec2(CONTROL_H + 8.0, CONTROL_H);
        let changed = if alpha {
            ui.color_edit_button_srgba(&mut c).changed()
        } else {
            let mut rgb = [c.r(), c.g(), c.b()];
            let ch = ui.color_edit_button_srgb(&mut rgb).changed();
            c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            ch
        };
        if changed {
            *value = to_hex(c, alpha);
        }
    });
}

/// A keyboard key drawn as a small rounded chip.
pub fn key_chip(ui: &mut Ui, label: &str, strong: bool) {
    let font = egui::FontId::monospace(12.5);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, if strong { ON_ACCENT() } else { TEXT() });
    let size = vec2(galley.size().x + 16.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let fill = if strong { ACCENT() } else { CARD_HOVER() };
    ui.painter().rect_filled(rect, radius(4), fill);
    ui.painter().galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, if strong { ON_ACCENT() } else { TEXT() });
}

/// Small colored status dot.
pub fn dot(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 3.5, color);
}

/// Subtle button used for secondary actions.
pub fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    if expressive() {
        return m3_button(ui, label, CONTAINER(), ON_CONTAINER());
    }
    ui.add(egui::Button::new(label).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, BORDER())))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    if expressive() {
        return m3_button(ui, label, ACCENT(), ON_ACCENT());
    }
    ui.add(egui::Button::new(RichText::new(label).color(ON_ACCENT()).family(medium())).fill(ACCENT()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Expressive button: a pill that squashes into a rounded square while pressed and springs back.
fn m3_button(ui: &mut Ui, label: &str, fill: Color32, fg: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::new(14.0, medium()), fg);
    let size = vec2(galley.size().x + 36.0, 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let press = spring_with(ui.ctx(), resp.id.with("press"), resp.is_pointer_button_down_on() as u8 as f32, 800.0, 0.6).clamp(0.0, 1.3);
    let hover = tween(ui.ctx(), resp.id.with("hover"), resp.hovered() as u8 as f32, 0.12);
    let r = egui::lerp(rect.height() / 2.0..=9.0, press);
    // it leans toward the pointer while pressed and springs back past its rest (Hearth's elastic press)
    let down = resp.is_pointer_button_down_on();
    let (mut tx, mut ty) = (0.0, 0.0);
    if let (true, Some(p)) = (down, ui.ctx().input(|i| i.pointer.interact_pos())) {
        tx = ((p.x - rect.center().x) / (rect.width() / 2.0)).clamp(-1.0, 1.0);
        ty = ((p.y - rect.center().y) / (rect.height() / 2.0)).clamp(-1.0, 1.0);
    }
    let (k, z) = if down { (550.0, 0.82) } else { (350.0, 0.52) };
    let dx = spring_with(ui.ctx(), resp.id.with("dx"), tx, k, z);
    let dy = spring_with(ui.ctx(), resp.id.with("dy"), ty, k, z);
    let shown = rect.shrink2(vec2(press * 3.0, press * 1.5)).translate(vec2(dx, dy) * 2.0);
    ui.painter().rect_filled(shown, r, lerp_color(fill, fg, hover * 0.08 + press * 0.06));
    if press > 0.01 {
        // the light under the pointer
        if let Some(p) = ui.ctx().input(|i| i.pointer.interact_pos()).filter(|_| down) {
            let light = if crate::theme::is_light() { ACCENT() } else { Color32::WHITE };
            for (k, a) in [(1.0f32, 0.05f32), (0.6, 0.05), (0.3, 0.05)] {
                ui.painter_at(shown).circle_filled(p, shown.width() * 0.5 * k, light.gamma_multiply(a * press));
            }
        }
    }
    ui.painter().galley(shown.center() - galley.size() / 2.0, galley, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[allow(dead_code)]
pub fn rect_contains(r: Rect, p: Pos2) -> bool {
    r.contains(p)
}

/// Wi-Fi style signal bars (0..=100).
pub fn signal_icon(ui: &mut Ui, percent: u8, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(20.0, 16.0), Sense::hover());
    let lit = ((percent as f32 / 100.0) * 4.0).ceil() as usize;
    for i in 0..4 {
        let h = 4.0 + 4.0 * i as f32;
        let x = rect.left() + i as f32 * 5.0;
        let r = Rect::from_min_max(Pos2::new(x, rect.bottom() - h), Pos2::new(x + 3.5, rect.bottom()));
        ui.painter().rect_filled(r, radius(1), if i < lit { color } else { BORDER() });
    }
}

/// A horizontal strip showing a color gradient.
pub fn gradient_strip(ui: &mut Ui, colors: &[Color32], height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let color_at = |x: f32| {
        let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        match colors.len() {
            0 => Color32::WHITE,
            1 => colors[0],
            len => {
                let s = t * (len - 1) as f32;
                let k = (s.floor() as usize).min(len - 2);
                lerp_color(colors[k], colors[k + 1], s - k as f32)
            }
        }
    };
    let r = (height / 2.0).min(6.0);
    // vertical strips in the gradient's colors, cut to the rounded shape at both ends
    let mut strips = egui::Mesh::default();
    // every half pixel along the rounded ends, coarser in between
    let mut xs: Vec<f32> = vec![];
    let mut x = rect.left();
    while x < rect.right() {
        xs.push(x);
        x += if x < rect.left() + r || x > rect.right() - r - 8.0 { 0.5 } else { 8.0 };
    }
    xs.push(rect.right());
    for (i, &x) in xs.iter().enumerate() {
        let inset = if x < rect.left() + r {
            r - (r * r - (rect.left() + r - x).powi(2)).max(0.0).sqrt()
        } else if x > rect.right() - r {
            r - (r * r - (x - (rect.right() - r)).powi(2)).max(0.0).sqrt()
        } else {
            0.0
        };
        let c = color_at(x);
        strips.colored_vertex(Pos2::new(x, rect.top() + inset), c);
        strips.colored_vertex(Pos2::new(x, rect.bottom() - inset), c);
        if i > 0 {
            let b = (i * 2) as u32;
            strips.add_triangle(b - 2, b - 1, b);
            strips.add_triangle(b - 1, b, b + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(strips));
    ui.painter().rect_stroke(rect, r, Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
}

/// A slider row that reports when the user finished dragging (so commands run once, not per pixel).
/// Returns `Some(final value)` when a change was released.
pub fn commit_slider_row(
    ui: &mut Ui,
    name: &str,
    help: &str,
    last: bool,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) -> Option<f32> {
    let mut committed = None;
    row(ui, name, help, last, |ui| {
        let t = track(ui, value, range.clone(), 200.0);
        let n = number(ui, value, range, suffix);
        if t.drag_stopped() || (t.changed() && !t.dragged()) || n.drag_stopped() || (n.changed() && !n.dragged() && !n.has_focus()) || n.lost_focus() {
            committed = Some(*value);
        }
    });
    committed
}

/// Small pill-shaped tag, e.g. "Connected".
pub fn tag(ui: &mut Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::monospace(11.5), color);
    let size = galley.size() + vec2(14.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, radius(3), color.linear_multiply(0.16));
    ui.painter().galley(Pos2::new(rect.left() + 7.0, rect.center().y - galley.size().y / 2.0), galley, color);
}

/// A Material 3 filter chip: square-ish when off, a pill with a check and a tonal fill when chosen; it pops a little
/// when it gets chosen and leans toward the pointer while pressed.
pub fn m3_chip(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let font = egui::FontId::new(14.0, medium());
    let fg = if selected { ON_CONTAINER() } else { TEXT() };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, fg);
    let check = ui.ctx().animate_bool_with_time(ui.next_auto_id().with("chk"), selected, 0.16);
    let w = galley.size().x + 28.0 + 22.0 * check;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
    let id = resp.id;
    let corner = spring_with(ui.ctx(), id.with("corner"), if selected { 18.0 } else { 8.0 }, 800.0, 0.6).clamp(0.0, 18.0);
    let pop = spring_with(ui.ctx(), id.with("pop"), 1.0, 700.0, 0.4);
    if selected && !ui.ctx().data(|d| d.get_temp::<bool>(id.with("was"))).unwrap_or(false) {
        ui.ctx().data_mut(|d| d.insert_temp(id.with("pop"), (0.9f32, 0.0f32)));
    }
    let was = selected;
    ui.ctx().data_mut(|d| d.insert_temp(id.with("was"), was));
    let press = spring_with(ui.ctx(), id.with("press"), resp.is_pointer_button_down_on() as u8 as f32, 800.0, 0.6).clamp(0.0, 1.3);
    let shown = Rect::from_center_size(rect.center(), rect.size() * pop * (1.0 - 0.03 * press));
    let hov = tween(ui.ctx(), id.with("hov"), resp.hovered() as u8 as f32, 0.12);
    let p = ui.painter();
    if selected {
        p.rect_filled(shown, corner, lerp_color(CONTAINER(), ON_CONTAINER(), 0.05 * press));
    } else {
        p.rect_filled(shown, corner, TEXT().gamma_multiply(0.06 * hov + 0.06 * press));
        p.rect_stroke(shown, corner, Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
    }
    let mut x = shown.center().x - (galley.size().x + 22.0 * check) / 2.0;
    if check > 0.01 {
        let c = Pos2::new(x + 7.0, shown.center().y);
        let k = 4.5 * check;
        p.add(egui::Shape::line(vec![Pos2::new(c.x - k, c.y), Pos2::new(c.x - k * 0.3, c.y + k * 0.7), Pos2::new(c.x + k, c.y - k * 0.7)], Stroke::new(2.0, ON_CONTAINER().gamma_multiply(check))));
        x += 22.0 * check;
    }
    p.galley(Pos2::new(x, shown.center().y - galley.size().y / 2.0), galley, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
