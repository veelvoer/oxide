//! The Oxide Remote app: your PCs, pairing, and per PC the screen (also full screen), the numbers and the controls.
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{mpsc::Receiver, Arc, Mutex},
    time::{Duration, Instant},
};

use egui::{vec2, Align2, Color32, Pos2, Rect, RichText, Sense, Ui, Vec2};
use oxide_remote_proto::{Announce, PcInfo, Stats, ToPc, ToPhone};

use crate::{
    net::{self, Event, PairEvent, Route, Session},
    platform,
    store::Store,
    theme::*,
    ui::{self, ic, tap, Kind},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
// the releases feed on the website: unlike the API it has no 60-requests-an-hour limit per address
const RELEASES: &str = "https://github.com/veelvoer/oxide/releases.atom";
const DOWNLOAD: &str = "https://github.com/veelvoer/oxide/releases/download";
const SOURCE: &str = "https://github.com/veelvoer/oxide";

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Screen,
    /// touchpad and quick actions, no picture
    Remote,
    Stats,
    Control,
    /// talk to the PC's assistant
    Assistant,
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Home,
    Add,
    Settings,
    Pc(usize),
}

enum Confirm {
    Forget(usize),
    Power(&'static str),
}

struct Pairing {
    host: String,
    events: Receiver<PairEvent>,
    code_tx: std::sync::mpsc::Sender<String>,
    pc_name: Option<String>,
    code: String,
    sent: bool,
}

struct Live {
    session: Session,
    connected: Option<String>,
    route: Option<Route>,
    error: Option<String>,
    stats: Option<Stats>,
    info: Option<PcInfo>,
    cpu_hist: VecDeque<f32>,
    mem_hist: VecDeque<f32>,
    net_hist: VecDeque<f32>,
    texture: Option<egui::TextureHandle>,
    full: (u32, u32),
    fps: f32,
    last_frame: Option<Instant>,
    kbps: f32,
    /// what the PC streams now: (max width, quality, fps)
    streaming: Option<(u32, u8, u8)>,
    last_stats: Instant,
    clipboard: String,
    text: String,
    open: String,
    volume: f32,
    /// the conversation with the PC's assistant: (mine, text)
    chat: Vec<(bool, String)>,
    ask: String,
    ask_busy: bool,
    /// the assistant wants a yes/no before doing something: (id, what, risky)
    ask_confirm: Option<(u64, String, bool)>,
    /// the tool the assistant is using now
    ask_tool: String,
}

/// The newest app version on GitHub: (version, APK address).
#[derive(Default)]
struct Update {
    rx: Option<Receiver<Result<Option<(String, String)>, String>>>,
    latest: Option<(String, String)>,
    checked: bool,
    error: Option<String>,
}

/// The touchpad's memory between frames.
#[derive(Default, Clone)]
struct PadState {
    two: bool,
    three: bool,
    swipe: Vec2,
    moved: f32,
    holding: bool,
    press_start: Option<f64>,
    touch_time: f64,
    dragging: bool,
}

/// Full-screen view: zoom and the toolbar.
struct Full {
    zoom: f32,
    pan: Vec2,
    bar_until: Instant,
    typing: bool,
    text: String,
}

pub struct App {
    store: Store,
    page: Page,
    tab: Tab,
    scanning: Option<Receiver<Vec<(Announce, String)>>>,
    found: Vec<(Announce, String)>,
    manual_host: String,
    pairing: Option<Pairing>,
    pair_error: Option<String>,
    /// per PC: None = offline; missing = not checked yet
    online: Arc<Mutex<HashMap<String, Option<Route>>>>,
    /// PCs found at another local address (another network): remembered on the next frame
    new_hosts: Arc<Mutex<HashMap<String, String>>>,
    last_probe: Instant,
    live: Option<Live>,
    toast: Option<(String, Instant)>,
    ctx: egui::Context,
    /// system bars and keyboard in points: top, bottom, left, right, keyboard
    insets: [f32; 5],
    last_platform: Instant,
    /// waiting for the phone's fingerprint/PIN before the PC is unlocked
    unlocking: bool,
    last_theme: Instant,
    pal: Pal,
    full: Option<Full>,
    confirm: Option<Confirm>,
    sheet: Option<usize>,
    update: Update,
    last_view: Option<(Page, Tab)>,
    epoch: u64,
}

fn human_bytes(b: u64) -> String {
    let b = b as f64;
    if b >= 1e12 {
        format!("{:.1} TB", b / 1e12)
    } else if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", b / 1e3)
    }
}

fn rate(b: u64) -> String {
    let b = b as f64 * 8.0;
    if b >= 1e9 {
        format!("{:.1} Gb/s", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} Mb/s", b / 1e6)
    } else {
        format!("{:.0} kb/s", b / 1e3)
    }
}

fn push(h: &mut VecDeque<f32>, v: f32) {
    h.push_back(v);
    while h.len() > 60 {
        h.pop_front();
    }
}

fn semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim_start_matches('v').split('.').map(|n| n.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// The newest phone release in the releases feed: tags are "phone-vX.Y.Z", the file "OxideRemote-X.Y.Z.apk".
fn newest_apk(feed: &str) -> Option<(String, String)> {
    let mut best: Option<(String, String)> = None;
    for part in feed.split("/releases/tag/").skip(1) {
        let tag: String = part.chars().take_while(|c| !matches!(c, '"' | '<' | '&' | ' ')).collect();
        let Some(v) = tag.strip_prefix("phone-v") else { continue };
        if best.as_ref().is_none_or(|(b, _)| semver(v) > semver(b)) {
            best = Some((v.to_string(), format!("{DOWNLOAD}/{tag}/OxideRemote-{v}.apk")));
        }
    }
    best
}

impl App {
    pub fn new(ctx: &egui::Context, dir: PathBuf) -> Self {
        fonts(ctx);
        let mut store = Store::load(dir);
        // the name PCs show for this phone: the phone's own name rather than "Phone"
        if store.phone_name == "Phone" {
            if let Some(n) = platform::device_name() {
                store.phone_name = n;
                store.save();
            }
        }
        let mut a = Self {
            store,
            page: Page::Home,
            tab: Tab::Screen,
            scanning: None,
            found: vec![],
            manual_host: String::new(),
            pairing: None,
            pair_error: None,
            online: Default::default(),
            new_hosts: Default::default(),
            last_probe: Instant::now() - Duration::from_secs(60),
            live: None,
            toast: None,
            ctx: ctx.clone(),
            insets: [0.0; 5],
            last_platform: Instant::now() - Duration::from_secs(10),
            unlocking: false,
            last_theme: Instant::now() - Duration::from_secs(10),
            pal: Pal::oxide(true),
            full: None,
            confirm: None,
            sheet: None,
            update: Update::default(),
            last_view: None,
            epoch: 0,
        };
        a.refresh_theme();
        // wireless debugging turns itself off on another network or after a restart: switch it back on
        if a.store.show_on_pc && platform::can_show_on_pc() {
            platform::set_show_on_pc(true);
        }
        if a.store.pcs.is_empty() {
            a.page = Page::Add;
            a.scan();
        }
        a.check_update();
        a
    }

    fn say(&mut self, t: impl Into<String>) {
        self.toast = Some((t.into(), Instant::now()));
    }

    // ------------------------------------------------------------------ phone and theme

    fn refresh_theme(&mut self) {
        let dark = platform::dark().unwrap_or(true);
        let pal = match self.store.theme.as_str() {
            "pc" => self.store.pcs.iter().rev().find_map(|p| Pal::from_pc(&p.palette)).unwrap_or_else(|| Pal::oxide(dark)),
            "oxide" => Pal::oxide(dark),
            _ => Pal::material(dark, &platform::system_colors()).unwrap_or_else(|| Pal::oxide(dark)),
        };
        if pal != self.pal || self.last_theme.elapsed() > Duration::from_secs(3600) {
            platform::light_bars(!pal.dark);
        }
        self.pal = pal;
        apply(&self.ctx, pal);
        self.last_theme = Instant::now();
    }

    fn refresh_platform(&mut self) {
        if self.unlocking {
            match platform::auth_result() {
                2 => {
                    self.unlocking = false;
                    self.send(ToPc::Action { name: "unlock".into(), arg: None });
                    self.toast = Some(("Unlocking the PC…".into(), Instant::now()));
                }
                3 => self.unlocking = false,
                _ => {}
            }
        }
        if self.last_platform.elapsed() > Duration::from_millis(300) {
            let ppp = self.ctx.pixels_per_point().max(0.5);
            let i = platform::insets();
            self.insets = [i[0] as f32 / ppp, i[1] as f32 / ppp, i[2] as f32 / ppp, i[3] as f32 / ppp, i[4] as f32 / ppp];
            self.last_platform = Instant::now();
        }
        if self.last_theme.elapsed() > Duration::from_secs(2) {
            self.refresh_theme();
        }
    }

    fn check_update(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = match platform::http_get(RELEASES) {
                Some(json) => Ok(newest_apk(&json).filter(|(v, _)| semver(v) > semver(VERSION))),
                None => Err("Could not reach GitHub. Check the connection and try again.".to_string()),
            };
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        self.update.rx = Some(rx);
        self.update.error = None;
    }

    // ------------------------------------------------------------------ network

    fn scan(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(net::discover(Duration::from_millis(1800)));
            ctx.request_repaint();
        });
        self.scanning = Some(rx);
    }

    fn probe(&mut self) {
        if self.last_probe.elapsed() < Duration::from_secs(6) {
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
        if i >= self.store.pcs.len() {
            return;
        }
        let ctx = self.ctx.clone();
        let session = net::open(&self.store.pcs[i], &self.store.phone_id, move || ctx.request_repaint());
        self.live = Some(Live {
            session,
            connected: None,
            route: None,
            error: None,
            stats: None,
            info: None,
            cpu_hist: VecDeque::new(),
            mem_hist: VecDeque::new(),
            net_hist: VecDeque::new(),
            texture: None,
            full: (0, 0),
            fps: 0.0,
            last_frame: None,
            kbps: 0.0,
            streaming: None,
            last_stats: Instant::now() - Duration::from_secs(5),
            clipboard: String::new(),
            text: String::new(),
            open: String::new(),
            volume: 0.5,
            chat: Vec::new(),
            ask: String::new(),
            ask_busy: false,
            ask_confirm: None,
            ask_tool: String::new(),
        });
        self.page = Page::Pc(i);
        platform::keep_screen_on(true);
    }

    fn close_pc(&mut self) {
        if let Some(l) = &self.live {
            let _ = l.session.to_pc.send(ToPc::ScreenStop);
        }
        self.exit_full();
        self.live = None;
        self.page = Page::Home;
        self.last_probe = Instant::now() - Duration::from_secs(60);
        platform::keep_screen_on(false);
    }

    fn enter_full(&mut self) {
        self.full = Some(Full { zoom: 1.0, pan: Vec2::ZERO, bar_until: Instant::now() + Duration::from_secs(3), typing: false, text: String::new() });
        platform::set_fullscreen(true);
        platform::haptic(2);
    }

    fn exit_full(&mut self) {
        if self.full.take().is_some() {
            platform::set_fullscreen(false);
        }
    }

    /// Wake a PC: the magic packet from here (works at home), and through every other PC that is on (works away).
    fn wake(&mut self, i: usize) {
        let Some(pc) = self.store.pcs.get(i).cloned() else { return };
        let mut ok = false;
        for m in &pc.macs {
            ok |= oxide_remote_proto::wake(m, pc.subnet_broadcast()).is_ok();
        }
        let online = self.online.lock().unwrap().clone();
        let helpers: Vec<_> = self.store.pcs.iter().filter(|p| p.id != pc.id && online.get(&p.id).is_some_and(|r| r.is_some())).cloned().collect();
        for h in &helpers {
            let msgs = pc.macs.iter().map(|m| ToPc::Action { name: "wake".into(), arg: Some(format!("{m} {}", pc.host)) }).collect();
            let (h, id) = (h.clone(), self.store.phone_id.clone());
            std::thread::spawn(move || {
                let _ = net::one_shot(&h, &id, msgs);
            });
        }
        let via = if helpers.is_empty() { String::new() } else { format!(" (also through {})", helpers.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(", ")) };
        self.say(if ok || !helpers.is_empty() { format!("Wake-up sent to {}{via}", pc.name) } else { "Could not send the wake-up packet".into() });
        self.last_probe = Instant::now() - Duration::from_secs(4);
    }

    // ------------------------------------------------------------------ frame

    pub fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // another page (or tab): its cards arrive again
        let key = (self.page, self.tab);
        if self.last_view != Some(key) {
            self.last_view = Some(key);
            self.epoch += 1;
        }
        ui::begin_frame(&ctx, self.epoch);
        self.refresh_platform();
        self.poll(&ctx);
        let p = pal();
        let full_rect = ui.max_rect();
        ui.painter().rect_filled(full_rect, 0.0, p.bg);

        // the system back gesture
        if ui.input(|i| i.key_pressed(egui::Key::BrowserBack) || i.key_pressed(egui::Key::Escape)) {
            self.back();
        }

        if self.full.is_some() && self.live.as_ref().is_some_and(|l| l.texture.is_some()) {
            self.fullscreen(ui, full_rect);
            self.overlays(&ctx);
            return;
        }

        let [top, bottom, left, right, kb] = self.insets;
        let nav = matches!(self.page, Page::Pc(_));
        let nav_h = if nav { 76.0 + bottom } else { 0.0 };
        let bottom_space = if kb > 0.0 { kb } else if nav { nav_h } else { bottom };
        let inner = Rect::from_min_max(
            Pos2::new(full_rect.left() + left + 16.0, full_rect.top() + top + 4.0),
            Pos2::new(full_rect.right() - right - 16.0, full_rect.bottom() - bottom_space),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| match self.page {
            Page::Home => self.home(ui),
            Page::Add => self.add(ui),
            Page::Settings => self.settings(ui),
            Page::Pc(i) => self.pc(ui, i),
        });
        if nav && kb <= 0.0 {
            let items = [(ic::MONITOR, "Screen"), (ic::MOUSE, "Remote"), (ic::CHART, "Stats"), (ic::SLIDERS, "Control"), (ic::SPARKLE, "Assistant")];
            let sel = match self.tab {
                Tab::Screen => 0,
                Tab::Remote => 1,
                Tab::Stats => 2,
                Tab::Control => 3,
                Tab::Assistant => 4,
            };
            if let Some(n) = ui::nav_bar(ui, &items, sel, bottom) {
                self.tab = [Tab::Screen, Tab::Remote, Tab::Stats, Tab::Control, Tab::Assistant][n];
            }
        }
        if self.page == Page::Home && !self.store.pcs.is_empty() {
            self.fab(&ctx, bottom + 16.0, right + 16.0);
        }
        self.overlays(&ctx);
    }

    fn back(&mut self) {
        if self.confirm.take().is_some() || self.sheet.take().is_some() {
            return;
        }
        if self.full.is_some() {
            self.exit_full();
            return;
        }
        match self.page {
            Page::Pc(_) => self.close_pc(),
            Page::Add if !self.store.pcs.is_empty() => {
                self.pairing = None;
                self.page = Page::Home;
            }
            Page::Settings => self.page = Page::Home,
            _ => {}
        }
    }

    /// Dialogs, the PC sheet and messages.
    fn overlays(&mut self, ctx: &egui::Context) {
        let p = pal();
        if let Some(c) = &self.confirm {
            let (glyph, title, text, yes, danger) = match c {
                Confirm::Forget(i) => (ic::TRASH, "Forget this PC?", format!("{} has to be paired again to use it from this phone.", self.store.pcs.get(*i).map(|p| p.name.as_str()).unwrap_or("The PC")), "Forget", true),
                Confirm::Power("suspend") => (ic::MOON, "Put the PC to sleep?", "Wake it up again with Wake up.".to_string(), "Sleep", false),
                Confirm::Power("reboot") => (ic::RESTART, "Restart the PC?", "Unsaved work on the PC is lost.".to_string(), "Restart", true),
                Confirm::Power(_) => (ic::POWER, "Shut the PC down?", "Unsaved work on the PC is lost.".to_string(), "Shut down", true),
            };
            match ui::dialog(ctx, glyph, title, &text, yes, danger) {
                Some(true) => {
                    platform::haptic(2);
                    match self.confirm.take() {
                        Some(Confirm::Forget(i)) if i < self.store.pcs.len() => {
                            let name = self.store.pcs.remove(i).name;
                            self.store.save();
                            self.say(format!("Forgot {name}"));
                        }
                        Some(Confirm::Power(a)) => self.send(ToPc::Action { name: a.into(), arg: None }),
                        _ => {}
                    }
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }
        if let Some(i) = self.sheet {
            self.pc_sheet(ctx, i);
        }
        if let Some((t, at)) = &self.toast {
            if at.elapsed() < Duration::from_millis(2800) {
                let bottom = self.insets[1] + if matches!(self.page, Page::Pc(_)) && self.full.is_none() { 76.0 } else { 0.0 };
                ui::snackbar(ctx, &t.clone(), bottom);
                ctx.request_repaint_after(Duration::from_millis(200));
            } else {
                self.toast = None;
            }
        }
        let _ = p;
    }

    /// Network and phone events into state.
    fn poll(&mut self, ctx: &egui::Context) {
        // a PC that is on another network now: remember where
        let moved: Vec<(String, String)> = self.new_hosts.lock().unwrap().drain().collect();
        if !moved.is_empty() {
            for (id, host) in moved {
                if let Some(pc) = self.store.pcs.iter_mut().find(|p| p.id == id) {
                    pc.host = host;
                }
            }
            self.store.save();
        }
        if let Some(rx) = &self.scanning {
            if let Ok(found) = rx.try_recv() {
                self.found = found;
                self.scanning = None;
            }
        }
        if let Some(rx) = &self.update.rx {
            if let Ok(r) = rx.try_recv() {
                self.update.rx = None;
                self.update.checked = true;
                match r {
                    Ok(latest) => self.update.latest = latest,
                    Err(e) => self.update.error = Some(e),
                }
            }
        }
        if let Some(p) = &mut self.pairing {
            while let Ok(ev) = p.events.try_recv() {
                match ev {
                    PairEvent::NeedCode { pc_name } => p.pc_name = Some(pc_name),
                    PairEvent::Done(Ok(pc)) => {
                        let name = pc.name.clone();
                        self.store.upsert(pc);
                        self.pairing = None;
                        self.pair_error = None;
                        self.page = Page::Home;
                        self.last_probe = Instant::now() - Duration::from_secs(60);
                        self.toast = Some((format!("Paired with {name}"), Instant::now()));
                        platform::haptic(2);
                        break;
                    }
                    PairEvent::Done(Err(e)) => {
                        self.pair_error = Some(e);
                        self.pairing = None;
                        break;
                    }
                }
            }
        }
        let pc_idx = match self.page {
            Page::Pc(i) => Some(i),
            _ => None,
        };
        let mut save_store = false;
        let Some(l) = &mut self.live else { return };
        while let Ok(ev) = l.session.events.try_recv() {
            match ev {
                Event::Connected { pc_name, route, host } => {
                    if let (Some(h), Some(pc)) = (host, pc_idx.and_then(|i| self.store.pcs.get_mut(i))) {
                        pc.host = h;
                        save_store = true;
                    }
                    l.connected = Some(pc_name);
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
                            save_store = true;
                        }
                    }
                }
                Event::Msg(ToPhone::Info(info)) => {
                    if let Some(pc) = pc_idx.and_then(|i| self.store.pcs.get_mut(i)) {
                        if pc.palette != info.palette && !info.palette.is_empty() {
                            pc.palette = info.palette.clone();
                            save_store = true;
                        }
                        if !info.macs.is_empty() && pc.macs != info.macs {
                            pc.macs = info.macs.clone();
                            save_store = true;
                        }
                    }
                    l.info = Some(info);
                }
                Event::Msg(ToPhone::Stats(s)) => {
                    push(&mut l.cpu_hist, s.cpu);
                    push(&mut l.mem_hist, s.mem_used as f32 / s.mem_total.max(1) as f32);
                    push(&mut l.net_hist, (s.net_rx + s.net_tx) as f32);
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
                Event::Msg(ToPhone::Clipboard { text }) => {
                    l.clipboard = text;
                    self.toast = Some(("Got the PC's clipboard".into(), Instant::now()));
                }
                Event::Msg(ToPhone::Assistant { json }) => assistant_event(l, &json),
                Event::Msg(_) => {}
                Event::Frame { seq, rgba, w, h, full, bytes } => {
                    let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
                    match &mut l.texture {
                        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                        None => l.texture = Some(ctx.load_texture("screen", img, egui::TextureOptions::LINEAR)),
                    }
                    l.full = full;
                    if let Some(prev) = l.last_frame {
                        let dt = prev.elapsed().as_secs_f32().max(0.001);
                        l.fps = l.fps * 0.85 + (1.0 / dt) * 0.15;
                        l.kbps = l.kbps * 0.85 + (bytes as f32 * 8.0 / 1000.0 / dt) * 0.15;
                    }
                    l.last_frame = Some(Instant::now());
                    let _ = l.session.to_pc.send(ToPc::FrameAck { seq });
                }
                Event::Closed(e) => {
                    l.error = Some(e);
                    l.connected = None;
                    l.streaming = None;
                }
            }
        }
        // numbers once a second while they are shown
        if l.connected.is_some() && self.full.is_none() && self.tab != Tab::Screen && l.last_stats.elapsed() > Duration::from_secs(1) {
            l.last_stats = Instant::now();
            let _ = l.session.to_pc.send(ToPc::Stats);
            ctx.request_repaint_after(Duration::from_millis(1000));
        }
        // the screen streams only while it is visible; sharper at home and in full screen
        let visible = l.connected.is_some() && (self.tab == Tab::Screen || self.full.is_some());
        let want = visible.then(|| {
            let away = l.route == Some(Route::Away);
            let w = if self.full.is_some() { 1920 } else { 1280 };
            let q = self.store.quality.clamp(20, 90);
            if away {
                (w.min(1280), q.min(55), 20)
            } else {
                (w, q, 30)
            }
        });
        if want != l.streaming {
            if l.streaming.is_some() {
                let _ = l.session.to_pc.send(ToPc::ScreenStop);
            }
            if let Some((max_w, quality, fps)) = want {
                let _ = l.session.to_pc.send(ToPc::ScreenStart { max_w, quality, fps, output: None });
            }
            l.streaming = want;
        }
        if save_store {
            self.store.save();
        }
    }

    fn send(&self, m: ToPc) {
        if let Some(l) = &self.live {
            let _ = l.session.to_pc.send(m);
        }
    }

    // ------------------------------------------------------------------ home

    fn fab(&mut self, ctx: &egui::Context, bottom: f32, right: f32) {
        let p = pal();
        egui::Area::new(egui::Id::unique("fab")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, vec2(-right, -bottom)).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(132.0, 56.0), Sense::click());
            ui.painter().rect_filled(rect.translate(vec2(0.0, 2.0)), 18.0, Color32::from_black_alpha(if p.dark { 60 } else { 25 }));
            ui.painter().rect_filled(rect, 18.0, p.container);
            ui::icon_at(ui, Pos2::new(rect.left() + 30.0, rect.center().y), ic::PLUS, 22.0, p.on_container, false);
            ui.painter().text(Pos2::new(rect.left() + 48.0, rect.center().y), Align2::LEFT_CENTER, "Add PC", medium(15.0), p.on_container);
            if tap(&resp) {
                self.page = Page::Add;
                self.pair_error = None;
                self.scan();
            }
        });
    }

    fn home(&mut self, ui: &mut Ui) {
        let p = pal();
        self.probe();
        ui.ctx().request_repaint_after(Duration::from_secs(2));
        ui.horizontal(|ui| {
            ui.set_min_height(64.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(8.0);
                ui.label(RichText::new("Oxide Remote").font(semibold(28.0)).color(p.text));
                ui.label(RichText::new("Your computers").size(15.0).color(p.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tap(&ui::icon_button(ui, ic::GEAR, p.text, None)) {
                    self.page = Page::Settings;
                }
            });
        });
        ui.add_space(10.0);
        let online = self.online.lock().unwrap().clone();
        let mut open = None;
        let mut wake = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // a new version of the app
            if let Some((v, url)) = self.update.latest.clone() {
                let (state, prog) = platform::update_status();
                egui::Frame::new().fill(p.container).corner_radius(r(24)).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui::icon(ui, ic::UPDATE, 24.0, p.on_container);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(RichText::new(format!("Version {v} is available")).font(medium(16.0)).color(p.on_container));
                            ui.label(RichText::new(match state {
                                1 => format!("Downloading… {prog}%"),
                                2 => "Installing… tap Update if Android asks".into(),
                                3 => platform::update_error(),
                                4 => "Installed. The app restarts".into(),
                                _ => format!("You have {VERSION}"),
                            }).size(13.5).color(p.on_container.gamma_multiply(0.8)));
                            if state == 1 {
                                ui.add_space(6.0);
                                ui::bar(ui, prog as f32 / 100.0, p.accent);
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if state == 1 {
                                ui::spinner(ui, 22.0);
                            } else if tap(&ui::button(ui, Kind::Filled, None, "Update", 0.0)) {
                                platform::install_update(&url);
                            }
                        });
                    });
                });
                ui.add_space(12.0);
                if state == 1 || state == 2 {
                    ui.ctx().request_repaint_after(Duration::from_millis(120));
                }
            }
            if self.store.pcs.is_empty() {
                ui::empty_state(ui, ic::MONITOR, "No PCs yet", "Pair one: it takes a minute.", p.accent);
                ui.vertical_centered(|ui| {
                    if tap(&ui::button(ui, Kind::Filled, Some(ic::PLUS), "Add a PC", 0.0)) {
                        self.page = Page::Add;
                        self.scan();
                    }
                });
            }
            for (i, pc) in self.store.pcs.clone().iter().enumerate() {
                let state = online.get(&pc.id).copied();
                let (status, color) = match state {
                    Some(Some(Route::Home)) => ("Online · home network".to_string(), p.good),
                    Some(Some(Route::Away)) => ("Online · over the internet".to_string(), p.good),
                    Some(None) => ("Offline".to_string(), p.muted),
                    None => ("Checking…".to_string(), p.line),
                };
                let resp = ui::tap_card(ui, egui::Id::unique(("pc", &pc.id)), |ui| {
                    ui.horizontal(|ui| {
                        let (tile, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
                        let on = matches!(state, Some(Some(_)));
                        ui.painter().circle_filled(tile.center(), 26.0, if on { p.container } else { p.raised });
                        ui::icon_at(ui, tile.center(), ic::MONITOR, 26.0, if on { p.on_container } else { p.muted }, on);
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.add_space(4.0);
                            ui.label(RichText::new(&pc.name).font(semibold(18.0)).color(p.text));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let (d, _) = ui.allocate_exact_size(vec2(8.0, 16.0), Sense::hover());
                                ui.painter().circle_filled(d.center(), 4.0, color);
                                ui.label(RichText::new(&status).size(13.5).color(p.muted));
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if tap(&ui::icon_button(ui, ic::DOTS, p.muted, None)) {
                                self.sheet = Some(i);
                            }
                        });
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        match state {
                            Some(Some(_)) => {
                                if tap(&ui::button(ui, Kind::Filled, Some(ic::MONITOR), "Connect", 0.0)) {
                                    open = Some(i);
                                }
                            }
                            _ => {
                                if !pc.macs.is_empty() && tap(&ui::button(ui, Kind::Tonal, Some(ic::LIGHTNING), "Wake up", 0.0)) {
                                    wake = Some(i);
                                }
                                if tap(&ui::button(ui, Kind::Text, None, "Try anyway", 0.0)) {
                                    open = Some(i);
                                }
                            }
                        }
                    });
                });
                if tap(&resp) && matches!(state, Some(Some(_))) {
                    open = Some(i);
                }
                ui.add_space(12.0);
            }
            ui.add_space(96.0);
        });
        if let Some(i) = wake {
            self.wake(i);
        }
        if let Some(i) = open {
            self.open_pc(i);
        }
    }

    /// The actions of one PC, as a sheet from the bottom.
    fn pc_sheet(&mut self, ctx: &egui::Context, i: usize) {
        let p = pal();
        let Some(pc) = self.store.pcs.get(i).cloned() else {
            self.sheet = None;
            return;
        };
        let screen = ctx.content_rect();
        let mut close = false;
        egui::Area::new(egui::Id::unique("sheet-scrim")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(110));
            close |= resp.clicked();
        });
        let bottom = self.insets[1];
        let mut action = None;
        egui::Area::new(egui::Id::unique("sheet")).order(egui::Order::Tooltip).anchor(Align2::CENTER_BOTTOM, Vec2::ZERO).show(ctx, |ui| {
            egui::Frame::new()
                .fill(mix(p.card, p.raised, 0.3))
                .corner_radius(egui::CornerRadius { nw: 28, ne: 28, sw: 0, se: 0 })
                .inner_margin(egui::Margin { left: 20, right: 20, top: 12, bottom: 20 + bottom as i8 })
                .show(ui, |ui| {
                    ui.set_width(screen.width() - 40.0);
                    ui.vertical_centered(|ui| {
                        let (h, _) = ui.allocate_exact_size(vec2(36.0, 4.0), Sense::hover());
                        ui.painter().rect_filled(h, 2.0, p.line);
                    });
                    ui.add_space(8.0);
                    ui.label(RichText::new(&pc.name).font(semibold(20.0)).color(p.text));
                    let away = if pc.remote.is_empty() { "on the same network".to_string() } else { "also over the internet".to_string() };
                    ui.label(RichText::new(format!("{}  ·  {away}", pc.host)).size(13.5).color(p.muted));
                    ui.add_space(8.0);
                    let mut item = |ui: &mut Ui, glyph: &str, text: &str, color: Color32, a: u8| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
                        if resp.is_pointer_button_down_on() {
                            ui.painter().rect_filled(rect, 14.0, p.text.gamma_multiply(0.08));
                        }
                        ui::icon_at(ui, Pos2::new(rect.left() + 20.0, rect.center().y), glyph, 22.0, color, false);
                        ui.painter().text(Pos2::new(rect.left() + 48.0, rect.center().y), Align2::LEFT_CENTER, text, medium(16.0), color);
                        if tap(&resp) {
                            action = Some(a);
                        }
                    };
                    item(ui, ic::MONITOR, "Connect", p.text, 0);
                    if !pc.macs.is_empty() {
                        item(ui, ic::LIGHTNING, "Wake up", p.text, 1);
                    }
                    item(ui, ic::TRASH, "Forget this PC", p.bad, 2);
                });
        });
        match action {
            Some(0) => {
                self.sheet = None;
                self.open_pc(i);
            }
            Some(1) => {
                self.sheet = None;
                self.wake(i);
            }
            Some(2) => {
                self.sheet = None;
                self.confirm = Some(Confirm::Forget(i));
            }
            _ if close => self.sheet = None,
            _ => {}
        }
    }

    // ------------------------------------------------------------------ add a PC

    fn add(&mut self, ui: &mut Ui) {
        let p = pal();
        let can_back = !self.store.pcs.is_empty();
        if ui::top_bar(ui, can_back, "Add a PC", None, |_| {}) {
            self.page = Page::Home;
            self.pairing = None;
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // the code
            if let Some(pr) = &mut self.pairing {
                let mut cancel = false;
                ui::card(ui, |ui| match pr.pc_name.clone() {
                    None => {
                        ui.horizontal(|ui| {
                            ui::spinner(ui, 22.0);
                            ui.label(RichText::new(format!("Contacting {}…", pr.host)).size(16.0).color(p.text));
                        });
                    }
                    Some(name) => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new("Enter the code").font(semibold(22.0)).color(p.text));
                            ui.label(RichText::new(format!("{name} shows it in Settings → Phone")).size(14.5).color(p.muted));
                        });
                        ui.add_space(16.0);
                        let ready = ui::code_boxes(ui, &mut pr.code);
                        ui.add_space(16.0);
                        if pr.sent {
                            ui.vertical_centered(|ui| {
                                ui.horizontal(|ui| {
                                    ui::spinner(ui, 20.0);
                                    ui.label(RichText::new("Checking…").color(p.muted));
                                });
                            });
                        } else {
                            let w = ui.available_width();
                            ui.add_enabled_ui(ready, |ui| ui::button(ui, Kind::Filled, Some(ic::CHECK), "Pair", w));
                            // the sixth digit sends the code
                            if ready {
                                let _ = pr.code_tx.send(pr.code.clone());
                                pr.sent = true;
                            }
                        }
                    }
                });
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if tap(&ui::button(ui, Kind::Text, None, "Cancel", 0.0)) {
                        cancel = true;
                    }
                });
                if cancel {
                    self.pairing = None;
                }
                return;
            }
            if let Some(e) = self.pair_error.clone() {
                egui::Frame::new().fill(mix(p.card, p.bad, 0.14)).corner_radius(r(20)).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui::icon(ui, ic::WARNING, 22.0, p.bad);
                        ui.add(egui::Label::new(RichText::new(e).size(15.0).color(p.text)).wrap());
                    });
                });
                ui.add_space(10.0);
            }
            ui::card(ui, |ui| {
                for (n, t) in [(1, "On the PC, open Settings → Phone and switch on \"Let my phone connect\"."), (2, "Press \"Pair a phone\" there, then pick the PC below.")] {
                    ui.horizontal(|ui| {
                        let (c, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                        ui.painter().circle_filled(c.center(), 15.0, p.container);
                        ui.painter().text(c.center(), Align2::CENTER_CENTER, n.to_string(), medium(15.0), p.on_container);
                        ui.add(egui::Label::new(RichText::new(t).size(15.0).color(p.text)).wrap());
                    });
                    if n == 1 {
                        ui.add_space(6.0);
                    }
                }
            });
            let mut start: Option<(String, u16)> = None;
            ui::section(ui, "On this network");
            ui::card(ui, |ui| {
                for (k, (a, host)) in self.found.clone().into_iter().enumerate() {
                    let paired = self.store.pcs.iter().any(|p| p.id == a.id);
                    ui::row(ui, ic::MONITOR, &a.name, &host, |ui| {
                        if tap(&ui::button(ui, Kind::Tonal, None, if paired { "Pair again" } else { "Pair" }, 0.0)) {
                            start = Some((host.clone(), a.port));
                        }
                    });
                    if k + 1 < self.found.len() {
                        ui.add_space(4.0);
                    }
                }
                if self.scanning.is_some() {
                    ui.horizontal(|ui| {
                        ui::spinner(ui, 20.0);
                        ui.label(RichText::new("Looking for PCs…").color(p.muted));
                    });
                } else {
                    if self.found.is_empty() {
                        ui.label(RichText::new("No PC found. Is \"Let my phone connect\" on, and is the phone on the same Wi-Fi?").size(14.5).color(p.muted));
                    }
                    ui.add_space(4.0);
                    if tap(&ui::button(ui, Kind::Text, Some(ic::REFRESH), "Search again", 0.0)) {
                        self.scan();
                    }
                }
            });
            ui::section(ui, "Or by address");
            ui::card(ui, |ui| {
                let w = ui.available_width() - 96.0;
                ui.horizontal(|ui| {
                    ui::field(ui, &mut self.manual_host, "192.168.1.20", w);
                    if tap(&ui::button(ui, Kind::Filled, None, "Pair", 86.0)) && !self.manual_host.trim().is_empty() {
                        let (h, port) = match self.manual_host.trim().rsplit_once(':') {
                            Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
                            _ => (self.manual_host.trim().to_string(), oxide_remote_proto::TCP_PORT),
                        };
                        start = Some((h, port));
                    }
                });
            });
            if let Some((host, port)) = start {
                let (events, code_tx) = net::pair(host.clone(), port, self.store.phone_id.clone(), self.store.phone_name.clone());
                self.pairing = Some(Pairing { host, events, code_tx, pc_name: None, code: String::new(), sent: false });
                self.pair_error = None;
            }
            ui.add_space(24.0);
        });
    }

    // ------------------------------------------------------------------ settings

    fn settings(&mut self, ui: &mut Ui) {
        let p = pal();
        if ui::top_bar(ui, true, "Settings", None, |_| {}) {
            self.page = Page::Home;
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui::section(ui, "Look");
            ui::card(ui, |ui| {
                ui.label(RichText::new("Colors").font(medium(16.0)).color(p.text));
                ui.label(RichText::new(match self.store.theme.as_str() {
                    "pc" => "The colors of your PC's Oxide theme.",
                    "oxide" => "Oxide's own colors, dark or light like the phone.",
                    _ => "Your phone's colors and its dark or light mode.",
                }).size(13.5).color(p.muted));
                ui.add_space(8.0);
                let mut sel = match self.store.theme.as_str() {
                    "pc" => 1,
                    "oxide" => 2,
                    _ => 0,
                };
                if ui::segmented(ui, &[(ic::PHONE, "Phone"), (ic::MONITOR, "My PC"), (ic::GRID, "Oxide")], &mut sel) {
                    self.store.theme = ["phone", "pc", "oxide"][sel].into();
                    self.store.save();
                    self.refresh_theme();
                }
            });
            ui::section(ui, "Screen");
            ui::card(ui, |ui| {
                ui.label(RichText::new("Picture").font(medium(16.0)).color(p.text));
                ui.label(RichText::new("Smooth uses less data; sharp shows small text better.").size(13.5).color(p.muted));
                ui.add_space(8.0);
                let mut q = match self.store.quality {
                    0..=49 => 0,
                    50..=69 => 1,
                    _ => 2,
                };
                if ui::segmented(ui, &[(ic::WAVE, "Smooth"), (ic::GAUGE, "Balanced"), (ic::SEARCH, "Sharp")], &mut q) {
                    self.store.quality = [40, 60, 80][q];
                    self.store.save();
                }
                ui.add_space(14.0);
                ui.label(RichText::new("Touch").font(medium(16.0)).color(p.text));
                ui.label(RichText::new("Touchpad moves the pointer like a laptop's; Tap clicks where you touch.").size(13.5).color(p.muted));
                ui.add_space(8.0);
                let mut t = if self.store.touchpad { 0 } else { 1 };
                if ui::segmented(ui, &[(ic::MOUSE, "Touchpad"), (ic::TAP, "Tap")], &mut t) {
                    self.store.touchpad = t == 0;
                    self.store.save();
                }
            });
            ui::section(ui, "On my PCs");
            ui::card(ui, |ui| {
                let allowed = platform::can_show_on_pc();
                let mut on = self.store.show_on_pc && allowed;
                let sub = if allowed {
                    "Super+P on the PC shows this screen, also while the phone is locked in your pocket."
                } else {
                    "Once: connect the phone with USB and choose Settings > Phone > Set up phone screen on the PC."
                };
                ui.label(RichText::new("Show this phone on my PCs").font(medium(16.0)).color(p.text));
                ui.add(egui::Label::new(RichText::new(sub).size(13.5).color(p.muted)).wrap());
                ui.add_space(6.0);
                ui.add_enabled_ui(allowed, |ui| {
                    let mut sel = if on { 1 } else { 0 };
                    if ui::segmented(ui, &[(ic::CLOSE, "Off"), (ic::MONITOR, "On")], &mut sel) {
                        on = sel == 1;
                    }
                });
                if allowed && on != self.store.show_on_pc {
                    self.store.show_on_pc = on;
                    self.store.save();
                    if !platform::set_show_on_pc(on) {
                        self.toast = Some(("Could not switch wireless debugging".into(), Instant::now()));
                    }
                }
            });
            ui::section(ui, "My watch");
            ui::card(ui, |ui| {
                let state = platform::watch_bridge_state();
                ui.label(RichText::new("Let my watch connect").font(medium(16.0)).color(p.text));
                ui.add(egui::Label::new(RichText::new(match state {
                    1 => "On: the watch reaches your PCs through this phone over Bluetooth, so it needs no Wi-Fi.",
                    2 => "Allow Bluetooth for this app when Android asks.",
                    3 => "This phone has no Bluetooth.",
                    _ => "The watch uses this phone's connection over Bluetooth, which saves the watch's battery.",
                }).size(13.5).color(p.muted)).wrap());
                ui.add_space(6.0);
                let mut sel = if state == 0 { 0 } else { 1 };
                if ui::segmented(ui, &[(ic::CLOSE, "Off"), (ic::MONITOR, "On")], &mut sel) {
                    platform::watch_bridge(sel == 1);
                }
                if state == 1 {
                    ui.add_space(8.0);
                    if let Some((code, left)) = crate::bridge::pairing_code() {
                        ui.label(RichText::new("Type this code on the watch").size(13.5).color(p.muted));
                        ui.label(RichText::new(format!("{} {}", &code[..3], &code[3..])).font(semibold(32.0)).color(p.accent));
                        ui.label(RichText::new(format!("valid for {left} s")).size(12.5).color(p.muted));
                        ui.ctx().request_repaint_after(Duration::from_millis(500));
                    } else if tap(&ui::button(ui, Kind::Tonal, Some(ic::WATCH), "Pair a watch", 0.0)) {
                        crate::bridge::start_pairing();
                    }
                    for w in crate::bridge::watches(&self.store.dir) {
                        ui.add_space(4.0);
                        ui::row(ui, ic::WATCH, &w.name, "Paired", |ui| {
                            if tap(&ui::icon_button(ui, ic::TRASH, p.bad, None)) {
                                crate::bridge::forget_watch(&self.store.dir, &w.token);
                            }
                        });
                    }
                }
            });
            ui::section(ui, "Updates");
            ui::card(ui, |ui| {
                let (state, prog) = platform::update_status();
                let checking = self.update.rx.is_some();
                let sub = if checking {
                    "Checking…".to_string()
                } else if let Some((v, _)) = &self.update.latest {
                    match state {
                        1 => format!("Downloading {v}… {prog}%"),
                        2 => "Confirm the update on the next screen".into(),
                        3 => platform::update_error(),
                        _ => format!("Version {v} is available"),
                    }
                } else if let Some(e) = &self.update.error {
                    e.clone()
                } else if self.update.checked {
                    "You have the newest version".into()
                } else {
                    String::new()
                };
                let latest = self.update.latest.clone();
                ui::row(ui, ic::UPDATE, &format!("Oxide Remote {VERSION}"), &sub, |ui| {
                    if checking || state == 1 {
                        ui::spinner(ui, 22.0);
                    } else if let Some((_, url)) = &latest {
                        if tap(&ui::button(ui, Kind::Filled, None, "Update", 0.0)) {
                            platform::install_update(url);
                        }
                    } else if tap(&ui::button(ui, Kind::Tonal, None, "Check", 0.0)) {
                        self.check_update();
                    }
                });
                if state == 1 || state == 2 || checking {
                    ui.ctx().request_repaint_after(Duration::from_millis(250));
                }
            });
            ui::section(ui, "About");
            ui::card(ui, |ui| {
                ui::row(ui, ic::GLOBE, "Source code", "github.com/veelvoer/oxide", |ui| {
                    if tap(&ui::icon_button(ui, ic::OPEN, p.accent, None)) {
                        platform::open_url(SOURCE);
                    }
                });
                ui.add_space(4.0);
                ui::row(ui, ic::PHONE, "This phone", &format!("Seen by your PCs as \"{}\"", self.store.phone_name), |_| {});
            });
            ui.add_space(24.0);
        });
    }

    // ------------------------------------------------------------------ a PC

    fn pc(&mut self, ui: &mut Ui, i: usize) {
        let p = pal();
        let name = self.store.pcs.get(i).map(|p| p.name.clone()).unwrap_or_default();
        let (status, color) = match &self.live {
            Some(l) if l.connected.is_some() => (
                match l.route {
                    Some(Route::Away) => "Connected over the internet",
                    _ => "Connected",
                }
                .to_string(),
                p.good,
            ),
            Some(l) if l.error.is_some() => ("Not connected".to_string(), p.bad),
            _ => ("Connecting…".to_string(), p.warn),
        };
        let can_full = self.tab == Tab::Screen && self.live.as_ref().is_some_and(|l| l.texture.is_some());
        let mut go_full = false;
        if ui::top_bar(ui, true, &name, Some((&status, color)), |ui| {
            if can_full && tap(&ui::icon_button(ui, ic::EXPAND, p.text, None)) {
                go_full = true;
            }
        }) {
            self.close_pc();
            return;
        }
        if go_full {
            self.enter_full();
        }
        if let Some(e) = self.live.as_ref().and_then(|l| l.error.clone()) {
            ui.add_space(20.0);
            ui::empty_state(ui, ic::WARNING, "Lost the connection", &e, p.bad);
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    let has_mac = self.store.pcs.get(i).is_some_and(|p| !p.macs.is_empty());
                    let w: f32 = if has_mac { 140.0 } else { 0.0 };
                    ui.add_space(((ui.available_width() - if has_mac { 292.0 } else { 140.0 }) / 2.0).max(0.0));
                    if tap(&ui::button(ui, Kind::Filled, Some(ic::REFRESH), "Try again", w.max(140.0))) {
                        self.open_pc(i);
                    }
                    if has_mac && tap(&ui::button(ui, Kind::Tonal, Some(ic::LIGHTNING), "Wake up", 140.0)) {
                        self.wake(i);
                    }
                });
            });
            return;
        }
        match self.tab {
            Tab::Screen => self.screen(ui),
            Tab::Remote => self.remote(ui),
            Tab::Stats => self.stats(ui),
            Tab::Control => self.control(ui),
            Tab::Assistant => self.assistant(ui),
        }
    }

    /// Touch on the picture of the screen → pointer, clicks and scrolling on the PC.
    fn screen_input(&self, ui: &Ui, resp: &egui::Response, img: Rect, touchpad: bool, two_finger_scroll: bool) {
        let Some(l) = &self.live else { return };
        let tx = &l.session.to_pc;
        let full = l.full;
        let (multi, wheel) = ui.input(|i| (i.multi_touch(), i.smooth_scroll_delta));
        if let Some(mt) = multi {
            if two_finger_scroll && mt.translation_delta.y.abs() > 0.5 {
                let _ = tx.send(ToPc::Scroll { dx: 0.0, dy: (-mt.translation_delta.y * 0.6) as f64 });
            }
            return;
        }
        if wheel.y.abs() > 0.5 && resp.hovered() {
            let _ = tx.send(ToPc::Scroll { dx: 0.0, dy: (-wheel.y * 0.4) as f64 });
        }
        if touchpad {
            if resp.dragged() {
                let d = resp.drag_delta();
                let k = full.0 as f32 / img.width().max(1.0) * 0.9;
                let _ = tx.send(ToPc::PointerBy { dx: (d.x * k) as f64, dy: (d.y * k) as f64 });
            }
            if resp.clicked() {
                platform::haptic(0);
                let _ = tx.send(ToPc::Click { button: "left".into() });
            }
            if resp.secondary_clicked() || resp.long_touched() {
                platform::haptic(1);
                let _ = tx.send(ToPc::Click { button: "right".into() });
            }
        } else if let Some(pt) = resp.interact_pointer_pos() {
            let fx = ((pt.x - img.left()) / img.width()) as f64;
            let fy = ((pt.y - img.top()) / img.height()) as f64;
            if resp.drag_started() {
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
                let _ = tx.send(ToPc::ButtonState { button: "left".into(), pressed: true });
            } else if resp.dragged() {
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
            } else if resp.drag_stopped() {
                let _ = tx.send(ToPc::ButtonState { button: "left".into(), pressed: false });
            }
            if resp.clicked() {
                platform::haptic(0);
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
                let _ = tx.send(ToPc::Click { button: "left".into() });
            }
            if resp.long_touched() || resp.secondary_clicked() {
                platform::haptic(1);
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
                let _ = tx.send(ToPc::Click { button: "right".into() });
            }
        }
    }

    fn keys(&self, ui: &mut Ui) {
        egui::ScrollArea::horizontal().id_salt("keys").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let keys: [(Option<&str>, &str, &str, &[&str]); 12] = [
                    (None, "Esc", "Escape", &[]),
                    (None, "Tab", "Tab", &[]),
                    (Some(ic::ENTER), "", "Return", &[]),
                    (Some(ic::BACKSPACE), "", "BackSpace", &[]),
                    (Some(ic::LEFT_ARROW), "", "Left", &[]),
                    (Some(ic::UP), "", "Up", &[]),
                    (Some(ic::DOWN), "", "Down", &[]),
                    (Some(ic::RIGHT), "", "Right", &[]),
                    (Some(ic::COMMAND), "Super", "Super_L", &[]),
                    (None, "Alt+Tab", "Tab", &["alt"]),
                    (None, "Ctrl+C", "c", &["ctrl"]),
                    (None, "Ctrl+V", "v", &["ctrl"]),
                ];
                for (g, lab, key, mods) in keys {
                    if tap(&ui::chip(ui, g, lab, false)) {
                        self.send(ToPc::Key { name: key.into(), mods: mods.iter().map(|m| m.to_string()).collect() });
                    }
                }
            });
        });
    }

    fn screen(&mut self, ui: &mut Ui) {
        let p = pal();
        let touchpad = self.store.touchpad;
        let Some(l) = &self.live else { return };
        let Some(tex) = l.texture.clone() else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui::spinner(ui, 36.0);
                ui.add_space(10.0);
                ui.label(RichText::new(if l.connected.is_some() { "Waiting for the screen…" } else { "Connecting…" }).size(16.0).color(p.muted));
            });
            return;
        };
        let (full, fps, kbps, route) = (l.full, l.fps, l.kbps, l.route);
        let size = tex.size_vec2();
        let avail = ui.available_width();
        let h = (avail * size.y / size.x).min(ui.available_height() - 210.0).max(140.0);
        let w = (h * size.x / size.y).min(avail);
        let h = w * size.y / size.x;
        let (rect, resp) = ui.allocate_exact_size(vec2(avail, h + 8.0), Sense::click_and_drag());
        let img = Rect::from_center_size(rect.center(), vec2(w, h));
        ui.painter().rect_filled(rect, 20.0, p.deep);
        egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), size)).corner_radius(14).paint_at(ui, img);
        self.screen_input(ui, &resp, img, touchpad, true);
        // full screen, over the picture
        let fs = Rect::from_center_size(Pos2::new(rect.right() - 30.0, rect.bottom() - 30.0), vec2(40.0, 40.0));
        let fs_resp = ui.interact(fs, ui.scope_id().with("fs"), Sense::click());
        ui.painter().circle_filled(fs.center(), 20.0, Color32::from_black_alpha(130));
        ui::icon_at(ui, fs.center(), ic::EXPAND, 20.0, Color32::WHITE, false);
        if tap(&fs_resp) {
            self.enter_full();
        }
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!(
                "{}×{}  ·  {:.0} fps  ·  {:.1} Mb/s{}",
                full.0,
                full.1,
                fps,
                kbps / 1000.0,
                if route == Some(Route::Away) { "  ·  over the internet" } else { "" }
            ))
            .size(12.5)
            .color(p.muted),
        );
        ui.add_space(4.0);
        let mut t = if touchpad { 0 } else { 1 };
        if ui::segmented(ui, &[(ic::MOUSE, "Touchpad"), (ic::TAP, "Tap to click")], &mut t) {
            self.store.touchpad = t == 0;
            self.store.save();
        }
        ui.add_space(4.0);
        self.keys(ui);
        let Some(l) = &mut self.live else { return };
        let mut send = None;
        ui.horizontal(|ui| {
            let w = ui.available_width() - 58.0;
            let r = ui::field(ui, &mut l.text, "Type on the PC…", w);
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (tap(&ui::icon_button(ui, ic::SEND, p.on_accent, Some(p.accent))) || enter) && !l.text.is_empty() {
                send = Some((std::mem::take(&mut l.text), enter));
            }
        });
        if let Some((text, enter)) = send {
            self.send(ToPc::Text { text });
            if enter {
                self.send(ToPc::Key { name: "Return".into(), mods: vec![] });
            }
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }

    /// The PC's screen on the whole phone: pinch to zoom, a toolbar at the top that hides itself.
    fn fullscreen(&mut self, ui: &mut Ui, screen: Rect) {
        let touchpad = self.store.touchpad;
        let Some(tex) = self.live.as_ref().and_then(|l| l.texture.clone()) else { return };
        ui.painter().rect_filled(screen, 0.0, Color32::BLACK);
        let size = tex.size_vec2();
        let fit = (screen.width() / size.x).min(screen.height() / size.y);
        let (zoom, pan) = self.full.as_ref().map(|f| (f.zoom, f.pan)).unwrap_or((1.0, Vec2::ZERO));
        let img = Rect::from_center_size(screen.center() + pan, size * fit * zoom);
        let resp = ui.interact(screen, ui.scope_id().with("full-screen"), Sense::click_and_drag());
        ui.painter_at(screen).image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        // pinch: zoom around the fingers; two fingers while zoomed: move the picture
        let multi = ui.input(|i| i.multi_touch());
        let mut scroll_pc = true;
        if let (Some(mt), Some(f)) = (multi, self.full.as_mut()) {
            let z = (f.zoom * mt.zoom_delta).clamp(1.0, 5.0);
            if (z - f.zoom).abs() > 0.001 {
                let c = mt.center_pos - screen.center() - f.pan;
                f.pan -= c * (z / f.zoom - 1.0);
                f.zoom = z;
            }
            if f.zoom > 1.01 {
                f.pan += mt.translation_delta;
                scroll_pc = false;
            }
            let max = (size * fit * f.zoom - screen.size()).max(Vec2::ZERO) / 2.0;
            f.pan = f.pan.clamp(-max, max);
            if f.zoom <= 1.01 {
                f.pan = Vec2::ZERO;
            }
        }
        self.screen_input(ui, &resp, img, touchpad, scroll_pc);

        // toolbar: shown for a moment, and again when the handle at the top is touched
        let p = pal();
        let now = Instant::now();
        let shown = self.full.as_ref().is_some_and(|f| now < f.bar_until || f.typing);
        let t = ui.ctx().animate_bool_with_time(egui::Id::unique("full-bar"), shown, 0.2);
        let handle = Rect::from_center_size(Pos2::new(screen.center().x, screen.top() + 14.0), vec2(120.0, 28.0));
        let h_resp = ui.interact(handle, ui.scope_id().with("full-handle"), Sense::click());
        ui.painter().rect_filled(Rect::from_center_size(Pos2::new(handle.center().x, screen.top() + 8.0), vec2(48.0, 5.0)), 3.0, Color32::from_white_alpha((90.0 * (1.0 - t)) as u8));
        if h_resp.clicked() {
            if let Some(f) = self.full.as_mut() {
                f.bar_until = now + Duration::from_secs(4);
            }
        }
        let mut exit = false;
        let mut toggle_typing = false;
        let mut toggle_mode = false;
        let mut key: Option<(&str, &[&str])> = None;
        if t > 0.01 {
            let bar_w = 360.0f32.min(screen.width() - 24.0);
            let bar = Rect::from_center_size(Pos2::new(screen.center().x, screen.top() + 40.0 - (1.0 - t) * 50.0), vec2(bar_w, 56.0));
            let painter = ui.painter();
            painter.rect_filled(bar, 28.0, Color32::from_rgba_unmultiplied(18, 18, 22, (242.0 * t) as u8));
            let items: [(&str, u8); 6] = [(ic::SHRINK, 0), (ic::KEYBOARD, 1), (if touchpad { ic::MOUSE } else { ic::TAP }, 2), (ic::COMMAND, 3), (ic::TABS, 4), (ic::CLOSE, 5)];
            let w = bar.width() / items.len() as f32;
            for (k, (g, a)) in items.iter().enumerate() {
                let cell = Rect::from_min_size(Pos2::new(bar.left() + w * k as f32, bar.top()), vec2(w, bar.height()));
                let r = ui.interact(cell, ui.scope_id().with(("full-bar", k)), Sense::click());
                let active = (*a == 1 && self.full.as_ref().is_some_and(|f| f.typing)) || (*a == 2 && !touchpad);
                if active {
                    ui.painter().circle_filled(cell.center(), 20.0, p.accent.gamma_multiply(t));
                }
                let label = match a {
                    5 => "Esc",
                    _ => "",
                };
                if label.is_empty() {
                    ui::icon_at(ui, cell.center(), g, 22.0, if active { p.on_accent } else { Color32::WHITE.gamma_multiply(t) }, false);
                } else {
                    ui.painter().text(cell.center(), Align2::CENTER_CENTER, label, medium(15.0), Color32::WHITE.gamma_multiply(t));
                }
                if tap(&r) {
                    if let Some(f) = self.full.as_mut() {
                        f.bar_until = now + Duration::from_secs(4);
                    }
                    match a {
                        0 => exit = true,
                        1 => toggle_typing = true,
                        2 => toggle_mode = true,
                        3 => key = Some(("Super_L", &[])),
                        4 => key = Some(("Tab", &["alt"])),
                        _ => key = Some(("Escape", &[])),
                    }
                }
            }
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
        // typing bar above the keyboard
        let mut send = None;
        if let Some(f) = self.full.as_mut() {
            if f.typing {
                let kb = self.insets[4];
                let bar = Rect::from_min_size(Pos2::new(screen.left() + 12.0, screen.bottom() - kb - 68.0), vec2(screen.width() - 24.0, 56.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
                egui::Frame::new().fill(p.card).corner_radius(r(28)).inner_margin(egui::Margin::symmetric(8, 6)).show(&mut child, |ui| {
                    ui.horizontal(|ui| {
                        let w = ui.available_width() - 52.0;
                        let resp = ui.add(egui::TextEdit::singleline(&mut f.text).hint_text("Type on the PC…").frame(egui::Frame::NONE).font(egui::FontId::proportional(16.0)).desired_width(w));
                        if !resp.has_focus() && f.text.is_empty() {
                            resp.request_focus();
                        }
                        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if tap(&ui::icon_button_sized(ui, ic::SEND, p.on_accent, Some(p.accent), 44.0, 20.0)) || enter {
                            send = Some((std::mem::take(&mut f.text), enter));
                        }
                    });
                });
            }
        }
        if let Some((text, enter)) = send {
            if !text.is_empty() {
                self.send(ToPc::Text { text });
            }
            if enter {
                self.send(ToPc::Key { name: "Return".into(), mods: vec![] });
            }
        }
        if let Some((k, mods)) = key {
            self.send(ToPc::Key { name: k.into(), mods: mods.iter().map(|m| m.to_string()).collect() });
        }
        if toggle_mode {
            self.store.touchpad = !self.store.touchpad;
            self.store.save();
            self.say(if self.store.touchpad { "Touchpad: drag to move, tap to click" } else { "Tap where you want to click" });
        }
        if toggle_typing {
            if let Some(f) = self.full.as_mut() {
                f.typing = !f.typing;
            }
        }
        if exit {
            self.exit_full();
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }

    /// Remote mode: no picture, just a touchpad and the things you reach for most. The pad takes the whole middle of
    /// the screen: one finger moves the pointer (with acceleration), a tap clicks, a long press holds the button for
    /// dragging, two fingers scroll (a two-finger tap right-clicks), three fingers swipe between workspaces (up:
    /// overview).
    fn remote(&mut self, ui: &mut Ui) {
        let p = pal();
        let Some(l) = &self.live else { return };
        let tx = l.session.to_pc.clone();
        let cur = l.stats.as_ref().and_then(|s| s.workspace);
        let act = |n: &str, a: Option<String>| ToPc::Action { name: n.into(), arg: a };
        let disp = |j: &str| ToPc::Action { name: "dispatch".into(), arg: Some(j.into()) };
        let mut sends: Vec<ToPc> = vec![];
        // the workspace strip
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if tap(&ui::icon_button_sized(ui, ic::LEFT_ARROW, p.text, Some(p.raised), 40.0, 18.0)) {
                sends.push(disp("{\"type\":\"workspace_existing\",\"arg\":-1}"));
            }
            let w = ((ui.available_width() - 46.0 - 6.0 * 8.0) / 9.0).floor().max(24.0);
            for n in 1..=9u32 {
                let on = cur == Some(n);
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                let fill = if on { p.accent } else { p.raised };
                ui.painter().rect_filled(rect, if on { 20.0 } else { 12.0 }, fill);
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, n.to_string(), medium(14.0), if on { p.on_accent } else { p.text });
                if tap(&resp) {
                    sends.push(act("workspace", Some(n.to_string())));
                }
            }
            if tap(&ui::icon_button_sized(ui, ic::RIGHT, p.text, Some(p.raised), 40.0, 18.0)) {
                sends.push(disp("{\"type\":\"workspace_existing\",\"arg\":1}"));
            }
        });
        ui.add_space(8.0);
        // the pad: all the height that is left, minus the buttons and quick actions below it
        let below = 52.0 + 8.0 + 8.0 + 44.0 + 8.0 + 2.0 * 68.0 + 36.0;
        let pad_h = (ui.available_height() - below).max(160.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), pad_h), Sense::click_and_drag());
        ui.painter().rect_filled(rect, 28.0, p.card);
        ui.painter().rect_stroke(rect, 28.0, egui::Stroke::new(1.5, p.line), egui::StrokeKind::Inside);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, "touchpad", medium(15.0), p.muted.gamma_multiply(0.5));
        let id = resp.id;
        let (multi, now) = ui.input(|i| (i.multi_touch(), i.time));
        let mut state: PadState = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_default();
        if let Some(mt) = multi {
            state.dragging = false;
            if mt.num_touches >= 3 {
                state.swipe += mt.translation_delta;
                state.three = true;
            } else {
                // two fingers: scroll (natural, like a phone)
                if mt.translation_delta.y.abs() > 0.2 {
                    sends.push(ToPc::Scroll { dx: (-mt.translation_delta.x * 0.4) as f64, dy: (-mt.translation_delta.y * 0.7) as f64 });
                }
                state.two = true;
            }
            state.touch_time = now;
        } else {
            if state.three && state.swipe.length() > 60.0 {
                // the swipe's main direction
                let s = state.swipe;
                if s.x.abs() > s.y.abs() {
                    sends.push(disp(if s.x < 0.0 { "{\"type\":\"workspace_existing\",\"arg\":1}" } else { "{\"type\":\"workspace_existing\",\"arg\":-1}" }));
                } else if s.y < 0.0 {
                    sends.push(disp("{\"type\":\"toggle_overview\"}"));
                }
                platform::haptic(2);
            }
            if state.two && !state.three && state.moved < 12.0 && resp.clicked() {
                sends.push(ToPc::Click { button: "right".into() });
                platform::haptic(1);
            }
            if !ui.input(|i| i.pointer.any_down()) {
                state.three = false;
                state.two = false;
                state.swipe = Vec2::ZERO;
            }
            if resp.dragged() && !state.three && !state.two {
                let d = resp.drag_delta();
                let speed = d.length();
                // a little acceleration: slow moves are precise, fast ones cross the screen
                let k = 1.5 * (1.0 + (speed * 0.06).min(1.6));
                if d != Vec2::ZERO {
                    sends.push(ToPc::PointerBy { dx: (d.x * k) as f64, dy: (d.y * k) as f64 });
                }
                state.moved += speed;
            }
            // a long press holds the button (drag a window, select text); letting go releases it
            if resp.is_pointer_button_down_on() && !state.holding && state.moved < 6.0 && state.press_start.is_some_and(|t| now - t > 0.45) {
                state.holding = true;
                sends.push(ToPc::ButtonState { button: "left".into(), pressed: true });
                platform::haptic(1);
            }
            if resp.drag_started() || (resp.is_pointer_button_down_on() && state.press_start.is_none()) {
                state.press_start = Some(now);
                state.moved = 0.0;
            }
            if !ui.input(|i| i.pointer.any_down()) {
                if state.holding {
                    sends.push(ToPc::ButtonState { button: "left".into(), pressed: false });
                    state.holding = false;
                }
                state.press_start = None;
            }
            if resp.clicked() && !state.two && !state.holding && state.moved < 8.0 && now - state.touch_time > 0.15 {
                sends.push(ToPc::Click { button: "left".into() });
                platform::haptic(0);
            }
        }
        if state.holding {
            ui.painter().rect_stroke(rect, 28.0, egui::Stroke::new(3.0, p.accent), egui::StrokeKind::Inside);
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, state));
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        ui.add_space(8.0);
        // the buttons
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            for (label, btn) in [("Left click", "left"), ("Right click", "right")] {
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
                ui.painter().rect_filled(rect, 26.0, p.container);
                if resp.is_pointer_button_down_on() {
                    ui.painter().rect_filled(rect, 26.0, p.on_container.gamma_multiply(0.12));
                }
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, medium(15.0), p.on_container);
                if resp.drag_started() {
                    sends.push(ToPc::ButtonState { button: btn.into(), pressed: true });
                    platform::haptic(0);
                }
                if resp.drag_stopped() || (resp.clicked() && !resp.dragged()) {
                    sends.push(ToPc::ButtonState { button: btn.into(), pressed: false });
                }
            }
        });
        ui.add_space(8.0);
        // quick actions: three groups, one at a time, in a fixed grid of four (nothing scrolls, nothing is cut off)
        type Quick = (&'static str, &'static str, u8, &'static str);
        let windows: [Quick; 8] = [
            (ic::GRID, "Overview", 0, "{\"type\":\"toggle_overview\"}"),
            (ic::SEARCH, "Apps", 1, "SUPER+r"),
            (ic::KEYBOARD, "Terminal", 1, "SUPER+q"),
            (ic::CLOSE, "Close", 0, "{\"type\":\"close\"}"),
            (ic::WINDOW, "Float", 0, "{\"type\":\"toggle_floating\"}"),
            (ic::EXPAND, "Full", 0, "{\"type\":\"toggle_fullscreen\"}"),
            (ic::TABS, "Alt+Tab", 1, "ALT+Tab"),
            (ic::COMMAND, "Super", 1, "Super_L"),
        ];
        let media: [Quick; 8] = [
            (ic::PREV, "Previous", 2, "media_prev"),
            (ic::PLAY, "Play", 2, "media_play_pause"),
            (ic::NEXT, "Next", 2, "media_next"),
            (ic::MUTE, "Mute", 2, "mute"),
            (ic::MUTE, "Quieter", 2, "volume_down"),
            (ic::SPEAKER, "Louder", 2, "volume_up"),
            ("", "", 9, ""),
            ("", "", 9, ""),
        ];
        let pc: [Quick; 8] = [
            (ic::LOCK, "Lock", 2, "lock"),
            (ic::LOCK, "Unlock", 3, "unlock"),
            (ic::SEND, "Screenshot", 2, "screenshot"),
            (ic::LEFT_ARROW, "Esc", 1, "Escape"),
            (ic::ENTER, "Enter", 1, "Return"),
            ("", "", 9, ""),
            ("", "", 9, ""),
            ("", "", 9, ""),
        ];
        let gid = ui.id().with("quick-group");
        let mut group = ui.data(|d| d.get_temp::<usize>(gid)).unwrap_or(0);
        ui::segmented(ui, &[(ic::GRID, "Windows"), (ic::PLAY, "Media"), (ic::LOCK, "PC")], &mut group);
        ui.data_mut(|d| d.insert_temp(gid, group));
        ui.add_space(8.0);
        let set: &[Quick; 8] = [&windows, &media, &pc][group.min(2)];
        let gap = 8.0;
        let w = (ui.available_width() - 3.0 * gap) / 4.0;
        for row in 0..2 {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(gap, gap);
                for col in 0..4 {
                    let (glyph, label, kind, arg) = set[row * 4 + col];
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 60.0), if kind == 9 { Sense::hover() } else { Sense::click() });
                    if kind == 9 {
                        continue;
                    }
                    let primary = kind == 3;
                    let (bg, fg) = if primary { (p.accent, p.on_accent) } else { (p.raised, p.text) };
                    ui.painter().rect_filled(rect, 18.0, bg);
                    if resp.is_pointer_button_down_on() {
                        ui.painter().rect_filled(rect, 18.0, fg.gamma_multiply(0.12));
                    }
                    ui::icon_at(ui, Pos2::new(rect.center().x, rect.top() + 22.0), glyph, 20.0, fg, false);
                    ui.painter().text(Pos2::new(rect.center().x, rect.bottom() - 15.0), Align2::CENTER_CENTER, label, medium(12.0), fg);
                    if tap(&resp) {
                        match kind {
                            0 => sends.push(disp(arg)),
                            1 => {
                                let (mods, key) = match arg.rsplit_once('+') {
                                    Some((m, k)) => (m.split('+').map(|x| x.to_lowercase()).collect::<Vec<_>>(), k.to_string()),
                                    None => (vec![], arg.to_string()),
                                };
                                sends.push(ToPc::Key { name: key, mods });
                            }
                            // the PC is unlocked only after this phone's own screen-lock check
                            3 => {
                                platform::authenticate("Unlock your PC", "Confirm that it is you");
                                self.unlocking = true;
                            }
                            _ => sends.push(act(arg, None)),
                        }
                    }
                }
            });
        }
        for m in sends {
            let _ = tx.send(m);
        }
    }

    /// Chat with the PC's assistant: it works on the PC; its answer arrives word by word.
    fn assistant(&mut self, ui: &mut Ui) {
        let p = pal();
        let Some(l) = self.live.as_mut() else { return };
        let mut out: Vec<ToPc> = Vec::new();
        let bar_h = 62.0 + if l.ask_confirm.is_some() { 116.0 } else { 0.0 };
        let width = ui.available_width();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height((ui.available_height() - bar_h).max(80.0))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if l.chat.is_empty() {
                    ui::empty_state(ui, ic::SPARKLE, "Your PC's assistant", "Say what it should do: open an app, change a setting, find something. It works on the PC.", p.accent);
                }
                for (mine, text) in &l.chat {
                    ui.add_space(6.0);
                    let layout = if *mine { egui::Layout::right_to_left(egui::Align::TOP) } else { egui::Layout::left_to_right(egui::Align::TOP) };
                    ui.with_layout(layout, |ui| {
                        ui.set_max_width(width * 0.88);
                        let (fill, ink) = if *mine { (p.container, p.on_container) } else { (p.card, p.text) };
                        egui::Frame::new().fill(fill).corner_radius(r(18)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(if text.is_empty() { "…" } else { text }).size(15.0).color(ink)).wrap());
                        });
                    });
                }
                if l.ask_busy && !l.ask_tool.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("{}…", l.ask_tool)).size(13.0).color(p.muted));
                }
                ui.add_space(6.0);
            });
        if let Some((id, what, risky)) = l.ask_confirm.clone() {
            ui::card(ui, |ui| {
                ui.label(RichText::new(if risky { "Allow this? It may be hard to undo." } else { "Allow this?" }).size(13.0).color(if risky { p.warn } else { p.muted }));
                ui.add(egui::Label::new(RichText::new(what).size(15.0).color(p.text)).wrap());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if tap(&ui::button(ui, Kind::Filled, Some(ic::CHECK), "Allow", 0.0)) {
                        out.push(ToPc::AskConfirm { id, allow: true });
                        l.ask_confirm = None;
                    }
                    if tap(&ui::button(ui, Kind::Tonal, Some(ic::CLOSE), "Deny", 0.0)) {
                        out.push(ToPc::AskConfirm { id, allow: false });
                        l.ask_confirm = None;
                    }
                });
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let w = ui.available_width() - 56.0;
            let resp = ui::field(ui, &mut l.ask, "Ask your PC…", w);
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if l.ask_busy {
                if tap(&ui::icon_button_sized(ui, ic::STOP, p.on_accent, Some(p.accent), 48.0, 22.0)) {
                    out.push(ToPc::AskCancel);
                }
            } else if tap(&ui::icon_button_sized(ui, ic::SEND, p.on_accent, Some(p.accent), 48.0, 22.0)) || enter {
                let text = l.ask.trim().to_string();
                if !text.is_empty() {
                    l.ask.clear();
                    l.chat.push((true, text.clone()));
                    l.ask_busy = true;
                    out.push(ToPc::Ask { text });
                }
            }
        });
        for m in out {
            self.send(m);
        }
    }

    fn stats(&mut self, ui: &mut Ui) {
        let p = pal();
        let Some(l) = &self.live else { return };
        let Some(s) = l.stats.clone() else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui::spinner(ui, 36.0);
                ui.add_space(10.0);
                ui.label(RichText::new("Reading the numbers…").size(16.0).color(p.muted));
            });
            return;
        };
        let (cpu_hist, mem_hist, net_hist) = (l.cpu_hist.clone(), l.mem_hist.clone(), l.net_hist.clone());
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            let mem = s.mem_used as f32 / s.mem_total.max(1) as f32;
            ui.columns(2, |cols| {
                ui::card(&mut cols[0], |ui| {
                    ui.vertical_centered(|ui| {
                        ui::ring(ui, s.cpu, p.accent, 120.0, &format!("{:.0}%", s.cpu * 100.0), "processor");
                    });
                });
                ui::card(&mut cols[1], |ui| {
                    ui.vertical_centered(|ui| {
                        ui::ring(ui, mem, if mem > 0.9 { p.bad } else { p.good }, 120.0, &format!("{:.0}%", mem * 100.0), &format!("of {}", human_bytes(s.mem_total)));
                    });
                });
            });
            ui.add_space(10.0);
            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui::icon(ui, ic::CPU, 20.0, p.accent);
                    ui.label(RichText::new("Processor").font(medium(16.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("load {:.2}", s.load[0])).size(13.5).color(p.muted));
                    });
                });
                ui.add_space(4.0);
                ui::spark(ui, &cpu_hist, p.accent, 1.0, 60.0);
                ui.add_space(6.0);
                let n = s.cores.len().max(1);
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
                let bw = rect.width() / n as f32;
                for (k, c) in s.cores.iter().enumerate() {
                    let x = rect.left() + k as f32 * bw;
                    let col = Rect::from_min_max(Pos2::new(x + 1.5, rect.top()), Pos2::new(x + bw - 1.5, rect.bottom()));
                    ui.painter().rect_filled(col, 3.0, p.raised);
                    let mut f = col;
                    f.set_top(col.bottom() - col.height() * c.clamp(0.0, 1.0));
                    ui.painter().rect_filled(f, 3.0, p.accent);
                }
            });
            ui.add_space(10.0);
            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui::icon(ui, ic::MEMORY, 20.0, p.good);
                    ui.label(RichText::new("Memory").font(medium(16.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} of {}", human_bytes(s.mem_used), human_bytes(s.mem_total))).size(13.5).color(p.muted));
                    });
                });
                ui.add_space(4.0);
                ui::spark(ui, &mem_hist, p.good, 1.0, 48.0);
                if s.swap_total > 0 {
                    ui.label(RichText::new(format!("swap {} of {}", human_bytes(s.swap_used), human_bytes(s.swap_total))).size(13.0).color(p.muted));
                }
            });
            ui.add_space(10.0);
            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui::icon(ui, ic::UPDOWN, 20.0, p.warn);
                    ui.label(RichText::new("Network").font(medium(16.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("↓ {}   ↑ {}", rate(s.net_rx), rate(s.net_tx))).size(13.5).color(p.muted));
                    });
                });
                ui.add_space(4.0);
                let max = net_hist.iter().cloned().fold(1.0, f32::max);
                ui::spark(ui, &net_hist, p.warn, max, 48.0);
            });
            if !s.temps.is_empty() || s.gpu.is_some() || s.battery.is_some() {
                ui.add_space(10.0);
                ui::card(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                        let tile = |ui: &mut Ui, glyph: &str, label: &str, value: String, color: Color32| {
                            egui::Frame::new().fill(p.raised).corner_radius(r(16)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        ui::icon(ui, glyph, 15.0, p.muted);
                                        ui.label(RichText::new(label).size(13.0).color(p.muted));
                                    });
                                    ui.label(RichText::new(value).font(semibold(20.0)).color(color));
                                });
                            });
                        };
                        for t in &s.temps {
                            let col = if t.celsius > 85.0 { p.bad } else if t.celsius > 70.0 { p.warn } else { p.text };
                            tile(ui, ic::THERMOMETER, &t.label, format!("{:.0}°C", t.celsius), col);
                        }
                        if let Some(g) = s.gpu {
                            tile(ui, ic::GAUGE, "GPU", format!("{:.0}%", g * 100.0), p.text);
                        }
                        if let Some(b) = s.battery {
                            tile(ui, ic::BATTERY, if s.charging { "Charging" } else { "Battery" }, format!("{b:.0}%"), p.text);
                        }
                    });
                });
            }
            if !s.disks.is_empty() {
                ui.add_space(10.0);
                ui::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui::icon(ui, ic::DISKS, 20.0, p.accent);
                        ui.label(RichText::new("Disks").font(medium(16.0)).color(p.text));
                    });
                    for d in &s.disks {
                        let f = d.used as f32 / d.total.max(1) as f32;
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&d.mount).size(14.5).color(p.text));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("{} free", human_bytes(d.total.saturating_sub(d.used)))).size(13.0).color(p.muted));
                            });
                        });
                        ui::bar(ui, f, if f > 0.9 { p.bad } else { p.accent });
                    }
                });
            }
            ui.add_space(10.0);
            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui::icon(ui, ic::WINDOW, 20.0, p.accent);
                    ui.label(RichText::new("Busiest programs").font(medium(16.0)).color(p.text));
                });
                ui.add_space(2.0);
                for pr in &s.top {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&pr.name).size(14.5).color(p.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(human_bytes(pr.mem)).size(13.0).color(p.muted));
                            ui.label(RichText::new(format!("{:.0}%", pr.cpu * 100.0)).font(egui::FontId::monospace(14.0)).color(p.text));
                        });
                    });
                }
                let up = s.uptime;
                ui.add_space(4.0);
                ui.label(RichText::new(format!("On for {}d {}h {}m", up / 86400, (up / 3600) % 24, (up / 60) % 60)).size(13.0).color(p.muted));
            });
            ui.add_space(20.0);
        });
    }

    fn control(&mut self, ui: &mut Ui) {
        let p = pal();
        let mut sends: Vec<ToPc> = vec![];
        let mut confirm = None;
        let Some(l) = &mut self.live else { return };
        if l.stats.is_none() && l.last_stats.elapsed() > Duration::from_secs(2) {
            l.last_stats = Instant::now();
            sends.push(ToPc::Stats);
        }
        let act = |n: &str, a: Option<String>| ToPc::Action { name: n.into(), arg: a };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui::icon(ui, ic::WAVE, 20.0, p.accent);
                    let title = l.stats.as_ref().and_then(|s| s.media.clone()).unwrap_or_else(|| "Media".into());
                    ui.add(egui::Label::new(RichText::new(title).font(medium(16.0)).color(p.text)).truncate());
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let w = ui.available_width();
                    ui.add_space((w - 64.0 * 3.0 - 32.0) / 2.0);
                    ui.spacing_mut().item_spacing.x = 16.0;
                    if tap(&ui::icon_button_sized(ui, ic::PREV, p.text, Some(p.raised), 56.0, 24.0)) {
                        sends.push(act("media_prev", None));
                    }
                    if tap(&ui::icon_button_sized(ui, ic::PLAY, p.on_accent, Some(p.accent), 72.0, 30.0)) {
                        sends.push(act("media_play_pause", None));
                    }
                    if tap(&ui::icon_button_sized(ui, ic::NEXT, p.text, Some(p.raised), 56.0, 24.0)) {
                        sends.push(act("media_next", None));
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if tap(&ui::icon_button_sized(ui, ic::MUTE, p.muted, None, 40.0, 20.0)) {
                        sends.push(act("mute", None));
                    }
                    ui.spacing_mut().slider_width = ui.available_width() - 48.0;
                    let resp = ui.add(egui::Slider::new(&mut l.volume, 0.0..=1.0).show_value(false).trailing_fill(true));
                    if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                        sends.push(act("volume", Some(format!("{}", (l.volume * 100.0).round()))));
                    }
                    ui::icon(ui, ic::SPEAKER, 20.0, p.muted);
                });
            });
            ui::section(ui, "Workspaces");
            ui::card(ui, |ui| {
                let cur = l.stats.as_ref().and_then(|s| s.workspace);
                let gap = 8.0;
                let w = ((ui.available_width() - gap * 4.0) / 5.0).floor();
                for row in [1..=5u32, 6..=9u32] {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for n in row {
                            let on = cur == Some(n);
                            let (rect, resp) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
                            ui.painter().rect_filled(rect, 16.0, if on { p.accent } else { p.raised });
                            ui.painter().text(rect.center(), Align2::CENTER_CENTER, n.to_string(), semibold(17.0), if on { p.on_accent } else { p.text });
                            if tap(&resp) {
                                sends.push(act("workspace", Some(n.to_string())));
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
            });
            ui::section(ui, "Power");
            ui.columns(2, |cols| {
                let tiles: [(&str, &str, &'static str); 4] = [(ic::LOCK, "Lock", "lock"), (ic::MOON, "Sleep", "suspend"), (ic::RESTART, "Restart", "reboot"), (ic::POWER, "Shut down", "shutdown")];
                for (k, (g, label, a)) in tiles.iter().enumerate() {
                    let ui = &mut cols[k % 2];
                    let danger = *a == "shutdown";
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
                    ui.painter().rect_filled(rect, 20.0, if danger { mix(p.card, p.bad, 0.14) } else { p.card });
                    if resp.is_pointer_button_down_on() {
                        ui.painter().rect_filled(rect, 20.0, p.text.gamma_multiply(0.08));
                    }
                    let fg = if danger { p.bad } else { p.text };
                    ui::icon_at(ui, Pos2::new(rect.left() + 28.0, rect.center().y), g, 22.0, fg, false);
                    ui.painter().text(Pos2::new(rect.left() + 52.0, rect.center().y), Align2::LEFT_CENTER, *label, medium(15.5), fg);
                    if tap(&resp) {
                        if *a == "lock" {
                            sends.push(act("lock", None));
                        } else {
                            confirm = Some(*a);
                        }
                    }
                    ui.add_space(4.0);
                }
            });
            ui::section(ui, "Clipboard");
            ui::card(ui, |ui| {
                ui::field_multi(ui, &mut l.clipboard, "Text to send to the PC's clipboard", 3);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if tap(&ui::button(ui, Kind::Tonal, Some(ic::SEND), "Send", 0.0)) {
                        sends.push(act("clipboard_set", Some(l.clipboard.clone())));
                    }
                    if tap(&ui::button(ui, Kind::Outlined, Some(ic::DOWNLOAD), "From PC", 0.0)) {
                        sends.push(ToPc::ClipboardGet);
                    }
                    if tap(&ui::icon_button(ui, ic::COPY, p.text, None)) {
                        ui.ctx().copy_text(l.clipboard.clone());
                    }
                });
            });
            ui::section(ui, "Open on the PC");
            ui::card(ui, |ui| {
                let w = ui.available_width();
                ui::field(ui, &mut l.open, "An app (firefox) or a web address", w);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if tap(&ui::button(ui, Kind::Filled, Some(ic::OPEN), "Open", 0.0)) && !l.open.trim().is_empty() {
                        let t = l.open.trim().to_string();
                        if t.contains("://") || (t.contains('.') && !t.contains(' ')) {
                            let url = if t.contains("://") { t } else { format!("https://{t}") };
                            sends.push(act("open_url", Some(url)));
                        } else {
                            sends.push(act("run", Some(t)));
                        }
                        l.open.clear();
                    }
                    if tap(&ui::button(ui, Kind::Text, Some(ic::BELL), "As notification", 0.0)) && !l.open.trim().is_empty() {
                        sends.push(act("notify", Some(std::mem::take(&mut l.open))));
                    }
                });
            });
            ui.add_space(24.0);
        });
        for m in sends {
            self.send(m);
        }
        if let Some(c) = confirm {
            self.confirm = Some(Confirm::Power(c));
        }
    }
}

/// One event from the PC's assistant (`oxide-ai`'s JSON line).
fn assistant_event(l: &mut Live, json: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return };
    match v["type"].as_str() {
        Some("restore") => {
            l.chat = v["items"]
                .as_array()
                .map(|a| a.iter().filter_map(|i| Some((i["role"] == "user", i["text"].as_str()?.to_string()))).filter(|(_, t)| !t.trim().is_empty()).collect())
                .unwrap_or_default();
        }
        Some("turn_start") => {
            l.ask_busy = true;
            l.ask_tool.clear();
        }
        Some("assistant_start") => l.chat.push((false, String::new())),
        Some("text_delta") => {
            if let Some(t) = v["text"].as_str() {
                match l.chat.last_mut() {
                    Some((false, s)) => s.push_str(t),
                    _ => l.chat.push((false, t.to_string())),
                }
            }
        }
        Some("tool_start") => l.ask_tool = v["name"].as_str().unwrap_or("").replace('_', " "),
        Some("confirm") => {
            l.ask_confirm = Some((v["id"].as_u64().unwrap_or(0), v["summary"].as_str().unwrap_or("An action").to_string(), v["risk"].as_str().is_some_and(|r| r != "low" && !r.is_empty())));
        }
        Some("done") => {
            l.ask_busy = false;
            l.ask_tool.clear();
            l.ask_confirm = None;
            if l.chat.last().is_some_and(|(m, s)| !m && s.is_empty()) {
                l.chat.pop();
            }
        }
        Some("error") => {
            l.ask_busy = false;
            l.chat.push((false, v["message"].as_str().unwrap_or("Something went wrong.").to_string()));
        }
        _ => {}
    }
}
