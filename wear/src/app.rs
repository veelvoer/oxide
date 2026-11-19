//! The watch app: a PC list, and per PC three pages side by side (touchpad or gyro mouse, controls, stats) that you swipe
//! between. Made for a small round screen: big touch targets, little drawing, repaint only when something moves.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use egui::{vec2, Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use oxide_remote_proto::{Announce, PcInfo, Stats, ToPc, ToPhone};

use crate::{
    motion,
    net::{self, Event, PairEvent, Route, Session},
    platform,
    settings::{Settings, ACCENTS},
    store::Store,
    theme::*,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES: &str = "https://github.com/veelvoer/oxide/releases.atom";
const DOWNLOAD: &str = "https://github.com/veelvoer/oxide/releases/download";

mod ic {
    pub const MONITOR: &str = "\u{E32E}";
    pub const PLUS: &str = "\u{E3D4}";
    pub const GEAR: &str = "\u{E272}";
    pub const POWER: &str = "\u{E3DA}";
    pub const LOCK: &str = "\u{E2FA}";
    pub const MOON: &str = "\u{E330}";
    pub const SPEAKER: &str = "\u{E44A}";
    pub const MUTE: &str = "\u{E45C}";
    pub const PLAY: &str = "\u{E3D0}";
    pub const PREV: &str = "\u{E5A4}";
    pub const NEXT: &str = "\u{E5A6}";
    pub const BACK: &str = "\u{E058}";
    pub const CHECK: &str = "\u{E182}";
    pub const BACKSPACE: &str = "\u{E0AE}";
    pub const UPDATE: &str = "\u{E1AC}";
    pub const REFRESH: &str = "\u{E094}";
    pub const MOUSE: &str = "\u{E33A}";
    pub const GRID: &str = "\u{E464}";
    pub const SLIDERS: &str = "\u{E434}";
    pub const CHART: &str = "\u{E156}";
    pub const CARET: &str = "\u{E13A}";
    pub const MINUS: &str = "\u{E32A}";
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Home,
    Add,
    Pc(usize),
    Settings,
    Theme,
    Cursor,
    Updates,
}

/// An open PC.
struct Live {
    session: Session,
    connected: bool,
    route: Option<Route>,
    error: Option<String>,
    stats: Option<Stats>,
    info: Option<PcInfo>,
    last_stats: Instant,
    volume: f32,
    closed_at: Option<Instant>,
}

#[derive(Default)]
struct Update {
    rx: Option<std::sync::mpsc::Receiver<Result<Option<(String, String)>, String>>>,
    latest: Option<(String, String)>,
    checked: bool,
    error: Option<String>,
}

/// Pairing from the watch: choose a PC that answers on the network, then type the 6 digits it shows.
struct Adding {
    found: Option<Vec<(Announce, String)>>,
    scanning: Option<std::sync::mpsc::Receiver<Vec<(Announce, String)>>>,
    pairing: Option<(String, std::sync::mpsc::Receiver<PairEvent>, std::sync::mpsc::Sender<String>)>,
    need_code: bool,
    code: String,
    error: Option<String>,
}

/// A finger on the PC pages.
#[derive(Default)]
struct Touch {
    start: Option<Pos2>,
    started_at: Option<Instant>,
    /// the finger pages between the screens (it does not move the pointer)
    paging: bool,
    /// the pager's position when the finger went down
    pager0: f32,
    moved: f32,
    long_sent: bool,
    can_page: bool,
}

pub struct App {
    ctx: egui::Context,
    store: Store,
    set: Settings,
    page: Page,
    tab: usize,
    /// where the pager is shown (0 = pad, 1 = controls, 2 = stats), moving smoothly toward `tab`
    pager: f32,
    touch: Touch,
    online: Arc<Mutex<HashMap<String, Option<Route>>>>,
    new_hosts: Arc<Mutex<HashMap<String, String>>>,
    last_probe: Instant,
    live: Option<Live>,
    update: Update,
    adding: Adding,
    toast: Option<(String, Instant)>,
    pal: Option<Pal>,
    last_theme: Instant,
    /// the gyro mouse reads the sensors
    motion_on: bool,
    /// the pointer follows the hand (tap the middle to pause)
    gyro_live: bool,
    last_pinches: Option<(u32, Instant)>,
    test_pinches: u32,
    gyro_ok: bool,
}

fn semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim_start_matches('v').split('.').map(|n| n.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// The newest watch release in the releases feed: tags are "watch-vX.Y.Z", the file "OxideWatch-X.Y.Z.apk".
fn newest_apk(feed: &str) -> Option<(String, String)> {
    let mut best: Option<(String, String)> = None;
    for part in feed.split("/releases/tag/").skip(1) {
        let tag: String = part.chars().take_while(|c| !matches!(c, '"' | '<' | '&' | ' ')).collect();
        let Some(v) = tag.strip_prefix("watch-v") else { continue };
        if best.as_ref().is_none_or(|(b, _)| semver(v) > semver(b)) {
            best = Some((v.to_string(), format!("{DOWNLOAD}/{tag}/OxideWatch-{v}.apk")));
        }
    }
    best
}

// ---------------------------------------------------------------- small widgets

enum Look {
    Filled,
    Tonal,
    Plain,
}

/// A rounded button with an icon and/or text, that sinks a little when pressed. Returns true when tapped.
fn button(ui: &mut Ui, size: Vec2, icon: Option<&str>, text: &str, look: Look) -> bool {
    let p = pal();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    paint_button(ui, rect, &resp, icon, text, look, p);
    resp.clicked()
}

fn paint_button(ui: &Ui, rect: Rect, resp: &egui::Response, icon: Option<&str>, text: &str, look: Look, p: Pal) {
    let down = ui.ctx().animate_bool_with_time(resp.id.with("down"), resp.is_pointer_button_down_on(), 0.08);
    let r2 = rect.shrink(down * 2.0);
    let (fill, ink) = match look {
        Look::Filled => (p.accent, p.on_accent),
        Look::Tonal => (p.raised, p.text),
        Look::Plain => (Color32::TRANSPARENT, p.text),
    };
    let fill = if matches!(look, Look::Plain) { mix(p.raised, p.bg, 1.0 - down * 0.8) } else { mix(fill, p.text, down * 0.12) };
    let pill = r2.height() / 2.0;
    ui.painter().rect_filled(r2, pill.min(255.0) as u8, fill);
    match (icon, text.is_empty()) {
        (Some(g), true) => {
            ui.painter().text(r2.center(), Align2::CENTER_CENTER, g, icon_font(r2.height() * 0.5, false), ink);
        }
        (Some(g), false) => {
            ui.painter().text(Pos2::new(r2.left() + 16.0, r2.center().y), Align2::LEFT_CENTER, g, icon_font(16.0, false), ink);
            ui.painter().text(Pos2::new(r2.left() + 38.0, r2.center().y), Align2::LEFT_CENTER, text, medium(13.0), ink);
        }
        (None, _) => {
            ui.painter().text(r2.center(), Align2::CENTER_CENTER, text, medium(13.0), ink);
        }
    }
}

/// A row in a list: icon, title, a small line under it, and a caret.
fn row(ui: &mut Ui, width: f32, icon: &str, title: &str, sub: &str, caret: bool) -> bool {
    let p = pal();
    let h = if sub.is_empty() { 44.0 } else { 52.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    let down = ui.ctx().animate_bool_with_time(resp.id.with("down"), resp.is_pointer_button_down_on(), 0.08);
    let r2 = rect.shrink(down * 2.0);
    ui.painter().rect_filled(r2, 22, mix(p.card, p.text, down * 0.1));
    ui.painter().text(Pos2::new(r2.left() + 20.0, r2.center().y), Align2::CENTER_CENTER, icon, icon_font(18.0, false), p.accent);
    let ty = if sub.is_empty() { r2.center().y } else { r2.center().y - 8.0 };
    ui.painter().text(Pos2::new(r2.left() + 40.0, ty), Align2::LEFT_CENTER, title, medium(13.5), p.text);
    if !sub.is_empty() {
        ui.painter().text(Pos2::new(r2.left() + 40.0, r2.center().y + 9.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.0), p.muted);
    }
    if caret {
        ui.painter().text(Pos2::new(r2.right() - 16.0, r2.center().y), Align2::CENTER_CENTER, ic::CARET, icon_font(14.0, false), p.muted);
    }
    resp.clicked()
}

use egui::FontId;

/// A switch row: text left, a small switch right.
fn toggle(ui: &mut Ui, width: f32, title: &str, on: &mut bool) -> bool {
    let p = pal();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 44.0), Sense::click());
    ui.painter().rect_filled(rect, 22, p.card);
    ui.painter().text(Pos2::new(rect.left() + 16.0, rect.center().y), Align2::LEFT_CENTER, title, medium(13.0), p.text);
    let t = ui.ctx().animate_bool_with_time(resp.id.with("on"), *on, 0.12);
    let sw = Rect::from_center_size(Pos2::new(rect.right() - 34.0, rect.center().y), vec2(40.0, 22.0));
    ui.painter().rect_filled(sw, 11, mix(p.raised, p.accent, t));
    ui.painter().circle_filled(Pos2::new(sw.left() + 11.0 + 18.0 * t, sw.center().y), 8.0, mix(p.muted, p.on_accent, t));
    if resp.clicked() {
        *on = !*on;
        return true;
    }
    false
}

/// `-  value  +` with a title: for numbers on a screen where a slider is too fiddly.
fn stepper(ui: &mut Ui, width: f32, title: &str, value: &mut f32, step: f32, min: f32, max: f32, unit: &str) -> bool {
    let p = pal();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 52.0), Sense::hover());
    ui.painter().rect_filled(rect, 26, p.card);
    ui.painter().text(Pos2::new(rect.center().x, rect.top() + 12.0), Align2::CENTER_CENTER, title, FontId::proportional(11.0), p.muted);
    ui.painter().text(Pos2::new(rect.center().x, rect.bottom() - 17.0), Align2::CENTER_CENTER, format!("{value:.2}{unit}").replace(".00", "").replace(".50", ".5"), semibold(15.0), p.text);
    let mut changed = false;
    for (dx, ic_, d) in [(-1.0, ic::MINUS, -step), (1.0, ic::PLUS, step)] {
        let c = Pos2::new(rect.center().x + dx * (width / 2.0 - 26.0), rect.bottom() - 17.0);
        let r = Rect::from_center_size(c, vec2(40.0, 34.0));
        let resp = ui.interact(r, ui.scope_id().with((title, dx as i32)), Sense::click());
        let down = resp.is_pointer_button_down_on();
        ui.painter().rect_filled(r.shrink(2.0), 17, mix(p.raised, p.text, if down { 0.12 } else { 0.0 }));
        ui.painter().text(c, Align2::CENTER_CENTER, ic_, icon_font(14.0, false), p.text);
        if resp.clicked() {
            *value = (*value + d).clamp(min, max);
            changed = true;
        }
    }
    changed
}

/// A choice between a few options as pills side by side.
fn choice(ui: &mut Ui, width: f32, options: &[&str], selected: usize) -> Option<usize> {
    let p = pal();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 38.0), Sense::hover());
    ui.painter().rect_filled(rect, 19, p.card);
    let w = rect.width() / options.len() as f32;
    let mut pick = None;
    for (i, o) in options.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(rect.left() + w * i as f32, rect.top()), vec2(w, rect.height())).shrink(3.0);
        let resp = ui.interact(r, ui.scope_id().with(("choice", o)), Sense::click());
        let on = i == selected;
        let t = ui.ctx().animate_bool_with_time(resp.id.with("sel"), on, 0.14);
        ui.painter().rect_filled(r, 16, mix(Color32::TRANSPARENT, p.accent, t));
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *o, medium(12.0), mix(p.text, p.on_accent, t));
        if resp.clicked() {
            pick = Some(i);
        }
    }
    pick
}

fn title(ui: &mut Ui, width: f32, text: &str) {
    let p = pal();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, semibold(14.0), p.text);
}

fn arc(ui: &Ui, center: Pos2, radius: f32, from: f32, to: f32, stroke: Stroke) {
    let steps = (((to - from).abs() * radius) / 3.0).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            center + vec2(a.cos(), a.sin()) * radius
        })
        .collect();
    ui.painter().add(egui::Shape::line(pts, stroke));
}

impl App {
    pub fn new(ctx: &egui::Context, dir: PathBuf) -> Self {
        fonts(ctx);
        let store = Store::load(dir.clone());
        let set = Settings::load(dir);
        let mut a = Self {
            ctx: ctx.clone(),
            store,
            set,
            page: Page::Home,
            tab: 0,
            pager: 0.0,
            touch: Touch::default(),
            online: Arc::default(),
            new_hosts: Arc::default(),
            last_probe: Instant::now() - Duration::from_secs(10),
            live: None,
            update: Update::default(),
            adding: Adding { found: None, scanning: None, pairing: None, need_code: false, code: String::new(), error: None },
            toast: None,
            pal: None,
            last_theme: Instant::now() - Duration::from_secs(10),
            motion_on: false,
            gyro_live: true,
            last_pinches: None,
            test_pinches: 0,
            gyro_ok: motion::has_gyro(),
        };
        if a.store.phone_name == "Phone" {
            a.store.phone_name = platform::device_name().map(|n| format!("{n} (watch)")).unwrap_or_else(|| "Watch".into());
            a.store.save();
        }
        if a.set.auto_update {
            a.check_update();
        }
        a
    }

    fn say(&mut self, t: impl Into<String>) {
        self.toast = Some((t.into(), Instant::now()));
    }

    // ------------------------------------------------------------ theme

    fn palette(&self) -> Pal {
        let mut p = match self.set.theme.as_str() {
            "pc" => {
                let pc = self.live.as_ref().and_then(|_| if let Page::Pc(i) = self.page { self.store.pcs.get(i) } else { None }).or_else(|| self.store.pcs.first());
                pc.and_then(|pc| Pal::from_pc(&pc.palette)).unwrap_or_else(|| Pal::oxide(true))
            }
            "custom" => {
                let c = ACCENTS[self.set.accent.min(ACCENTS.len() - 1)].1;
                let accent = Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
                let mut p = Pal::oxide(true);
                p.accent = accent;
                p.on_accent = Color32::from_rgb(0x10, 0x12, 0x18);
                p.container = mix(p.card, accent, 0.28);
                p
            }
            _ => Pal::oxide(true),
        };
        // a watch screen is OLED: real black saves battery and looks deeper
        if p.dark && self.set.theme != "pc" {
            p.bg = Color32::BLACK;
            p.deep = Color32::BLACK;
            p.card = mix(Color32::BLACK, p.card, 0.8);
        }
        p
    }

    fn refresh_theme(&mut self) {
        if self.last_theme.elapsed() < Duration::from_millis(1500) && self.pal.is_some() {
            return;
        }
        self.last_theme = Instant::now();
        let p = self.palette();
        if self.pal != Some(p) {
            self.pal = Some(p);
            apply(&self.ctx, p);
            // smaller everything: 225 points across
            self.ctx.all_styles_mut(|s| {
                s.spacing.item_spacing = vec2(6.0, 6.0);
                s.spacing.button_padding = vec2(10.0, 6.0);
                s.spacing.interact_size.y = 36.0;
                s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.0));
                s.text_styles.insert(egui::TextStyle::Button, medium(13.0));
                s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(11.0));
                s.text_styles.insert(egui::TextStyle::Heading, semibold(16.0));
            });
        }
    }

    // ------------------------------------------------------------ network

    fn probe(&mut self) {
        if self.last_probe.elapsed() < Duration::from_secs(8) || self.page != Page::Home {
            return;
        }
        self.last_probe = Instant::now();
        for pc in self.store.pcs.clone() {
            let online = self.online.clone();
            let new_hosts = self.new_hosts.clone();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let r = net::reachable(&pc);
                if let Some((_, Some(h))) = &r {
                    new_hosts.lock().unwrap().insert(pc.id.clone(), h.clone());
                }
                online.lock().unwrap().insert(pc.id.clone(), r.map(|(route, _)| route));
                ctx.request_repaint();
            });
        }
    }

    fn open_pc(&mut self, i: usize) {
        let ctx = self.ctx.clone();
        let session = net::open(&self.store.pcs[i], &self.store.phone_id, move || ctx.request_repaint());
        self.live = Some(Live { session, connected: false, route: None, error: None, stats: None, info: None, last_stats: Instant::now() - Duration::from_secs(5), volume: 0.5, closed_at: None });
        self.page = Page::Pc(i);
        self.tab = 0;
        self.pager = 0.0;
        self.last_theme = Instant::now() - Duration::from_secs(10);
    }

    fn wake(&mut self, i: usize) {
        let pc = self.store.pcs[i].clone();
        let mut ok = false;
        for m in &pc.macs {
            ok |= oxide_remote_proto::wake(m, pc.subnet_broadcast()).is_ok();
        }
        // through the other PCs that are on, for when the watch is not on the PC's network
        let online = self.online.lock().unwrap().clone();
        let helpers: Vec<_> = self.store.pcs.iter().filter(|p| p.id != pc.id && online.get(&p.id).is_some_and(|r| r.is_some())).cloned().collect();
        for h in &helpers {
            let msgs = pc.macs.iter().map(|m| ToPc::Action { name: "wake".into(), arg: Some(format!("{m} {}", pc.host)) }).collect();
            let (h, id) = (h.clone(), self.store.phone_id.clone());
            std::thread::spawn(move || {
                let _ = net::one_shot(&h, &id, msgs);
            });
        }
        platform::haptic(1);
        self.say(if ok || !helpers.is_empty() { format!("Waking {}", pc.name) } else { "Could not send the wake-up".to_string() });
        self.last_probe = Instant::now() - Duration::from_secs(6);
    }

    fn send(&self, m: ToPc) {
        if let Some(l) = &self.live {
            let _ = l.session.to_pc.send(m);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let pc_idx = if let Page::Pc(i) = self.page { Some(i) } else { None };
        let mut save = false;
        let mut reopen = false;
        if let Some(l) = self.live.as_mut() {
            while let Ok(ev) = l.session.events.try_recv() {
                match ev {
                    Event::Connected { route, host, .. } => {
                        if let (Some(h), Some(pc)) = (host, pc_idx.and_then(|i| self.store.pcs.get_mut(i))) {
                            pc.host = h;
                            save = true;
                        }
                        l.connected = true;
                        l.route = Some(route);
                        l.error = None;
                        let _ = l.session.to_pc.send(ToPc::Info);
                        let _ = l.session.to_pc.send(ToPc::Stats);
                    }
                    Event::Moved(w) | Event::Msg(ToPhone::Whereabouts(w)) => {
                        if let Some(pc) = pc_idx.and_then(|i| self.store.pcs.get_mut(i)) {
                            if pc.remote != w.addrs || pc.rendezvous != w.rendezvous || pc.lan != w.lan {
                                pc.remote = w.addrs;
                                pc.lan = w.lan;
                                pc.rendezvous = w.rendezvous;
                                save = true;
                            }
                        }
                    }
                    Event::Msg(ToPhone::Info(info)) => {
                        if let Some(pc) = pc_idx.and_then(|i| self.store.pcs.get_mut(i)) {
                            if !info.palette.is_empty() && pc.palette != info.palette {
                                pc.palette = info.palette.clone();
                                save = true;
                            }
                            if !info.macs.is_empty() && pc.macs != info.macs {
                                pc.macs = info.macs.clone();
                                save = true;
                            }
                        }
                        l.info = Some(info);
                    }
                    Event::Msg(ToPhone::Stats(s)) => {
                        if let Some(v) = s.volume {
                            l.volume = v;
                        }
                        l.stats = Some(s);
                    }
                    Event::Msg(ToPhone::Done { ok, message }) => {
                        if !ok || !message.is_empty() {
                            self.toast = Some((if message.is_empty() { "Done".into() } else { message }, Instant::now()));
                        }
                    }
                    Event::Msg(_) | Event::Frame { .. } => {}
                    Event::Closed(e) => {
                        l.error = Some(e);
                        l.connected = false;
                        l.closed_at = Some(Instant::now());
                    }
                }
            }
            // a dropped connection (the watch's Wi-Fi sleeps) is opened again
            if l.closed_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
                reopen = true;
            }
            // numbers once a second, only while they are shown
            if l.connected && self.tab == 2 && l.last_stats.elapsed() > Duration::from_secs(1) {
                l.last_stats = Instant::now();
                let _ = l.session.to_pc.send(ToPc::Stats);
                ctx.request_repaint_after(Duration::from_millis(1000));
            }
        }
        if reopen {
            if let Some(i) = pc_idx {
                let (tab, pager) = (self.tab, self.pager);
                self.open_pc(i);
                self.tab = tab;
                self.pager = pager;
            }
        }
        if save {
            self.store.save();
        }
        // updates
        if let Some(rx) = &self.update.rx {
            if let Ok(r) = rx.try_recv() {
                self.update.checked = true;
                match r {
                    Ok(l) => self.update.latest = l,
                    Err(e) => self.update.error = Some(e),
                }
                self.update.rx = None;
            }
        }
        // pairing
        if let Some(rx) = &self.adding.scanning {
            if let Ok(f) = rx.try_recv() {
                self.adding.found = Some(f);
                self.adding.scanning = None;
            }
        }
        let mut done = None;
        if let Some((_, rx, _)) = &self.adding.pairing {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    PairEvent::NeedCode { .. } => self.adding.need_code = true,
                    PairEvent::Done(r) => done = Some(r),
                }
            }
        }
        if let Some(r) = done {
            self.adding.pairing = None;
            self.adding.need_code = false;
            self.adding.code.clear();
            match r {
                Ok(pc) => {
                    let name = pc.name.clone();
                    self.store.upsert(pc);
                    self.say(format!("Paired with {name}"));
                    self.page = Page::Home;
                }
                Err(e) => self.adding.error = Some(e),
            }
        }
    }

    fn check_update(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = match platform::http_get(RELEASES) {
                Some(feed) => Ok(newest_apk(&feed).filter(|(v, _)| semver(v) > semver(VERSION))),
                None => Err("Could not reach GitHub".to_string()),
            };
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        self.update.rx = Some(rx);
        self.update.error = None;
    }

    fn scan(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(net::discover(Duration::from_millis(2200)));
            ctx.request_repaint();
        });
        self.adding.scanning = Some(rx);
        self.adding.found = None;
        self.adding.error = None;
    }

    // ------------------------------------------------------------ frame

    pub fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.refresh_theme();
        self.poll(&ctx);
        self.probe();
        let p = pal();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0, p.bg);
        // the corners of a round screen are cut: content lives in a rectangle that fits inside the circle
        let inner = Rect::from_min_max(full.min + vec2(14.0, 8.0), full.max - vec2(14.0, 8.0));
        // the sensors run only on the gyro pad (and its test in the settings)
        let want_motion = (matches!(self.page, Page::Pc(_)) && self.tab == 0 && self.set.cursor == "gyro") || self.page == Page::Cursor;
        if want_motion != self.motion_on {
            self.motion_on = want_motion;
            motion::enable(want_motion, self.set.pinch, self.set.pinch_level);
            self.test_pinches = 0;
        }
        match self.page {
            Page::Home => self.home(ui, inner),
            Page::Add => self.add(ui, inner),
            Page::Pc(i) => self.pc(ui, full, i),
            Page::Settings => self.settings(ui, inner),
            Page::Theme => self.theme_page(ui, inner),
            Page::Cursor => self.cursor_page(ui, inner),
            Page::Updates => self.updates_page(ui, inner),
        }
        if let Some((t, at)) = &self.toast {
            if at.elapsed() < Duration::from_millis(2200) {
                let r = Rect::from_center_size(Pos2::new(full.center().x, full.bottom() - 30.0), vec2(170.0, 30.0));
                ui.painter().rect_filled(r, 15, mix(p.raised, p.text, 0.1));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, t, medium(12.0), p.text);
                ctx.request_repaint_after(Duration::from_millis(300));
            } else {
                self.toast = None;
            }
        }
    }

    /// A scrolling column in `inner`, with room above and below for the round screen's edge.
    fn column(&mut self, ui: &mut Ui, inner: Rect, f: impl FnOnce(&mut Self, &mut Ui, f32)) {
        let area = Rect::from_min_max(inner.min + vec2(0.0, 18.0), inner.max - vec2(0.0, 6.0));
        let w = area.width();
        ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                f(self, ui, w);
                ui.add_space(26.0);
            });
        });
    }

    fn back(&mut self, ui: &mut Ui, w: f32, to: Page, text: &str) {
        let p = pal();
        ui.horizontal(|ui| {
            if button(ui, vec2(34.0, 30.0), Some(ic::BACK), "", Look::Tonal) {
                self.page = to;
            }
            let (rect, _) = ui.allocate_exact_size(vec2(w - 40.0, 30.0), Sense::hover());
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, semibold(14.0), p.text);
        });
    }

    // ------------------------------------------------------------ home

    fn home(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let online = self.online.lock().unwrap().clone();
        if let Ok(mut h) = self.new_hosts.lock() {
            for (id, host) in h.drain() {
                if let Some(pc) = self.store.pcs.iter_mut().find(|p| p.id == id) {
                    pc.host = host;
                }
            }
        }
        let pcs = self.store.pcs.clone();
        let mut open = None;
        let mut wake = None;
        let mut go = None;
        let update_ready = self.update.latest.is_some();
        self.column(ui, inner, |_, ui, w| {
            title(ui, w, "Your PCs");
            if pcs.is_empty() {
                let (rect, _) = ui.allocate_exact_size(vec2(w, 54.0), Sense::hover());
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, "No PC yet. Pair one\nfrom this watch.", FontId::proportional(12.0), p.muted);
            }
            for (i, pc) in pcs.iter().enumerate() {
                let state = online.get(&pc.id).copied();
                let (sub, dot) = match state {
                    Some(Some(Route::Home)) => ("Online", p.good),
                    Some(Some(Route::Away)) => ("Online · internet", p.good),
                    Some(None) => ("Offline", p.muted),
                    None => ("Looking…", p.muted),
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
                let down = ui.ctx().animate_bool_with_time(resp.id.with("down"), resp.is_pointer_button_down_on(), 0.08);
                let r2 = rect.shrink(down * 2.0);
                ui.painter().rect_filled(r2, 27, mix(p.card, p.text, down * 0.1));
                ui.painter().text(Pos2::new(r2.left() + 22.0, r2.center().y), Align2::CENTER_CENTER, ic::MONITOR, icon_font(20.0, false), p.accent);
                ui.painter().circle_filled(Pos2::new(r2.left() + 33.0, r2.center().y + 9.0), 4.0, dot);
                ui.painter().text(Pos2::new(r2.left() + 44.0, r2.center().y - 8.0), Align2::LEFT_CENTER, &pc.name, medium(13.5), p.text);
                ui.painter().text(Pos2::new(r2.left() + 44.0, r2.center().y + 9.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.0), p.muted);
                // wake: its own round button, separate from opening
                let wr = Rect::from_center_size(Pos2::new(r2.right() - 24.0, r2.center().y), vec2(36.0, 36.0));
                let wresp = ui.interact(wr, resp.id.with("wake"), Sense::click());
                let wdown = wresp.is_pointer_button_down_on();
                ui.painter().circle_filled(wr.center(), 17.0, mix(p.raised, p.text, if wdown { 0.14 } else { 0.0 }));
                ui.painter().text(wr.center(), Align2::CENTER_CENTER, ic::POWER, icon_font(16.0, false), if state == Some(None) { p.accent } else { p.muted });
                if wresp.clicked() {
                    wake = Some(i);
                } else if resp.clicked() {
                    open = Some(i);
                }
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let bw = (w - 6.0) / 2.0;
                if button(ui, vec2(bw, 38.0), Some(ic::PLUS), "Add", Look::Tonal) {
                    go = Some(Page::Add);
                }
                if button(ui, vec2(bw, 38.0), Some(ic::GEAR), if update_ready { "Update" } else { "Setup" }, if update_ready { Look::Filled } else { Look::Tonal }) {
                    go = Some(Page::Settings);
                }
            });
        });
        if let Some(i) = open {
            self.open_pc(i);
        }
        if let Some(i) = wake {
            self.wake(i);
        }
        if let Some(g) = go {
            if g == Page::Add {
                self.adding = Adding { found: None, scanning: None, pairing: None, need_code: false, code: String::new(), error: None };
                self.scan();
            }
            self.page = g;
        }
        ui.ctx().request_repaint_after(Duration::from_secs(2));
    }

    // ------------------------------------------------------------ pairing

    fn add(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let mut pick: Option<(String, u16)> = None;
        let mut rescan = false;
        let mut leave = false;
        let mut digit = None;
        let mut backspace = false;
        let mut submit = false;
        let need_code = self.adding.need_code;
        let pairing = self.adding.pairing.as_ref().map(|(n, _, _)| n.clone());
        let found = self.adding.found.clone();
        let scanning = self.adding.scanning.is_some();
        let error = self.adding.error.clone();
        let code = self.adding.code.clone();
        let paired: Vec<String> = self.store.pcs.iter().map(|p| p.id.clone()).collect();
        self.column(ui, inner, |app, ui, w| {
            app.back(ui, w, Page::Home, "Add a PC");
            if need_code {
                let (rect, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
                ui.painter().text(Pos2::new(rect.center().x, rect.top() + 8.0), Align2::CENTER_CENTER, format!("Code on {}", pairing.clone().unwrap_or_default()), FontId::proportional(11.0), p.muted);
                let shown: String = (0..6).map(|i| code.chars().nth(i).map(|c| c.to_string()).unwrap_or_else(|| "·".into())).collect::<Vec<_>>().join(" ");
                ui.painter().text(Pos2::new(rect.center().x, rect.bottom() - 10.0), Align2::CENTER_CENTER, shown, semibold(20.0), p.text);
                let cell = (w - 12.0) / 3.0;
                for rowk in 0..4 {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for col in 0..3 {
                            let k = rowk * 3 + col;
                            let (label, ic_): (String, Option<&str>) = match k {
                                9 => (String::new(), Some(ic::BACKSPACE)),
                                10 => ("0".into(), None),
                                11 => (String::new(), Some(ic::CHECK)),
                                n => ((n + 1).to_string(), None),
                            };
                            let look = if k == 11 { Look::Filled } else { Look::Tonal };
                            if button(ui, vec2(cell, 34.0), ic_, &label, look) {
                                match k {
                                    9 => backspace = true,
                                    11 => submit = true,
                                    10 => digit = Some('0'),
                                    n => digit = Some(char::from(b'1' + n as u8)),
                                }
                            }
                        }
                    });
                }
            } else if let Some(name) = &pairing {
                let (rect, _) = ui.allocate_exact_size(vec2(w, 60.0), Sense::hover());
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("Connecting to\n{name}…"), medium(13.0), p.muted);
            } else {
                if scanning {
                    let (rect, _) = ui.allocate_exact_size(vec2(w, 50.0), Sense::hover());
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Looking for PCs…", medium(13.0), p.muted);
                } else if let Some(f) = &found {
                    let fresh: Vec<_> = f.iter().filter(|(a, _)| !paired.contains(&a.id)).collect();
                    if fresh.is_empty() {
                        let (rect, _) = ui.allocate_exact_size(vec2(w, 78.0), Sense::hover());
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            "No PC found.\nOn the PC: Settings >\nPhone > Pair a phone",
                            FontId::proportional(12.0),
                            p.muted,
                        );
                    }
                    for (a, host) in fresh {
                        if row(ui, w, ic::MONITOR, &a.name, host, true) {
                            pick = Some((host.clone(), a.port));
                        }
                    }
                }
                if let Some(e) = &error {
                    let (rect, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, e, FontId::proportional(11.0), p.bad);
                }
                if !scanning && button(ui, vec2(w, 36.0), Some(ic::REFRESH), "Look again", Look::Tonal) {
                    rescan = true;
                }
            }
            let _ = &mut leave;
        });
        if let Some((host, port)) = pick {
            let name = found.as_ref().and_then(|f| f.iter().find(|(_, h)| *h == host).map(|(a, _)| a.name.clone())).unwrap_or_default();
            let (rx, tx) = net::pair(host, port, self.store.phone_id.clone(), self.store.phone_name.clone());
            self.adding.pairing = Some((name, rx, tx));
            self.adding.error = None;
        }
        if let Some(d) = digit {
            if self.adding.code.len() < 6 {
                self.adding.code.push(d);
            }
        }
        if backspace {
            self.adding.code.pop();
        }
        if submit && self.adding.code.len() == 6 {
            if let Some((_, _, tx)) = &self.adding.pairing {
                let _ = tx.send(self.adding.code.clone());
            }
            self.adding.need_code = false;
        }
        if rescan {
            self.scan();
        }
        if scanning || pairing.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }

    // ------------------------------------------------------------ one PC: three pages side by side

    fn pc(&mut self, ui: &mut Ui, full: Rect, i: usize) {
        let p = pal();
        let ctx = ui.ctx().clone();
        let w = full.width();
        let gyro = self.set.cursor == "gyro";
        // ---- the finger: it moves the pointer, or it pages between the screens
        let (pressed, down, released, pos, vel, delta) = ctx.input(|inp| {
            (inp.pointer.primary_pressed(), inp.pointer.primary_down(), inp.pointer.primary_released(), inp.pointer.latest_pos(), inp.pointer.velocity(), inp.pointer.delta())
        });
        let strip_top = full.bottom() - 52.0;
        if pressed {
            if let Some(pos) = pos {
                let edge = pos.x > full.right() - 0.26 * w || pos.x < full.left() + 0.26 * w;
                // swipes page from the edges on the touchpad (its middle is the pointer's), from anywhere else
                self.touch = Touch { start: Some(pos), started_at: Some(Instant::now()), paging: false, pager0: self.pager, moved: 0.0, long_sent: false, can_page: self.tab != 0 || gyro || edge };
            }
        }
        if down {
            if let (Some(start), Some(pos)) = (self.touch.start, pos) {
                let d = pos - start;
                self.touch.moved = self.touch.moved.max(d.length());
                if !self.touch.paging && self.touch.can_page && d.x.abs() > 12.0 && d.x.abs() > d.y.abs() * 1.4 {
                    self.touch.paging = true;
                    self.touch.pager0 = self.pager;
                }
                if self.touch.paging {
                    self.pager = (self.touch.pager0 - d.x / w).clamp(-0.15, 2.15);
                    ctx.request_repaint();
                } else if self.tab == 0 && !gyro && start.y < strip_top {
                    let k = 2.6 * self.set.speed;
                    let (mut dx, mut dy) = (delta.x * k, delta.y * k);
                    // a little acceleration: a quick flick travels further
                    let speed = (delta.length() * 60.0 / 400.0).clamp(0.0, 1.0);
                    dx *= 1.0 + speed * 0.8;
                    dy *= 1.0 + speed * 0.8;
                    if self.set.invert_x {
                        dx = -dx;
                    }
                    if self.set.invert_y {
                        dy = -dy;
                    }
                    if dx != 0.0 || dy != 0.0 {
                        self.send(ToPc::PointerBy { dx: dx as f64, dy: dy as f64 });
                    }
                    // held still: a right click
                    if !self.touch.long_sent && self.touch.moved < 6.0 && self.touch.started_at.is_some_and(|t| t.elapsed() > Duration::from_millis(550)) {
                        self.touch.long_sent = true;
                        self.send(ToPc::Click { button: "right".into() });
                        platform::haptic(1);
                    }
                }
            }
            if self.touch.start.is_some() && !self.touch.paging && self.tab == 0 && !gyro && !self.touch.long_sent {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if released {
            if self.touch.paging {
                let target = (self.pager - vel.x / w * 0.18).round().clamp(0.0, 2.0) as usize;
                if target != self.tab {
                    self.tab = target;
                    platform::haptic(0);
                }
            } else if self.tab == 0 && !gyro && self.touch.start.is_some_and(|s| s.y < strip_top) && self.touch.moved < 8.0 && !self.touch.long_sent && self.touch.started_at.is_some_and(|t| t.elapsed() < Duration::from_millis(350)) {
                self.send(ToPc::Click { button: "left".into() });
                platform::haptic(0);
            }
            self.touch.start = None;
            self.touch.paging = false;
        }
        // ---- the pager glides to its page
        if !self.touch.paging {
            let dt = ctx.input(|inp| inp.stable_dt).min(0.05);
            let diff = self.tab as f32 - self.pager;
            if diff.abs() > 0.001 {
                self.pager += diff * (1.0 - (-dt * 16.0).exp());
                ctx.request_repaint();
            } else {
                self.pager = self.tab as f32;
            }
        }
        // ---- the three pages
        for k in 0..3usize {
            let off = (k as f32 - self.pager) * w;
            if off.abs() >= w {
                continue;
            }
            let r = full.translate(vec2(off, 0.0));
            let mut child = ui.new_child(UiBuilder::new().max_rect(r));
            child.set_clip_rect(r.intersect(full));
            let inner = Rect::from_min_max(r.min + vec2(14.0, 8.0), r.max - vec2(14.0, 8.0));
            match k {
                0 => self.pad_page(&mut child, r, inner, i),
                1 => self.control_page(&mut child, inner),
                _ => self.stats_page(&mut child, inner),
            }
        }
        // ---- the dots
        let dots = Pos2::new(full.center().x, full.bottom() - 9.0);
        for k in 0..3 {
            let near = (1.0 - (k as f32 - self.pager).abs()).clamp(0.0, 1.0);
            ui.painter().circle_filled(dots + vec2((k as f32 - 1.0) * 10.0, 0.0), 2.0 + near * 1.6, mix(p.raised, p.accent, near));
        }
        // ---- the top: which PC, and the way back
        let status = match &self.live {
            Some(l) if l.connected => (self.store.pcs.get(i).map(|p| p.name.clone()).unwrap_or_default(), p.good),
            Some(l) if l.error.is_some() => ("Reconnecting…".to_string(), p.warn),
            _ => ("Connecting…".to_string(), p.muted),
        };
        ui.painter().circle_filled(Pos2::new(full.center().x - 4.0 - status.0.chars().count() as f32 * 3.2, full.top() + 17.0), 3.0, status.1);
        ui.painter().text(Pos2::new(full.center().x, full.top() + 17.0), Align2::CENTER_CENTER, &status.0, medium(11.0), p.muted);
        // back: a button in the top-left of the first page; the other pages swipe back
        let back = Rect::from_center_size(Pos2::new(full.left() + 38.0, full.top() + 32.0), vec2(30.0, 30.0));
        if self.tab == 0 {
            let resp = ui.interact(back, ui.scope_id().with("pc-back"), Sense::click());
            if self.touch.start.is_none() || resp.is_pointer_button_down_on() {
                ui.painter().circle_filled(back.center(), 14.0, mix(p.raised, p.text, if resp.is_pointer_button_down_on() { 0.14 } else { 0.0 }));
                ui.painter().text(back.center(), Align2::CENTER_CENTER, ic::BACK, icon_font(14.0, false), p.muted);
            }
            if resp.clicked() {
                self.leave_pc();
            }
        }
    }

    fn leave_pc(&mut self) {
        self.live = None;
        self.page = Page::Home;
        self.last_probe = Instant::now() - Duration::from_secs(10);
        self.last_theme = Instant::now() - Duration::from_secs(10);
    }

    /// The first page: a touchpad, or the gyro mouse.
    fn pad_page(&mut self, ui: &mut Ui, full: Rect, inner: Rect, _i: usize) {
        let p = pal();
        let gyro = self.set.cursor == "gyro";
        let strip = Rect::from_min_max(Pos2::new(full.left() + 36.0, full.bottom() - 52.0), Pos2::new(full.right() - 36.0, full.bottom() - 22.0));
        // the pad
        let pad = Rect::from_min_max(Pos2::new(inner.left() + 10.0, full.top() + 44.0), Pos2::new(inner.right() - 10.0, strip.top() - 6.0));
        ui.painter().rect_filled(pad, 28, p.card);
        if !gyro {
            ui.painter().text(pad.center(), Align2::CENTER_CENTER, "Touchpad\ntap · hold for right click", FontId::proportional(11.0), mix(p.card, p.muted, 0.8));
        } else {
            self.gyro_pad(ui, pad);
        }
        // the click buttons
        let half = (strip.width() - 6.0) / 2.0;
        for (n, (label, btn)) in [("Left", "left"), ("Right", "right")].into_iter().enumerate() {
            let r = Rect::from_min_size(strip.min + vec2((half + 6.0) * n as f32, 0.0), vec2(half, strip.height()));
            let resp = ui.interact(r, ui.scope_id().with(("click", btn)), Sense::click());
            paint_button(ui, r, &resp, None, label, Look::Tonal, p);
            if resp.clicked() {
                self.send(ToPc::Click { button: btn.into() });
                platform::haptic(0);
            }
        }
    }

    /// The gyro mouse: the pointer follows the hand, a pinch clicks.
    fn gyro_pad(&mut self, ui: &mut Ui, pad: Rect) {
        let p = pal();
        let ctx = ui.ctx().clone();
        let m = motion::poll();
        if self.gyro_live {
            const K: f32 = 2600.0;
            let mut dx = -m.yaw * K * self.set.speed;
            let mut dy = -m.pitch * K * self.set.speed;
            if self.set.invert_x {
                dx = -dx;
            }
            if self.set.invert_y {
                dy = -dy;
            }
            if dx.abs() > 0.2 || dy.abs() > 0.2 {
                self.send(ToPc::PointerBy { dx: dx as f64, dy: dy as f64 });
            }
            if m.pinches > 0 && self.set.pinch {
                let (btn, what) = match m.pinches {
                    1 => (Some("left"), "Click"),
                    3 => (Some("right"), "Right click"),
                    _ => (None, "Pinch"),
                };
                if let Some(b) = btn {
                    self.send(ToPc::Click { button: b.into() });
                    platform::haptic(1);
                }
                self.last_pinches = Some((m.pinches, Instant::now()));
                let _ = what;
            }
        }
        // the middle: tap to pause the pointer (so the arm can rest)
        let c = pad.center();
        let resp = ui.interact(Rect::from_center_size(c, vec2(pad.width() * 0.8, pad.height() * 0.8)), ui.scope_id().with("gyro-toggle"), Sense::click());
        if resp.clicked() {
            self.gyro_live = !self.gyro_live;
            platform::haptic(0);
        }
        let live_t = ctx.animate_bool_with_time(resp.id.with("live"), self.gyro_live, 0.18);
        let flash = self.last_pinches.map(|(_, t)| (1.0 - t.elapsed().as_secs_f32() / 0.4).clamp(0.0, 1.0)).unwrap_or(0.0);
        ui.painter().circle_filled(c, 26.0 + flash * 8.0, mix(p.raised, p.accent, live_t * 0.25 + flash * 0.6));
        ui.painter().circle_stroke(c, 26.0 + flash * 8.0, Stroke::new(2.0, mix(p.muted, p.accent, live_t)));
        ui.painter().text(c, Align2::CENTER_CENTER, ic::MOUSE, icon_font(22.0, false), mix(p.muted, p.text, live_t));
        let hint = if !self.gyro_ok {
            "No gyro sensor found".to_string()
        } else if !self.gyro_live {
            "Paused · tap to move".to_string()
        } else if self.set.pinch {
            match self.last_pinches {
                Some((1, t)) if t.elapsed() < Duration::from_millis(900) => "Click".into(),
                Some((3, t)) if t.elapsed() < Duration::from_millis(900) => "Right click".into(),
                Some((n, t)) if t.elapsed() < Duration::from_millis(900) => format!("{n}× pinch"),
                _ => "Move your hand · pinch".into(),
            }
        } else {
            "Move your hand".into()
        };
        ui.painter().text(Pos2::new(c.x, pad.bottom() - 16.0), Align2::CENTER_CENTER, hint, FontId::proportional(11.0), p.muted);
        if self.gyro_live {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else if flash > 0.0 {
            ctx.request_repaint();
        }
        if flash > 0.0 {
            ctx.request_repaint();
        }
    }

    /// The second page: what you do from a distance.
    fn control_page(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let ctx = ui.ctx().clone();
        let w = inner.width();
        // the rotary bezel (or two-finger scroll) sets the volume
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.5 {
            if let Some(l) = self.live.as_mut() {
                l.volume = (l.volume + scroll * 0.0025).clamp(0.0, 1.0);
                let _ = l.session.to_pc.send(ToPc::Action { name: "volume".into(), arg: Some(format!("{:.0}", l.volume * 100.0)) });
            }
        }
        let vol = self.live.as_ref().map_or(0.0, |l| l.volume);
        let top = inner.top() + 26.0;
        // the volume bar
        let bar = Rect::from_min_size(Pos2::new(inner.left() + 22.0, top), vec2(w - 44.0, 8.0));
        ui.painter().rect_filled(bar, 4, p.raised);
        ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * vol.clamp(0.0, 1.0), bar.height())), 4, p.accent);
        let actions: [(&str, &str, &str, Option<String>); 9] = [
            (ic::LOCK, "Lock", "lock", None),
            (ic::MOON, "Sleep", "suspend", None),
            (ic::MUTE, "Mute", "mute", None),
            (ic::PREV, "Prev", "media_prev", None),
            (ic::PLAY, "Play", "media_play_pause", None),
            (ic::NEXT, "Next", "media_next", None),
            (ic::MINUS, "Vol −", "volume_down", None),
            (ic::SPEAKER, "Vol +", "volume_up", None),
            (ic::GRID, "Overview", "dispatch", Some("{\"type\":\"toggle_overview\"}".to_string())),
        ];
        let cell = (w - 12.0) / 3.0;
        let h = 46.0;
        let mut send = None;
        for (n, (icon, label, name, arg)) in actions.iter().enumerate() {
            let (cx, cy) = (n % 3, n / 3);
            let r = Rect::from_min_size(Pos2::new(inner.left() + cx as f32 * (cell + 6.0), top + 20.0 + cy as f32 * (h + 6.0)), vec2(cell, h));
            let resp = ui.interact(r, ui.scope_id().with(("act", n)), Sense::click());
            let down = ctx.animate_bool_with_time(resp.id.with("down"), resp.is_pointer_button_down_on(), 0.08);
            let r2 = r.shrink(down * 2.0);
            ui.painter().rect_filled(r2, 16, mix(p.card, p.text, down * 0.12));
            ui.painter().text(Pos2::new(r2.center().x, r2.top() + 17.0), Align2::CENTER_CENTER, *icon, icon_font(18.0, false), p.accent);
            ui.painter().text(Pos2::new(r2.center().x, r2.bottom() - 10.0), Align2::CENTER_CENTER, *label, FontId::proportional(10.0), p.muted);
            if resp.clicked() {
                send = Some((name.to_string(), arg.clone()));
            }
        }
        if let Some((name, arg)) = send {
            platform::haptic(0);
            self.send(ToPc::Action { name, arg });
        }
    }

    /// The third page: how the PC is doing.
    fn stats_page(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let Some(s) = self.live.as_ref().and_then(|l| l.stats.clone()) else {
            ui.painter().text(inner.center(), Align2::CENTER_CENTER, "Reading the numbers…", FontId::proportional(12.0), p.muted);
            return;
        };
        let c = Pos2::new(inner.center().x, inner.top() + 66.0);
        let rad = 40.0;
        use std::f32::consts::PI;
        arc(ui, c, rad, -PI * 1.25, PI * 0.25, Stroke::new(8.0, p.raised));
        let frac = s.cpu.clamp(0.0, 1.0);
        if frac > 0.01 {
            let col = if frac > 0.85 { p.bad } else if frac > 0.6 { p.warn } else { p.accent };
            arc(ui, c, rad, -PI * 1.25, -PI * 1.25 + PI * 1.5 * frac, Stroke::new(8.0, col));
        }
        ui.painter().text(c + vec2(0.0, -4.0), Align2::CENTER_CENTER, format!("{:.0}%", s.cpu * 100.0), semibold(20.0), p.text);
        ui.painter().text(c + vec2(0.0, 14.0), Align2::CENTER_CENTER, "processor", FontId::proportional(10.0), p.muted);
        let mem = s.mem_used as f32 / s.mem_total.max(1) as f32;
        let gb = |b: u64| b as f32 / 1_073_741_824.0;
        let mut y = c.y + rad + 22.0;
        let w = inner.width() - 20.0;
        let x0 = inner.left() + 10.0;
        let mut line = |ui: &mut Ui, label: &str, value: String, frac: Option<f32>, y: &mut f32| {
            ui.painter().text(Pos2::new(x0, *y), Align2::LEFT_CENTER, label, FontId::proportional(11.0), p.muted);
            ui.painter().text(Pos2::new(x0 + w, *y), Align2::RIGHT_CENTER, value, medium(12.0), p.text);
            if let Some(f) = frac {
                let bar = Rect::from_min_size(Pos2::new(x0, *y + 9.0), vec2(w, 4.0));
                ui.painter().rect_filled(bar, 2, p.raised);
                ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), 4.0)), 2, if f > 0.85 { p.bad } else { p.accent });
                *y += 28.0;
            } else {
                *y += 20.0;
            }
        };
        line(ui, "Memory", format!("{:.1} / {:.0} GB", gb(s.mem_used), gb(s.mem_total)), Some(mem), &mut y);
        if let Some(t) = s.temps.iter().map(|t| t.celsius).fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b)))) {
            line(ui, "Temperature", format!("{t:.0}°C"), None, &mut y);
        }
        if let Some(b) = s.battery {
            line(ui, if s.charging { "Battery · charging" } else { "Battery" }, format!("{:.0}%", b * 100.0), None, &mut y);
        }
    }

    // ------------------------------------------------------------ settings

    fn settings(&mut self, ui: &mut Ui, inner: Rect) {
        let mut go = None;
        let theme = match self.set.theme.as_str() {
            "pc" => "Like my PC",
            "custom" => "Your color",
            _ => "Watch",
        };
        let cursor = if self.set.cursor == "gyro" { "Gyro mouse" } else { "Touchpad" };
        let upd = if let Some((v, _)) = &self.update.latest { format!("{v} available") } else { format!("Version {VERSION}") };
        self.column(ui, inner, |app, ui, w| {
            app.back(ui, w, Page::Home, "Settings");
            if row(ui, w, ic::SLIDERS, "Theme", theme, true) {
                go = Some(Page::Theme);
            }
            if row(ui, w, ic::MOUSE, "Cursor", cursor, true) {
                go = Some(Page::Cursor);
            }
            if row(ui, w, ic::UPDATE, "Updates", &upd, true) {
                go = Some(Page::Updates);
            }
        });
        if let Some(g) = go {
            self.page = g;
        }
    }

    fn theme_page(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let before = self.set.clone();
        self.column(ui, inner, |app, ui, w| {
            app.back(ui, w, Page::Settings, "Theme");
            let sel = match app.set.theme.as_str() {
                "custom" => 1,
                "pc" => 2,
                _ => 0,
            };
            if let Some(n) = choice(ui, w, &["Watch", "Color", "PC"], sel) {
                app.set.theme = ["watch", "custom", "pc"][n].into();
            }
            let hint = match app.set.theme.as_str() {
                "custom" => "Pick the accent color",
                "pc" => "Uses the colors of your PC's Oxide theme (open a PC once)",
                _ => "Oxide's colors on black",
            };
            let (rect, _) = ui.allocate_exact_size(vec2(w, if app.set.theme == "pc" { 42.0 } else { 18.0 }), Sense::hover());
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, hint, FontId::proportional(10.5), p.muted);
            if app.set.theme == "custom" {
                for rowk in 0..2 {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for col in 0..4 {
                            let k = rowk * 4 + col;
                            let c = ACCENTS[k].1;
                            let color = Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
                            let (rect, resp) = ui.allocate_exact_size(vec2((w - 18.0) / 4.0, 36.0), Sense::click());
                            let on = app.set.accent == k;
                            ui.painter().circle_filled(rect.center(), 13.0, color);
                            if on {
                                ui.painter().circle_stroke(rect.center(), 17.0, Stroke::new(2.0, p.text));
                            }
                            if resp.clicked() {
                                app.set.accent = k;
                            }
                        }
                    });
                }
            }
        });
        if self.set != before {
            self.set.save();
            self.last_theme = Instant::now() - Duration::from_secs(10);
        }
    }

    fn cursor_page(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let before = self.set.clone();
        let tested = self.test_pinches;
        let mut test_add = 0;
        let m = if self.motion_on { motion::poll() } else { motion::Motion::default() };
        if m.pinches > 0 {
            test_add = m.pinches;
        }
        self.column(ui, inner, |app, ui, w| {
            app.back(ui, w, Page::Settings, "Cursor");
            let sel = if app.set.cursor == "gyro" { 1 } else { 0 };
            if let Some(n) = choice(ui, w, &["Touchpad", "Gyro"], sel) {
                app.set.cursor = ["pad", "gyro"][n].into();
            }
            let hint = if app.set.cursor == "gyro" { "Move your hand to move the pointer,\npinch to click, pinch 3× to right click" } else { "Drag to move, tap to click,\nhold for right click" };
            let (rect, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, hint, FontId::proportional(10.5), p.muted);
            stepper(ui, w, "Speed", &mut app.set.speed, 0.25, 0.5, 3.0, "×");
            toggle(ui, w, "Flip left/right", &mut app.set.invert_x);
            toggle(ui, w, "Flip up/down", &mut app.set.invert_y);
            if app.set.cursor == "gyro" {
                toggle(ui, w, "Click with a pinch", &mut app.set.pinch);
                if app.set.pinch {
                    stepper(ui, w, "Pinch sensitivity (lower = easier)", &mut app.set.pinch_level, 0.2, 0.6, 4.0, "");
                    // test: pinch now and see it counted
                    let (rect, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
                    ui.painter().rect_filled(rect, 20, p.card);
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("Test: {} pinch{} seen", tested, if tested == 1 { "" } else { "es" }), medium(12.0), p.text);
                }
            }
        });
        self.test_pinches += test_add;
        if self.motion_on {
            // the sensors keep their own copy of the sensitivity
            if self.set.pinch != before.pinch || self.set.pinch_level != before.pinch_level {
                motion::enable(true, self.set.pinch, self.set.pinch_level);
            }
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if self.set != before {
            self.set.save();
        }
    }

    fn updates_page(&mut self, ui: &mut Ui, inner: Rect) {
        let p = pal();
        let before = self.set.clone();
        let (state, prog) = platform::update_status();
        let latest = self.update.latest.clone();
        let checking = self.update.rx.is_some();
        let checked = self.update.checked;
        let error = self.update.error.clone();
        let mut check = false;
        let mut install = None;
        self.column(ui, inner, |app, ui, w| {
            app.back(ui, w, Page::Settings, "Updates");
            let (rect, _) = ui.allocate_exact_size(vec2(w, 54.0), Sense::hover());
            ui.painter().rect_filled(rect, 27, p.card);
            ui.painter().text(Pos2::new(rect.center().x, rect.center().y - 8.0), Align2::CENTER_CENTER, format!("Oxide Remote {VERSION}"), semibold(13.0), p.text);
            let sub = match (state, &latest) {
                (1, _) => format!("Downloading… {prog}%"),
                (2, _) => "Installing… confirm if asked".to_string(),
                (3, _) => platform::update_error(),
                (4, _) => "Installed".to_string(),
                (_, Some((v, _))) => format!("{v} is available"),
                _ if checking => "Looking…".to_string(),
                _ if error.is_some() => error.clone().unwrap_or_default(),
                _ if checked => "You have the newest version".to_string(),
                _ => "Not checked yet".to_string(),
            };
            ui.painter().text(Pos2::new(rect.center().x, rect.center().y + 10.0), Align2::CENTER_CENTER, sub, FontId::proportional(10.5), p.muted);
            if state == 1 {
                let bar = Rect::from_min_size(Pos2::new(rect.left() + 24.0, rect.bottom() - 7.0), vec2(rect.width() - 48.0, 3.0));
                ui.painter().rect_filled(bar, 1, p.raised);
                ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * prog as f32 / 100.0, 3.0)), 1, p.accent);
            }
            if let Some((_, url)) = &latest {
                if state != 1 && button(ui, vec2(w, 38.0), Some(ic::UPDATE), "Update now", Look::Filled) {
                    install = Some(url.clone());
                }
            } else if !checking && button(ui, vec2(w, 38.0), Some(ic::REFRESH), "Check", Look::Tonal) {
                check = true;
            }
            toggle(ui, w, "Check by itself", &mut app.set.auto_update);
        });
        if check {
            self.check_update();
        }
        if let Some(url) = install {
            platform::install_update(&url);
        }
        if state == 1 || state == 2 || checking {
            ui.ctx().request_repaint_after(Duration::from_millis(150));
        }
        if self.set != before {
            self.set.save();
        }
    }
}
