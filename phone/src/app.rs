//! The Oxide Remote app: your PCs, pairing, and per PC the screen, the numbers and the controls.
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{mpsc::Receiver, Arc, Mutex},
    time::{Duration, Instant},
};

use egui::{vec2, Align, Color32, Layout, RichText, Sense, Stroke, Ui};
use oxide_remote_proto::{Announce, Stats, ToPc, ToPhone};

use crate::{
    net::{self, Event, PairEvent, Session},
    store::Store,
    theme::*,
};

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Screen,
    Stats,
    Control,
}

enum Page {
    Home,
    Add,
    Pc(usize),
}

struct Pairing {
    host: String,
    events: Receiver<PairEvent>,
    code_tx: std::sync::mpsc::Sender<String>,
    pc_name: Option<String>,
    code: String,
}

struct Live {
    session: Session,
    connected: Option<String>,
    error: Option<String>,
    stats: Option<Stats>,
    cpu_hist: VecDeque<f32>,
    mem_hist: VecDeque<f32>,
    net_hist: VecDeque<f32>,
    texture: Option<egui::TextureHandle>,
    full: (u32, u32),
    fps: f32,
    last_frame: Option<Instant>,
    kbps: f32,
    streaming: bool,
    last_stats: Instant,
    notice: Option<(String, bool, Instant)>,
    clipboard: String,
    text: String,
    volume: f32,
    confirm: Option<&'static str>,
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
    online: Arc<Mutex<HashMap<String, bool>>>,
    last_probe: Instant,
    live: Option<Live>,
    toast: Option<(String, Instant)>,
    ctx: egui::Context,
}

fn human_bytes(b: u64) -> String {
    let b = b as f64;
    if b >= 1e12 { format!("{:.1} TB", b / 1e12) } else if b >= 1e9 { format!("{:.1} GB", b / 1e9) } else if b >= 1e6 { format!("{:.0} MB", b / 1e6) } else { format!("{:.0} KB", b / 1e3) }
}

fn rate(b: u64) -> String {
    let b = b as f64 * 8.0;
    if b >= 1e9 { format!("{:.1} Gb/s", b / 1e9) } else if b >= 1e6 { format!("{:.1} Mb/s", b / 1e6) } else { format!("{:.0} kb/s", b / 1e3) }
}

fn push(h: &mut VecDeque<f32>, v: f32) {
    h.push_back(v);
    while h.len() > 60 {
        h.pop_front();
    }
}

/// A card: rounded surface with a hairline border.
fn card<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new().fill(CARD).stroke(Stroke::new(1.0, LINE)).corner_radius(r(16)).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        f(ui)
    }).inner
}

fn label(ui: &mut Ui, t: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(t.to_uppercase()).size(12.0).color(MUTED).extra_letter_spacing(1.2));
}

fn primary(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).color(INK).strong()).fill(ACCENT).corner_radius(r(22)).min_size(vec2(0.0, 44.0)))
}

fn ghost(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).color(TEXT)).fill(RAISED).stroke(Stroke::new(1.0, LINE)).corner_radius(r(22)).min_size(vec2(0.0, 44.0)))
}

fn danger(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).color(BAD)).fill(RAISED).stroke(Stroke::new(1.0, BAD.gamma_multiply(0.6))).corner_radius(r(22)).min_size(vec2(0.0, 44.0)))
}

/// A small line chart of the last minute.
fn spark(ui: &mut Ui, hist: &VecDeque<f32>, color: Color32, max: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, r(10), DEEP);
    if hist.len() < 2 {
        return;
    }
    let n = 60usize;
    let pts: Vec<egui::Pos2> = hist
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.right() - (hist.len() - 1 - i) as f32 * rect.width() / (n - 1) as f32;
            let y = rect.bottom() - 4.0 - (v / max.max(1e-6)).clamp(0.0, 1.0) * (rect.height() - 8.0);
            egui::pos2(x, y)
        })
        .collect();
    let mut fill = pts.clone();
    fill.push(egui::pos2(pts.last().unwrap().x, rect.bottom()));
    fill.push(egui::pos2(pts[0].x, rect.bottom()));
    p.add(egui::Shape::convex_polygon(fill, color.gamma_multiply(0.15), Stroke::NONE));
    p.add(egui::Shape::line(pts, Stroke::new(2.0, color)));
}

/// Previous (0), play/pause (1), next (2), drawn so they look the same on every phone.
fn media_icon(ui: &Ui, c: egui::Pos2, kind: u8) {
    let p = ui.painter();
    let tri = |dir: f32, x: f32| {
        vec![egui::pos2(c.x + x - 7.0 * dir, c.y - 9.0), egui::pos2(c.x + x + 7.0 * dir, c.y), egui::pos2(c.x + x - 7.0 * dir, c.y + 9.0)]
    };
    match kind {
        0 => {
            p.add(egui::Shape::convex_polygon(tri(-1.0, 2.0), TEXT, Stroke::NONE));
            p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x - 9.0, c.y), vec2(3.0, 18.0)), r(1), TEXT);
        }
        2 => {
            p.add(egui::Shape::convex_polygon(tri(1.0, -2.0), TEXT, Stroke::NONE));
            p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x + 9.0, c.y), vec2(3.0, 18.0)), r(1), TEXT);
        }
        _ => {
            p.add(egui::Shape::convex_polygon(tri(1.0, -7.0), ACCENT, Stroke::NONE));
            p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x + 6.0, c.y), vec2(4.0, 18.0)), r(1), ACCENT);
            p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x + 13.0, c.y), vec2(4.0, 18.0)), r(1), ACCENT);
        }
    }
}

fn bar(ui: &mut Ui, frac: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(rect, r(4), DEEP);
    let mut f = rect;
    f.set_width(rect.width() * frac.clamp(0.0, 1.0));
    ui.painter().rect_filled(f, r(4), color);
}

impl App {
    pub fn new(ctx: &egui::Context, dir: PathBuf, scale: f32) -> Self {
        apply(ctx, scale);
        let store = Store::load(dir);
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
            last_probe: Instant::now() - Duration::from_secs(60),
            live: None,
            toast: None,
            ctx: ctx.clone(),
        };
        if a.store.pcs.is_empty() {
            a.page = Page::Add;
            a.scan();
        }
        a
    }

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
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let ok = net::reachable(&pc);
                online.lock().unwrap().insert(pc.id.clone(), ok);
                ctx.request_repaint();
            });
        }
    }

    fn open_pc(&mut self, i: usize) {
        let ctx = self.ctx.clone();
        let session = net::open(&self.store.pcs[i], &self.store.phone_id, move || ctx.request_repaint());
        self.live = Some(Live {
            session,
            connected: None,
            error: None,
            stats: None,
            cpu_hist: VecDeque::new(),
            mem_hist: VecDeque::new(),
            net_hist: VecDeque::new(),
            texture: None,
            full: (0, 0),
            fps: 0.0,
            last_frame: None,
            kbps: 0.0,
            streaming: false,
            last_stats: Instant::now() - Duration::from_secs(5),
            notice: None,
            clipboard: String::new(),
            text: String::new(),
            volume: 0.5,
            confirm: None,
        });
        self.page = Page::Pc(i);
    }

    fn close_pc(&mut self) {
        if let Some(l) = &self.live {
            let _ = l.session.to_pc.send(ToPc::ScreenStop);
        }
        self.live = None;
        self.page = Page::Home;
    }

    pub fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 16, right: 16, top: 10, bottom: 10 }).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            match self.page {
                Page::Home => self.home(ui),
                Page::Add => self.add(ui),
                Page::Pc(i) => self.pc(ui, i),
            }
        });
        if let Some((t, at)) = &self.toast {
            if at.elapsed() < Duration::from_secs(3) {
                egui::Area::new(egui::Id::unique("toast")).anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -40.0)).show(&ctx, |ui| {
                    egui::Frame::new().fill(RAISED).stroke(Stroke::new(1.0, LINE)).corner_radius(r(20)).inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
                        ui.label(RichText::new(t).color(TEXT));
                    });
                });
                ctx.request_repaint_after(Duration::from_millis(200));
            } else {
                self.toast = None;
            }
        }
    }

    /// Network events into state.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.scanning {
            if let Ok(found) = rx.try_recv() {
                self.found = found;
                self.scanning = None;
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
        let Some(l) = &mut self.live else { return };
        while let Ok(ev) = l.session.events.try_recv() {
            match ev {
                Event::Connected { pc_name } => {
                    l.connected = Some(pc_name);
                    l.error = None;
                    let _ = l.session.to_pc.send(ToPc::Stats);
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
                        l.notice = Some((if message.is_empty() { "Done".into() } else { message }, ok, Instant::now()));
                    }
                }
                Event::Msg(ToPhone::Clipboard { text }) => {
                    l.clipboard = text;
                    l.notice = Some(("Got the PC's clipboard".into(), true, Instant::now()));
                }
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
                    l.streaming = false;
                }
            }
        }
        // numbers once a second while that tab is open
        if l.connected.is_some() && self.tab == Tab::Stats && l.last_stats.elapsed() > Duration::from_secs(1) {
            l.last_stats = Instant::now();
            let _ = l.session.to_pc.send(ToPc::Stats);
            ctx.request_repaint_after(Duration::from_millis(1000));
        }
        // the screen streams only while it is visible
        let want = l.connected.is_some() && self.tab == Tab::Screen;
        if want != l.streaming {
            l.streaming = want;
            let _ = l.session.to_pc.send(if want { ToPc::ScreenStart { max_w: 1280, quality: self.store.quality.clamp(20, 90), fps: 30, output: None } } else { ToPc::ScreenStop });
        }
    }

    // ------------------------------------------------------------------ pages

    fn header(ui: &mut Ui, title: &str, sub: &str, back: bool) -> bool {
        let mut go_back = false;
        ui.horizontal(|ui| {
            if back && ui.add(egui::Button::new(RichText::new("‹").size(26.0)).fill(Color32::TRANSPARENT).min_size(vec2(44.0, 44.0))).clicked() {
                go_back = true;
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(24.0).strong());
                if !sub.is_empty() {
                    ui.label(RichText::new(sub).color(MUTED).size(14.0));
                }
            });
        });
        ui.add_space(8.0);
        go_back
    }

    fn home(&mut self, ui: &mut Ui) {
        self.probe();
        ui.ctx().request_repaint_after(Duration::from_secs(3));
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Oxide Remote").size(26.0).strong());
                ui.label(RichText::new("Your PCs").color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(ui, "  +  Add PC  ").clicked() {
                    self.page = Page::Add;
                    self.pair_error = None;
                    self.scan();
                }
            });
        });
        ui.add_space(14.0);
        let online = self.online.lock().unwrap().clone();
        let mut open = None;
        let mut forget = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, pc) in self.store.pcs.clone().iter().enumerate() {
                let state = online.get(&pc.id).copied();
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (dot, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 6.0, match state {
                            Some(true) => GOOD,
                            Some(false) => MUTED,
                            None => LINE,
                        });
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&pc.name).size(18.0).strong());
                            ui.label(RichText::new(format!("{}  ·  {}", pc.host, match state {
                                Some(true) => "online",
                                Some(false) => "offline",
                                None => "checking…",
                            })).color(MUTED).size(13.0));
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if state == Some(true) {
                            if primary(ui, "  Open  ").clicked() {
                                open = Some(i);
                            }
                        } else if !pc.macs.is_empty() {
                            if primary(ui, "  Wake up  ").clicked() {
                                let mut ok = false;
                                for m in &pc.macs {
                                    ok |= oxide_remote_proto::wake(m, pc.subnet_broadcast()).is_ok();
                                }
                                self.toast = Some((if ok { format!("Wake-up sent to {}", pc.name) } else { "Could not send the wake-up packet".into() }, Instant::now()));
                                self.last_probe = Instant::now() - Duration::from_secs(4);
                            }
                            if ghost(ui, "  Try to open  ").clicked() {
                                open = Some(i);
                            }
                        } else if ghost(ui, "  Try to open  ").clicked() {
                            open = Some(i);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("Forget").color(MUTED)).fill(Color32::TRANSPARENT)).clicked() {
                                forget = Some(i);
                            }
                        });
                    });
                });
                ui.add_space(10.0);
            }
            if self.store.pcs.is_empty() {
                ui.label(RichText::new("No PCs yet. Add one: it takes a code from the PC's Settings > Phone.").color(MUTED));
            }
        });
        if let Some(i) = open {
            self.open_pc(i);
        }
        if let Some(i) = forget {
            self.store.pcs.remove(i);
            self.store.save();
        }
    }

    fn add(&mut self, ui: &mut Ui) {
        if Self::header(ui, "Add a PC", "On the PC: Settings > Phone > Pair a phone", !self.store.pcs.is_empty()) {
            self.page = Page::Home;
            self.pairing = None;
            return;
        }
        // waiting for the code
        if let Some(p) = &mut self.pairing {
            card(ui, |ui| match &p.pc_name {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Contacting {}…", p.host));
                    });
                }
                Some(name) => {
                    ui.label(RichText::new(format!("Enter the code {name} shows")).size(18.0).strong());
                    ui.add_space(8.0);
                    let resp = ui.add(egui::TextEdit::singleline(&mut p.code).hint_text("6 digits").font(egui::FontId::monospace(28.0)).desired_width(f32::INFINITY));
                    p.code.retain(|c| c.is_ascii_digit());
                    p.code.truncate(6);
                    if !resp.has_focus() && p.code.is_empty() {
                        resp.request_focus();
                    }
                    ui.add_space(8.0);
                    let ready = p.code.len() == 6;
                    if ui.add_enabled(ready, egui::Button::new(RichText::new("Pair").color(INK).strong()).fill(ACCENT).corner_radius(r(22)).min_size(vec2(ui.available_width(), 48.0))).clicked() {
                        let _ = p.code_tx.send(p.code.clone());
                        p.pc_name = Some(format!("{name} (checking…)"));
                    }
                }
            });
            ui.add_space(10.0);
            if ghost(ui, "Cancel").clicked() {
                self.pairing = None;
            }
            return;
        }
        if let Some(e) = &self.pair_error {
            card(ui, |ui| {
                ui.label(RichText::new(e).color(BAD));
            });
            ui.add_space(8.0);
        }
        label(ui, "On this network");
        let mut start: Option<(String, u16)> = None;
        card(ui, |ui| {
            if self.scanning.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Looking for PCs…").color(MUTED));
                });
            } else if self.found.is_empty() {
                ui.label(RichText::new("No PC found. Is oxide-remoted on (Settings > Phone) and the phone on the same Wi-Fi?").color(MUTED));
            }
            for (a, host) in self.found.clone() {
                let paired = self.store.pcs.iter().any(|p| p.id == a.id);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&a.name).size(17.0).strong());
                        ui.label(RichText::new(&host).color(MUTED).size(13.0));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary(ui, if paired { "  Pair again  " } else { "  Pair  " }).clicked() {
                            start = Some((host.clone(), a.port));
                        }
                    });
                });
                ui.separator();
            }
            if self.scanning.is_none() && ghost(ui, "Search again").clicked() {
                self.scan();
            }
        });
        label(ui, "Or by address");
        card(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.manual_host).hint_text("192.168.1.20").desired_width(f32::INFINITY));
            ui.add_space(6.0);
            if primary(ui, "  Pair  ").clicked() && !self.manual_host.trim().is_empty() {
                let (h, port) = match self.manual_host.trim().rsplit_once(':') {
                    Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
                    _ => (self.manual_host.trim().to_string(), oxide_remote_proto::TCP_PORT),
                };
                start = Some((h, port));
            }
        });
        if let Some((host, port)) = start {
            let (events, code_tx) = net::pair(host.clone(), port, self.store.phone_id.clone(), self.store.phone_name.clone());
            self.pairing = Some(Pairing { host, events, code_tx, pc_name: None, code: String::new() });
            self.pair_error = None;
        }
    }

    fn pc(&mut self, ui: &mut Ui, i: usize) {
        let name = self.store.pcs.get(i).map(|p| p.name.clone()).unwrap_or_default();
        let status = match &self.live {
            Some(l) if l.connected.is_some() => "connected".to_string(),
            Some(l) if l.error.is_some() => l.error.clone().unwrap_or_default(),
            _ => "connecting…".to_string(),
        };
        if Self::header(ui, &name, &status, true) {
            self.close_pc();
            return;
        }
        if self.live.as_ref().is_some_and(|l| l.error.is_some()) {
            card(ui, |ui| {
                ui.label(RichText::new(self.live.as_ref().and_then(|l| l.error.clone()).unwrap_or_default()).color(BAD));
            });
            ui.add_space(8.0);
            if primary(ui, "  Reconnect  ").clicked() {
                self.open_pc(i);
            }
            return;
        }
        // tabs as an inset segmented control
        egui::Frame::new().fill(DEEP).stroke(Stroke::new(1.0, LINE)).corner_radius(r(24)).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let w = (ui.available_width() - 8.0) / 3.0;
                for (t, label) in [(Tab::Screen, "Screen"), (Tab::Stats, "Stats"), (Tab::Control, "Control")] {
                    let on = self.tab == t;
                    let b = egui::Button::new(RichText::new(label).color(if on { INK } else { MUTED }).strong())
                        .fill(if on { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(r(20))
                        .min_size(vec2(w, 40.0));
                    if ui.add(b).clicked() {
                        self.tab = t;
                    }
                }
            });
        });
        ui.add_space(10.0);
        if let Some(l) = &mut self.live {
            if let Some((t, ok, at)) = &l.notice {
                if at.elapsed() < Duration::from_secs(3) {
                    ui.label(RichText::new(t).color(if *ok { GOOD } else { BAD }));
                    ui.ctx().request_repaint_after(Duration::from_millis(300));
                }
            }
        }
        match self.tab {
            Tab::Screen => self.screen(ui),
            Tab::Stats => self.stats(ui),
            Tab::Control => self.control(ui),
        }
    }

    fn send(&self, m: ToPc) {
        if let Some(l) = &self.live {
            let _ = l.session.to_pc.send(m);
        }
    }

    fn screen(&mut self, ui: &mut Ui) {
        let touchpad = self.store.touchpad;
        let Some(l) = &mut self.live else { return };
        let Some(tex) = &l.texture else {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Waiting for the screen…").color(MUTED));
                });
            });
            return;
        };
        let size = tex.size_vec2();
        let avail = ui.available_width();
        let h = (avail * size.y / size.x).min(ui.available_height() - 180.0).max(120.0);
        let w = h * size.x / size.y;
        let (rect, resp) = ui.allocate_exact_size(vec2(avail, h), Sense::click_and_drag());
        let img_rect = egui::Rect::from_center_size(rect.center(), vec2(w, h));
        ui.painter().rect_filled(rect, r(12), DEEP);
        ui.painter().image(tex.id(), img_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        let tx = l.session.to_pc.clone();
        let full = l.full;
        // touch → mouse
        let (multi, zoom_scroll) = ui.input(|i| (i.multi_touch(), i.smooth_scroll_delta));
        if let Some(mt) = multi {
            // two fingers: scroll
            if mt.translation_delta.y.abs() > 0.5 {
                let _ = tx.send(ToPc::Scroll { dx: 0.0, dy: (-mt.translation_delta.y * 0.6) as f64 });
            }
        } else if zoom_scroll.y.abs() > 0.5 && resp.hovered() {
            let _ = tx.send(ToPc::Scroll { dx: 0.0, dy: (-zoom_scroll.y * 0.4) as f64 });
        } else if touchpad {
            if resp.dragged() {
                let d = resp.drag_delta();
                let k = full.0 as f32 / w.max(1.0) * 0.9;
                let _ = tx.send(ToPc::PointerBy { dx: (d.x * k) as f64, dy: (d.y * k) as f64 });
            }
            if resp.clicked() {
                let _ = tx.send(ToPc::Click { button: "left".into() });
            }
            if resp.secondary_clicked() || resp.long_touched() {
                let _ = tx.send(ToPc::Click { button: "right".into() });
            }
        } else if let Some(p) = resp.interact_pointer_pos() {
            let fx = ((p.x - img_rect.left()) / img_rect.width()) as f64;
            let fy = ((p.y - img_rect.top()) / img_rect.height()) as f64;
            if resp.drag_started() {
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
                let _ = tx.send(ToPc::ButtonState { button: "left".into(), pressed: true });
            } else if resp.dragged() {
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
            } else if resp.drag_stopped() {
                let _ = tx.send(ToPc::ButtonState { button: "left".into(), pressed: false });
            }
            if resp.clicked() {
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
                let _ = tx.send(ToPc::Click { button: "left".into() });
            }
            if resp.long_touched() || resp.secondary_clicked() {
                let _ = tx.send(ToPc::PointerTo { x: fx, y: fy });
                let _ = tx.send(ToPc::Click { button: "right".into() });
            }
        }
        ui.label(RichText::new(format!("{}×{}  ·  {:.0} fps  ·  {:.1} Mb/s", full.0, full.1, l.fps, l.kbps / 1000.0)).color(MUTED).size(12.0));
        // mode and quality
        ui.horizontal(|ui| {
            let mut tp = touchpad;
            if ui.selectable_label(tp, "Touchpad").clicked() {
                tp = true;
            }
            if ui.selectable_label(!tp, "Tap to click").clicked() {
                tp = false;
            }
            if tp != touchpad {
                self.store.touchpad = tp;
                self.store.save();
            }
        });
        let Some(l) = &mut self.live else { return };
        // keyboard
        ui.horizontal(|ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut l.text).hint_text("Type on the PC…").desired_width(ui.available_width() - 80.0));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (primary(ui, "Send").clicked() || enter) && !l.text.is_empty() {
                let _ = l.session.to_pc.send(ToPc::Text { text: std::mem::take(&mut l.text) });
                if enter {
                    let _ = l.session.to_pc.send(ToPc::Key { name: "Return".into(), mods: vec![] });
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (lab, key, mods) in [("Esc", "Escape", &[][..]), ("Tab", "Tab", &[]), ("Enter", "Return", &[]), ("Del", "BackSpace", &[]), ("←", "Left", &[]), ("→", "Right", &[]), ("↑", "Up", &[]), ("↓", "Down", &[]), ("Super", "Super_L", &[]), ("Alt+Tab", "Tab", &["alt"][..])] {
                if ui.add(egui::Button::new(RichText::new(lab).monospace()).min_size(vec2(52.0, 40.0))).clicked() {
                    let _ = l.session.to_pc.send(ToPc::Key { name: key.into(), mods: mods.iter().map(|m| m.to_string()).collect() });
                }
            }
        });
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }

    fn stats(&mut self, ui: &mut Ui) {
        let Some(l) = &self.live else { return };
        let Some(s) = l.stats.clone() else {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Reading the numbers…").color(MUTED));
                });
            });
            return;
        };
        let (cpu_hist, mem_hist, net_hist) = (l.cpu_hist.clone(), l.mem_hist.clone(), l.net_hist.clone());
        egui::ScrollArea::vertical().show(ui, |ui| {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("CPU").color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.0}%", s.cpu * 100.0)).size(22.0).strong());
                    });
                });
                spark(ui, &cpu_hist, ACCENT, 1.0);
                // one thin bar per core
                let n = s.cores.len().max(1);
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
                let bw = rect.width() / n as f32;
                for (k, c) in s.cores.iter().enumerate() {
                    let x = rect.left() + k as f32 * bw;
                    let col = egui::Rect::from_min_max(egui::pos2(x + 1.0, rect.top()), egui::pos2(x + bw - 1.0, rect.bottom()));
                    ui.painter().rect_filled(col, r(2), DEEP);
                    let mut f = col;
                    f.set_top(col.bottom() - col.height() * c.clamp(0.0, 1.0));
                    ui.painter().rect_filled(f, r(2), ACCENT.gamma_multiply(0.85));
                }
                ui.label(RichText::new(format!("load {:.2} {:.2} {:.2}", s.load[0], s.load[1], s.load[2])).color(MUTED).size(13.0));
            });
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Memory").color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} / {}", human_bytes(s.mem_used), human_bytes(s.mem_total))).size(18.0).strong());
                    });
                });
                spark(ui, &mem_hist, GOOD, 1.0);
                if s.swap_total > 0 {
                    ui.label(RichText::new(format!("swap {} / {}", human_bytes(s.swap_used), human_bytes(s.swap_total))).color(MUTED).size(13.0));
                }
            });
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Network").color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} in  ·  {} out", rate(s.net_rx), rate(s.net_tx))).strong());
                    });
                });
                let max = net_hist.iter().cloned().fold(1.0, f32::max);
                spark(ui, &net_hist, WARN, max);
            });
            if !s.temps.is_empty() || s.gpu.is_some() || s.battery.is_some() {
                ui.add_space(8.0);
                card(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for t in &s.temps {
                            let col = if t.celsius > 85.0 { BAD } else if t.celsius > 70.0 { WARN } else { TEXT };
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&t.label).color(MUTED).size(13.0));
                                ui.label(RichText::new(format!("{:.0}°C", t.celsius)).color(col).size(20.0).strong());
                            });
                            ui.add_space(14.0);
                        }
                        if let Some(g) = s.gpu {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("GPU").color(MUTED).size(13.0));
                                ui.label(RichText::new(format!("{:.0}%", g * 100.0)).size(20.0).strong());
                            });
                            ui.add_space(14.0);
                        }
                        if let Some(b) = s.battery {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(if s.charging { "Battery ⚡" } else { "Battery" }).color(MUTED).size(13.0));
                                ui.label(RichText::new(format!("{b:.0}%")).size(20.0).strong());
                            });
                        }
                    });
                });
            }
            if !s.disks.is_empty() {
                ui.add_space(8.0);
                card(ui, |ui| {
                    ui.label(RichText::new("Disks").color(MUTED));
                    for d in &s.disks {
                        let f = d.used as f32 / d.total.max(1) as f32;
                        ui.horizontal(|ui| {
                            ui.label(&d.mount);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("{} of {}", human_bytes(d.used), human_bytes(d.total))).color(MUTED).size(13.0));
                            });
                        });
                        bar(ui, f, if f > 0.9 { BAD } else { ACCENT });
                    }
                });
            }
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.label(RichText::new("Busiest programs").color(MUTED));
                for p in &s.top {
                    ui.horizontal(|ui| {
                        ui.label(&p.name);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(human_bytes(p.mem)).color(MUTED).size(13.0));
                            ui.label(RichText::new(format!("{:.0}%", p.cpu * 100.0)).monospace());
                        });
                    });
                }
                let up = s.uptime;
                ui.label(RichText::new(format!("up {}d {}h {}m", up / 86400, (up / 3600) % 24, (up / 60) % 60)).color(MUTED).size(13.0));
            });
        });
    }

    fn control(&mut self, ui: &mut Ui) {
        let mut sends: Vec<ToPc> = vec![];
        let Some(l) = &mut self.live else { return };
        if l.stats.is_none() && l.last_stats.elapsed() > Duration::from_secs(2) {
            l.last_stats = Instant::now();
            sends.push(ToPc::Stats);
        }
        let act = |n: &str, a: Option<String>| ToPc::Action { name: n.into(), arg: a };
        egui::ScrollArea::vertical().show(ui, |ui| {
            label(ui, "Media");
            card(ui, |ui| {
                if let Some(m) = l.stats.as_ref().and_then(|s| s.media.clone()) {
                    ui.label(RichText::new(m).strong());
                }
                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 20.0) / 3.0;
                    for (kind, a) in [(0, "media_prev"), (1, "media_play_pause"), (2, "media_next")] {
                        let resp = ui.add(egui::Button::new("").min_size(vec2(w, 52.0)).corner_radius(r(16)));
                        media_icon(ui, resp.rect.center(), kind);
                        if resp.clicked() {
                            sends.push(act(a, None));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Volume");
                    let resp = ui.add(egui::Slider::new(&mut l.volume, 0.0..=1.0).show_value(false));
                    if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                        sends.push(act("volume", Some(format!("{}", (l.volume * 100.0).round()))));
                    }
                    if ghost(ui, "Mute").clicked() {
                        sends.push(act("mute", None));
                    }
                });
            });
            label(ui, "Workspaces");
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let cur = l.stats.as_ref().and_then(|s| s.workspace);
                    for n in 1..=9u32 {
                        let on = cur == Some(n);
                        let b = egui::Button::new(RichText::new(n.to_string()).color(if on { INK } else { TEXT }).strong()).fill(if on { ACCENT } else { RAISED }).corner_radius(r(22)).min_size(vec2(48.0, 48.0));
                        if ui.add(b).clicked() {
                            sends.push(act("workspace", Some(n.to_string())));
                        }
                    }
                });
            });
            label(ui, "Clipboard");
            card(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut l.clipboard).desired_rows(3).desired_width(f32::INFINITY).hint_text("Text to send to the PC's clipboard"));
                ui.horizontal(|ui| {
                    if primary(ui, "  Send to PC  ").clicked() {
                        sends.push(act("clipboard_set", Some(l.clipboard.clone())));
                    }
                    if ghost(ui, "  Get from PC  ").clicked() {
                        sends.push(ToPc::ClipboardGet);
                    }
                    if ghost(ui, "  Copy here  ").clicked() {
                        ui.ctx().copy_text(l.clipboard.clone());
                    }
                });
            });
            label(ui, "Open on the PC");
            card(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut l.text).desired_width(f32::INFINITY).hint_text("an app (firefox) or a web address"));
                ui.horizontal(|ui| {
                    if primary(ui, "  Open  ").clicked() && !l.text.trim().is_empty() {
                        let t = l.text.trim().to_string();
                        if t.contains("://") || (t.contains('.') && !t.contains(' ')) {
                            let url = if t.contains("://") { t } else { format!("https://{t}") };
                            sends.push(act("open_url", Some(url)));
                        } else {
                            sends.push(act("run", Some(t)));
                        }
                        l.text.clear();
                    }
                    if ghost(ui, "  Show as notification  ").clicked() && !l.text.trim().is_empty() {
                        sends.push(act("notify", Some(std::mem::take(&mut l.text))));
                    }
                });
            });
            label(ui, "Power");
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ghost(ui, "  Lock  ").clicked() {
                        sends.push(act("lock", None));
                    }
                    if ghost(ui, "  Sleep  ").clicked() {
                        l.confirm = Some("suspend");
                    }
                    if ghost(ui, "  Restart  ").clicked() {
                        l.confirm = Some("reboot");
                    }
                    if danger(ui, "  Shut down  ").clicked() {
                        l.confirm = Some("shutdown");
                    }
                });
                if let Some(c) = l.confirm {
                    ui.add_space(8.0);
                    ui.label(RichText::new(match c {
                        "suspend" => "Put the PC to sleep?",
                        "reboot" => "Restart the PC? Unsaved work is lost.",
                        _ => "Shut the PC down? Unsaved work is lost.",
                    }).color(WARN));
                    ui.horizontal(|ui| {
                        if danger(ui, "  Yes  ").clicked() {
                            sends.push(act(c, None));
                            l.confirm = None;
                        }
                        if ghost(ui, "  No  ").clicked() {
                            l.confirm = None;
                        }
                    });
                }
            });
            ui.add_space(20.0);
        });
        for m in sends {
            self.send(m);
        }
    }
}
