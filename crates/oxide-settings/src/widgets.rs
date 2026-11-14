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
    ""
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

/// A group of rows: one flat container with hairline dividers. The title is plain sentence case above it.
pub fn card<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    if !title.is_empty() {
        ui.add_space(10.0);
        ui.label(RichText::new(title.to_uppercase()).family(medium()).color(MUTED()).size(11.5).extra_letter_spacing(1.1));
        ui.add_space(4.0);
    } else {
        ui.add_space(6.0);
    }
    egui::Frame::new()
        .fill(CARD())
        .stroke(Stroke::new(1.0, BORDER()))
        .corner_radius(radius(8))
        .inner_margin(egui::Margin::symmetric(14, 2))
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
    let min_h = if sub.is_empty() { 44.0 } else { 52.0 };
    let r = ui.horizontal(|ui| {
        ui.set_min_height(min_h);
        if !name.is_empty() {
            let icon = icon.unwrap_or_else(|| auto_icon(name));
            ui.vertical(|ui| {
                ui.add_space((min_h - 22.0) / 2.0);
                if icon.is_empty() {
                    ui.add_space(22.0);
                } else {
                    icon_tile(ui, icon, 22.0);
                }
            });
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.add_space(if sub.is_empty() { 11.0 } else { 8.0 });
                ui.label(name);
                if !sub.is_empty() {
                    ui.label(RichText::new(sub).small().color(MUTED()));
                }
            });
        }
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
    if !tip.is_empty() {
        r.response.on_hover_text(tip);
    }
    if !last {
        let y = ui.cursor().top();
        let x = ui.max_rect().x_range();
        ui.painter().hline(x, y, Stroke::new(1.0, BORDER()));
    }
}

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
        ui.add(egui::Slider::new(value, range).suffix(suffix.to_string()).trailing_fill(true));
    });
}

pub fn text_row(ui: &mut Ui, name: &str, help: &str, last: bool, value: &mut String, hint: &str) {
    row(ui, name, help, last, |ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(200.0).hint_text(hint.to_string()));
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
        // segmented control in a dark inset; laid out right to left, so reversed
        egui::Frame::new().fill(DEEP()).stroke(Stroke::new(1.0, BORDER())).corner_radius(radius(8)).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (opt, label) in options.iter().rev() {
                let selected = value == opt;
                let btn = egui::Button::new(RichText::new(*label).color(if selected { ON_ACCENT() } else { MUTED() }))
                    .fill(if selected { ACCENT() } else { egui::Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .corner_radius(radius(6));
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
        if ui.add(egui::TextEdit::singleline(&mut text).font(egui::TextStyle::Monospace).desired_width(if alpha { 112.0 } else { 92.0 }).clip_text(false)).changed() {
            *value = text;
        }
        let mut c = parse_hex(value);
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
    ui.add(egui::Button::new(label).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, BORDER())))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(ON_ACCENT()).family(medium())).fill(ACCENT()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
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
    let n = 48;
    let seg = rect.width() / n as f32;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let c = match colors.len() {
            0 => Color32::WHITE,
            1 => colors[0],
            len => {
                let s = t * (len - 1) as f32;
                let k = (s.floor() as usize).min(len - 2);
                lerp_color(colors[k], colors[k + 1], s - k as f32)
            }
        };
        let r = Rect::from_min_size(Pos2::new(rect.left() + i as f32 * seg, rect.top()), vec2(seg + 0.6, height));
        ui.painter().rect_filled(r, radius(0), c);
    }
    ui.painter().rect_stroke(rect, radius(4), Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
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
        let r = ui.add(egui::Slider::new(value, range).suffix(suffix.to_string()).fixed_decimals(0).trailing_fill(true));
        if r.drag_stopped() || (r.changed() && !r.dragged() && !r.has_focus()) {
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
