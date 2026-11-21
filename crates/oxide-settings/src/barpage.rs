//! Bar editor: drag modules between the three lanes, tweak each one, and the shell redraws live
//! (the shell watches ~/.config/oxide/bar.json).
use std::cell::RefCell;

use eframe::egui::{self, vec2, Align2, Color32, FontId, Id, Rect, RichText, Sense, Stroke, Ui};
use serde_json::{json, Map, Value};

use crate::{icons, theme::*, widgets::*, App};

type Module = Map<String, Value>;

#[derive(Clone)]
struct BarCfg {
    position: String,
    style: String,
    height: i64,
    margin: i64,
    /// None follows the theme's roundness
    radius: Option<i64>,
    spacing: i64,
    pills: bool,
    opacity: f64,
    /// zero-bar: no permanent bar, shown while Super is held
    hidden: bool,
    /// edge HUD while Super is held
    hud: bool,
    lanes: [Vec<Module>; 3],
}

const LANE_NAMES: [&str; 3] = ["left", "center", "right"];
const LANE_TITLES: [&str; 3] = ["Left", "Center", "Right"];

enum Opt {
    Bool(&'static str, &'static str, bool),
    Choice(&'static str, &'static str, &'static [(&'static str, &'static str)], &'static str),
    Int(&'static str, &'static str, i64, i64, i64),
}

struct Def {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    about: &'static str,
    opts: &'static [Opt],
}

/// Offered by every module.
const SIZE: Opt = Opt::Int("scale", "Size", 70, 160, 100);

const DEFS: &[Def] = &[
    Def { id: "launcher", label: "Launcher", icon: "apps", about: "Opens the app launcher.", opts: &[] },
    Def { id: "workspaces", label: "Workspaces", icon: "view-grid-outline", about: "Workspace switcher.", opts: &[
        Opt::Choice("style", "Look", &[("numbers", "Numbers"), ("dots", "Dots"), ("buttons", "Buttons"), ("bars", "Bars"), ("scroll", "Scroll"), ("stretch", "Stretch")], "numbers"),
        Opt::Int("count", "Shown at once (bars, scroll, stretch)", 2, 12, 5),
        Opt::Bool("showEmpty", "Show empty workspaces", false),
        Opt::Int("highlightDelay", "Highlight delay (ms before it starts, 0 = at once)", 0, 1500, 0),
        Opt::Int("highlightSpeed", "Highlight speed (ms it takes, 0 = jumps)", 0, 1500, 380),
    ] },
    Def { id: "window", label: "Window title", icon: "application-outline", about: "Title of the focused window.", opts: &[
        Opt::Bool("showIcon", "Show app icon", true),
        Opt::Int("maxWidth", "Max width", 120, 600, 320),
    ] },
    Def { id: "clock", label: "Clock", icon: "timer-outline", about: "Time and date. Click for the calendar.", opts: &[
        Opt::Bool("format24", "24-hour time", true),
        Opt::Bool("seconds", "Show seconds", false),
        Opt::Bool("date", "Show date", true),
        Opt::Bool("weekday", "Show weekday", true),
    ] },
    Def { id: "media", label: "Media", icon: "music-note-outline", about: "Now playing, with controls.", opts: &[
        Opt::Int("maxWidth", "Max width", 100, 500, 260),
    ] },
    Def { id: "tray", label: "Tray", icon: "puzzle-outline", about: "Status icons from running apps.", opts: &[] },
    Def { id: "status", label: "Quick settings", icon: "tune-variant", about: "Wi-Fi, Bluetooth, volume and battery. Opens quick settings.", opts: &[
        Opt::Bool("wifi", "Wi-Fi", true),
        Opt::Bool("bluetooth", "Bluetooth", true),
        Opt::Bool("volume", "Volume", true),
        Opt::Bool("battery", "Battery", true),
        Opt::Bool("batteryPercent", "Battery percentage", true),
    ] },
    Def { id: "bell", label: "Notifications", icon: "bell-outline", about: "Notification count and do not disturb.", opts: &[] },
    Def { id: "assistant", label: "Assistant", icon: "robot-outline", about: "Opens the Oxide assistant.", opts: &[] },
    Def { id: "clipboard", label: "Clipboard", icon: "clipboard-text-outline", about: "Opens the clipboard history (Super+Shift+V).", opts: &[] },
    Def { id: "power", label: "Power", icon: "power", about: "Lock, log out, sleep, restart, shut down.", opts: &[] },
    Def { id: "cpu", label: "CPU", icon: "speedometer", about: "CPU load.", opts: &[] },
    Def { id: "memory", label: "Memory", icon: "chart-bell-curve", about: "Memory in use.", opts: &[] },
    Def { id: "spacer", label: "Spacer", icon: "arrow-expand-horizontal", about: "Empty space of a fixed width.", opts: &[
        Opt::Int("width", "Width", 4, 200, 24),
    ] },
    Def { id: "separator", label: "Separator", icon: "minus-circle-outline", about: "A thin vertical line.", opts: &[] },
];

fn def(id: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.id == id)
}

fn module(id: &str) -> Module {
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    m
}

fn mid(m: &Module) -> &str {
    m.get("id").and_then(|v| v.as_str()).unwrap_or("")
}

impl Default for BarCfg {
    fn default() -> Self {
        let l = |ids: &[&str]| ids.iter().map(|i| module(i)).collect::<Vec<_>>();
        BarCfg {
            position: "top".into(),
            style: "flat".into(),
            height: 36,
            margin: 6,
            radius: None,
            spacing: 6,
            pills: false,
            opacity: 0.96,
            hidden: false,
            hud: true,
            lanes: [
                l(&["launcher", "workspaces", "window"]),
                l(&["clock"]),
                l(&["tray", "status", "bell", "assistant"]),
            ],
        }
    }
}

fn path() -> std::path::PathBuf {
    oxide_config::Config::path().with_file_name("bar.json")
}

impl BarCfg {
    fn load() -> Self {
        let mut c = BarCfg::default();
        let Ok(text) = std::fs::read_to_string(path()) else { return c };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { return c };
        if let Some(s) = v["position"].as_str() { c.position = s.into(); }
        if let Some(s) = v["style"].as_str() { c.style = s.into(); }
        if let Some(n) = v["height"].as_i64() { c.height = n; }
        if let Some(n) = v["margin"].as_i64() { c.margin = n; }
        c.radius = v["radius"].as_i64().filter(|n| *n >= 0);
        if let Some(n) = v["spacing"].as_i64() { c.spacing = n; }
        c.pills = v["lanes"].as_str() == Some("pill");
        if let Some(n) = v["opacity"].as_f64() { c.opacity = n; }
        c.hidden = v["hidden"].as_bool().unwrap_or(false);
        c.hud = v["hud"].as_bool().unwrap_or(true);
        for (i, name) in LANE_NAMES.iter().enumerate() {
            if let Some(a) = v[*name].as_array() {
                c.lanes[i] = a.iter().filter_map(|m| m.as_object().cloned()).collect();
            }
        }
        c
    }

    fn save(&self) {
        let lane = |i: usize| Value::Array(self.lanes[i].iter().cloned().map(Value::Object).collect());
        let v = json!({
            "position": self.position, "style": self.style, "height": self.height, "margin": self.margin,
            "radius": self.radius.unwrap_or(-1), "spacing": self.spacing, "lanes": if self.pills { "pill" } else { "plain" }, "opacity": (self.opacity * 100.0).round() / 100.0,
            "hidden": self.hidden, "hud": self.hud,
            "left": lane(0), "center": lane(1), "right": lane(2),
        });
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}

#[derive(Clone, PartialEq)]
enum Src {
    New(&'static str),
    Slot(usize, usize),
}

struct State {
    cfg: BarCfg,
    sel: Option<(usize, usize)>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "", "Bar", "Drag modules to arrange the bar. Changes apply as you make them.");

    let mut st = STATE.with(|s| s.borrow_mut().take()).unwrap_or_else(|| State { cfg: BarCfg::load(), sel: None });
    let before = st.cfg.clone();
    let before_sel = st.sel;

    let own_shell = std::fs::read_link(oxide_config::active_shell_link())
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .map(|n| n == "oxide")
        .unwrap_or(true);
    if !own_shell {
        ui.label(RichText::new("The active shell is not the Oxide shell, so it ignores this layout. Pick Oxide on the Shells page.").color(WARN()));
    }

    preview_and_lanes(ui, &mut st);
    palette(ui, &mut st);
    selected_panel(ui, &mut st);
    bar_options(ui, &mut st.cfg);

    if st.cfg.lanes.iter().zip(before.lanes.iter()).any(|(a, b)| a != b)
        || st.cfg.position != before.position
        || st.cfg.style != before.style
        || st.cfg.height != before.height
        || st.cfg.margin != before.margin
        || st.cfg.radius != before.radius
        || st.cfg.spacing != before.spacing
        || st.cfg.pills != before.pills
        || st.cfg.hidden != before.hidden
        || st.cfg.hud != before.hud
        || (st.cfg.opacity - before.opacity).abs() > f64::EPSILON
    {
        st.cfg.save();
        app.ctx.request_repaint();
    }
    let _ = before_sel;

    ui.add_space(8.0);
    if ghost_button(ui, "Reset to default").clicked() {
        st.cfg = BarCfg::default();
        st.sel = None;
        st.cfg.save();
    }
    ui.add_space(16.0);
    STATE.with(|s| *s.borrow_mut() = Some(st));
}

fn chip_width(ui: &Ui, label: &str) -> f32 {
    ui.painter().layout_no_wrap(label.to_string(), FontId::new(13.0, medium()), TEXT()).size().x + 44.0
}

/// Drag sources do not wrap on their own, so split the chips into rows up front.
fn rows(ui: &Ui, labels: &[&str], avail: f32) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut used) = (0, 0.0);
    for (i, l) in labels.iter().enumerate() {
        let w = chip_width(ui, l) + 6.0;
        if used + w > avail && i > start {
            out.push(start..i);
            start = i;
            used = 0.0;
        }
        used += w;
    }
    out.push(start..labels.len());
    out
}

fn chip(ui: &mut Ui, d: Option<&Def>, id: &str, selected: bool) -> egui::Response {
    let label = d.map(|d| d.label).unwrap_or(id);
    let font = FontId::new(13.0, medium());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), TEXT());
    let size = vec2(galley.size().x + 44.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let fill = if selected { CARD_HOVER() } else if resp.hovered() { CARD_HOVER() } else { FIELD() };
    let stroke = if selected { Stroke::new(1.5, ACCENT()) } else { Stroke::new(1.0, BORDER()) };
    ui.painter().rect(rect, radius(4), fill, stroke, egui::StrokeKind::Inside);
    let ir = Rect::from_center_size(egui::pos2(rect.left() + 17.0, rect.center().y), vec2(15.0, 15.0));
    icons::paint(ui, d.map(|d| d.icon).unwrap_or("puzzle-outline"), ir, if selected { ACCENT() } else { MUTED() });
    ui.painter().text(egui::pos2(rect.left() + 30.0, rect.center().y), Align2::LEFT_CENTER, label, font, TEXT());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    resp
}

/// A thin sketch of the bar on top, with the three lanes as drop zones below it.
fn preview_and_lanes(ui: &mut Ui, st: &mut State) {
    let mut moved: Option<(Src, usize, usize)> = None;
    let mut select: Option<(usize, usize)> = None;
    let dragging = egui::DragAndDrop::has_payload_of_type::<Src>(ui.ctx());

    card(ui, "Layout", |ui| {
        ui.add_space(12.0);
        for lane in 0..3 {
            ui.label(RichText::new(LANE_TITLES[lane]).color(MUTED()).size(12.0));
            ui.add_space(3.0);
            let lane_id = Id::new(("bar-lane", lane));
            let zone_w = ui.available_width();
            let mut rects: Vec<Rect> = Vec::new();
            let (inner, dropped) = ui.dnd_drop_zone::<Src, ()>(
                egui::Frame::new().fill(BG()).stroke(Stroke::new(1.0, if dragging { ACCENT().gamma_multiply(0.6) } else { BORDER() })).corner_radius(radius(6)).inner_margin(egui::Margin::same(6)),
                |ui| {
                    ui.set_width(zone_w - 14.0);
                    ui.set_min_height(32.0);
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    if st.cfg.lanes[lane].is_empty() {
                        ui.label(RichText::new("Drop modules here").color(MUTED()));
                    }
                    let labels: Vec<&str> = st.cfg.lanes[lane].iter().map(|m| def(mid(m)).map(|d| d.label).unwrap_or(mid(m))).collect();
                    for r in rows(ui, &labels, zone_w - 20.0) {
                        ui.horizontal(|ui| {
                            for i in r {
                                let m = &st.cfg.lanes[lane][i];
                                let d = def(mid(m));
                                let id = lane_id.with(i);
                                let selected = st.sel == Some((lane, i));
                                let r = ui.dnd_drag_source(id, Src::Slot(lane, i), |ui| {
                                    let resp = chip(ui, d, mid(m), selected);
                                    if resp.clicked() {
                                        select = Some((lane, i));
                                    }
                                });
                                rects.push(r.response.rect);
                            }
                        });
                    }
                },
            );
            let zone = inner.response.rect;
            if let Some(src) = dropped {
                let px = ui.ctx().pointer_interact_pos().map(|p| p.x).unwrap_or(zone.right());
                let py = ui.ctx().pointer_interact_pos().map(|p| p.y).unwrap_or(zone.top());
                // insert before the first chip whose centre is after the pointer (same row)
                let idx = rects
                    .iter()
                    .position(|r| (py < r.bottom() && px < r.center().x) || py < r.top())
                    .unwrap_or(rects.len());
                moved = Some(((*src).clone(), lane, idx));
            } else if dragging && inner.response.contains_pointer() {
                if let Some(p) = ui.ctx().pointer_interact_pos() {
                    let idx = rects.iter().position(|r| (p.y < r.bottom() && p.x < r.center().x) || p.y < r.top()).unwrap_or(rects.len());
                    let x = match rects.get(idx) {
                        Some(r) => r.left() - 3.0,
                        None => rects.last().map(|r| r.right() + 3.0).unwrap_or(zone.left() + 8.0),
                    };
                    let y = rects.get(idx).or(rects.last()).map(|r| r.y_range()).unwrap_or(zone.y_range());
                    ui.painter().line_segment([egui::pos2(x, y.min), egui::pos2(x, y.max)], Stroke::new(2.0, ACCENT()));
                }
            }
            ui.add_space(8.0);
        }
    });

    if let Some(s) = select {
        st.sel = if st.sel == Some(s) { None } else { Some(s) };
    }
    if let Some((src, lane, idx)) = moved {
        match src {
            Src::New(id) => {
                let i = idx.min(st.cfg.lanes[lane].len());
                st.cfg.lanes[lane].insert(i, module(id));
                st.sel = Some((lane, i));
            }
            Src::Slot(sl, si) => {
                if si < st.cfg.lanes[sl].len() {
                    let m = st.cfg.lanes[sl].remove(si);
                    let mut i = idx;
                    if sl == lane && si < idx {
                        i -= 1;
                    }
                    let i = i.min(st.cfg.lanes[lane].len());
                    st.cfg.lanes[lane].insert(i, m);
                    st.sel = Some((lane, i));
                }
            }
        }
    }
}

/// Everything that can be added. Dropping a placed module here removes it.
fn palette(ui: &mut Ui, st: &mut State) {
    let mut remove: Option<(usize, usize)> = None;
    card(ui, "Modules", |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Drag a module into the bar. Drag one back here to remove it.").color(MUTED()).size(12.0));
        ui.add_space(6.0);
        let zone_w = ui.available_width();
        let (_, dropped) = ui.dnd_drop_zone::<Src, ()>(egui::Frame::new().inner_margin(egui::Margin::same(2)), |ui| {
            ui.set_width(zone_w - 4.0);
            ui.set_min_height(40.0);
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let labels: Vec<&str> = DEFS.iter().map(|d| d.label).collect();
            for r in rows(ui, &labels, zone_w - 10.0) {
                ui.horizontal(|ui| {
                    for d in &DEFS[r] {
                        ui.dnd_drag_source(Id::new(("bar-pal", d.id)), Src::New(d.id), |ui| {
                            chip(ui, Some(d), d.id, false).on_hover_text(d.about);
                        });
                    }
                });
            }
        });
        if let Some(src) = dropped {
            if let Src::Slot(l, i) = *src {
                remove = Some((l, i));
            }
        }
        ui.add_space(8.0);
    });
    if let Some((l, i)) = remove {
        if i < st.cfg.lanes[l].len() {
            st.cfg.lanes[l].remove(i);
        }
        st.sel = None;
    }
}

fn selected_panel(ui: &mut Ui, st: &mut State) {
    let Some((lane, i)) = st.sel else { return };
    if i >= st.cfg.lanes[lane].len() {
        st.sel = None;
        return;
    }
    let id = mid(&st.cfg.lanes[lane][i]).to_string();
    let Some(d) = def(&id) else { return };
    let mut remove = false;
    let mut shift: i32 = 0;
    card(ui, d.label, |ui| {
        row_full(ui, None, "", d.about, "", d.opts.is_empty(), |ui| {
            if ui.add(egui::Button::new(RichText::new("Remove").color(BAD())).fill(FIELD()).corner_radius(radius(4))).clicked() {
                remove = true;
            }
            if ui.add(egui::Button::new("Move right").fill(FIELD()).corner_radius(radius(4))).clicked() {
                shift = 1;
            }
            if ui.add(egui::Button::new("Move left").fill(FIELD()).corner_radius(radius(4))).clicked() {
                shift = -1;
            }
        });
        let all: Vec<&Opt> = d.opts.iter().chain(std::iter::once(&SIZE)).collect();
        let n = all.len();
        for (k, o) in all.into_iter().enumerate() {
            let last = k + 1 == n;
            let m = &mut st.cfg.lanes[lane][i];
            match o {
                Opt::Bool(key, label, default) => {
                    let mut v = m.get(*key).and_then(|v| v.as_bool()).unwrap_or(*default);
                    let old = v;
                    toggle_row(ui, label, "", last, &mut v);
                    if v != old {
                        m.insert((*key).into(), json!(v));
                    }
                }
                Opt::Choice(key, label, options, default) => {
                    let mut v = m.get(*key).and_then(|v| v.as_str()).unwrap_or(default).to_string();
                    let old = v.clone();
                    let opts: Vec<(String, &str)> = options.iter().map(|(a, b)| (a.to_string(), *b)).collect();
                    choice_row(ui, label, "", last, &mut v, &opts);
                    if v != old {
                        m.insert((*key).into(), json!(v));
                    }
                }
                Opt::Int(key, label, lo, hi, default) => {
                    let mut v = m.get(*key).and_then(|v| v.as_i64()).unwrap_or(*default);
                    let old = v;
                    slider_row(ui, label, "", last, &mut v, *lo..=*hi, if *key == "scale" { " %" } else { " px" });
                    if v != old {
                        m.insert((*key).into(), json!(v));
                    }
                }
            }
        }
    });
    let lane_ref = &mut st.cfg.lanes[lane];
    if remove {
        lane_ref.remove(i);
        st.sel = None;
    } else if shift != 0 {
        let j = i as i32 + shift;
        if j >= 0 && (j as usize) < lane_ref.len() {
            lane_ref.swap(i, j as usize);
            st.sel = Some((lane, j as usize));
        }
    }
}

fn bar_options(ui: &mut Ui, c: &mut BarCfg) {
    card(ui, "Bar", |ui| {
        let mut show = !c.hidden;
        row_sub(ui, "view-dashboard-outline", "Always show the bar", "Off: the whole screen is yours; hold Super to see time, workspaces and shortcuts.", false, |ui| { switch(ui, &mut show); });
        c.hidden = !show;
        if c.hidden {
            toggle_row(ui, "Show the HUD while holding Super", "", false, &mut c.hud);
        }
        choice_row(ui, "Position", "", false, &mut c.position, &[("top".to_string(), "Top"), ("bottom".to_string(), "Bottom")]);
        choice_row(
            ui,
            "Style",
            "Flat: a full-width strip. Island: a rounded bar with a gap around it. Floating: only the modules, no background.",
            false,
            &mut c.style,
            &[("flat".to_string(), "Flat"), ("island".to_string(), "Island"), ("floating".to_string(), "Floating")],
        );
        slider_row(ui, "Height", "", false, &mut c.height, 28..=56, " px");
        slider_row(ui, "Space between modules", "", false, &mut c.spacing, 0..=24, " px");
        if c.style != "floating" {
            toggle_row(ui, "Group modules in pills", "Give the left, center and right groups their own rounded background.", false, &mut c.pills);
        }
        let spaced = c.style != "flat";
        if spaced {
            slider_row(ui, "Gap to screen edge", "", false, &mut c.margin, 0..=20, " px");
        }
        if c.style != "flat" || c.pills {
            let mut follow = c.radius.is_none();
            toggle_row(ui, "Corners follow theme", "Uses the roundness from the Theme page.", follow, &mut follow);
            if follow {
                c.radius = None;
            } else {
                let mut r = c.radius.unwrap_or(8);
                slider_row(ui, "Corner radius", "", false, &mut r, 0..=24, " px");
                c.radius = Some(r);
            }
        }
        if c.style != "floating" {
            let mut pct = (c.opacity * 100.0).round() as i64;
            slider_row(ui, "Background opacity", "", true, &mut pct, 30..=100, "%");
            c.opacity = pct as f64 / 100.0;
        } else {
            ui.add_space(0.0);
        }
    });
    let _ = Color32::TRANSPARENT;
}
