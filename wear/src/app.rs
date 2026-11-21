//! The watch app: a PC list, and per PC three pages side by side (touchpad or gyro mouse, controls, stats) that you swipe
//! between. Made for a small round screen: big touch targets, little drawing, repaint only when something moves.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Stroke, Ui};
use oxide_remote_proto::{
    watch::{WatchPc, WatchReq, WatchResp},
    PcInfo, Stats, ToPc, ToPhone,
};

use crate::{
    link::{self, Link, LinkEvent},
    gyro::{Gyro, GyroCfg},
    motion, platform,
    round::{Cell, Geo, Hit, Scroller},
    settings::{Settings, ACCENTS},
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
    pub const PHONE: &str = "\u{E1E0}";
    pub const MINUS: &str = "\u{E32A}";
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Home,
    /// type the code the phone shows
    Pair,
    /// the link to the phone
    Phone,
    Pc(usize),
    Settings,
    Theme,
    Cursor,
    Updates,
}

/// An open PC (the phone keeps the real connection).
struct Live {
    connected: bool,
    error: Option<String>,
    stats: Option<Stats>,
    info: Option<PcInfo>,
    last_stats: Instant,
    volume: f32,
}

#[derive(Default)]
struct Update {
    rx: Option<std::sync::mpsc::Receiver<Result<Option<(String, String)>, String>>>,
    latest: Option<(String, String)>,
    checked: bool,
    error: Option<String>,
}

/// Pairing with the phone: the 6 digits it shows.
#[derive(Default)]
struct Pairing {
    code: String,
    error: Option<String>,
    waiting: bool,
}

/// A finger on the PC pages.
#[derive(Default)]
struct Touch {
    start: Option<Pos2>,
    /// where the finger was last: a touch screen has no position once the finger is up
    last: Option<Pos2>,
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
    set: Settings,
    token: Arc<Mutex<String>>,
    link: Link,
    /// the phone answered and knows this watch
    phone: Option<String>,
    /// what the link says while there is no phone
    link_status: String,
    /// the phone answered but wants the code first
    need_pairing: bool,
    pcs: Vec<WatchPc>,
    last_list: Instant,
    page: Page,
    tab: usize,
    /// where the pager is shown (0 = pad, 1 = controls, 2 = stats), moving smoothly toward `tab`
    pager: f32,
    touch: Touch,
    /// the last place a finger was seen (a touch screen reports none once the finger is up)
    last_pos: Option<Pos2>,
    live: Option<Live>,
    update: Update,
    pairing: Pairing,
    toast: Option<(String, Instant)>,
    pal: Option<Pal>,
    last_theme: Instant,
    /// the gyro mouse (its own thread)
    gyro: Gyro,
    /// the pointer follows the hand (tap the middle to pause)
    gyro_live: bool,
    test_pinches: u32,
    /// the strongest recent tap, fading (the sensitivity meter)
    meter: f32,
    /// the bezel's turn in this frame
    rot: f32,
    gyro_ok: bool,
    /// the scroll position of each list screen
    scroll: std::collections::HashMap<&'static str, Scroller>,
    /// frame timing, for the log (how smooth is it really)
    fps: (Instant, u32),
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

impl App {
    pub fn new(ctx: &egui::Context, dir: PathBuf) -> Self {
        fonts(ctx);
        let set = Settings::load(dir);
        let token = Arc::new(Mutex::new(set.token.clone()));
        let name = platform::device_name().map(|n| format!("{n} (watch)")).unwrap_or_else(|| "Watch".into());
        let wake_ctx = ctx.clone();
        {
            let c = ctx.clone();
            crate::input::set_repaint(move || c.request_repaint());
        }
        let link = link::start(name, token.clone(), set.phone_addr.clone(), move || wake_ctx.request_repaint());
        let mut a = Self {
            ctx: ctx.clone(),
            set,
            token,
            link,
            phone: None,
            link_status: "Looking for your phone…".into(),
            need_pairing: false,
            pcs: vec![],
            last_list: Instant::now() - Duration::from_secs(60),
            page: Page::Home,
            tab: 0,
            pager: 0.0,
            touch: Touch::default(),
            last_pos: None,
            live: None,
            update: Update::default(),
            pairing: Pairing::default(),
            toast: None,
            pal: None,
            last_theme: Instant::now() - Duration::from_secs(10),
            gyro: Gyro::new(GyroCfg { point: true, speed: 1.0, invert_x: false, invert_y: false, pinch: true, pinch_count: false, pinch_level: 7.0, right_wrist: false, send: false }),
            gyro_live: true,
            test_pinches: 0,
            meter: 0.0,
            rot: 0.0,
            gyro_ok: motion::has_gyro(),
            scroll: Default::default(),
            fps: (Instant::now(), 0),
        };
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
                let pc = if let Page::Pc(i) = self.page { self.pcs.get(i) } else { None }.or_else(|| self.pcs.first());
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

    fn ask(&self, r: WatchReq) {
        let _ = self.link.tx.send(r);
    }

    fn open_pc(&mut self, i: usize) {
        let Some(pc) = self.pcs.get(i) else { return };
        self.ask(WatchReq::Open { id: pc.id.clone() });
        self.live = Some(Live { connected: false, error: None, stats: None, info: None, last_stats: Instant::now() - Duration::from_secs(5), volume: 0.5 });
        self.page = Page::Pc(i);
        self.tab = 0;
        self.pager = 0.0;
        self.last_theme = Instant::now() - Duration::from_secs(10);
    }

    fn wake(&mut self, i: usize) {
        let Some(pc) = self.pcs.get(i) else { return };
        self.ask(WatchReq::Wake { id: pc.id.clone() });
        platform::haptic(1);
        self.last_list = Instant::now() - Duration::from_secs(6);
    }

    fn send(&self, m: ToPc) {
        self.ask(WatchReq::Send { msg: m });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.link.rx.try_recv() {
            match ev {
                LinkEvent::Status(s) => {
                    self.link_status = s;
                    self.phone = None;
                }
                LinkEvent::Up(addr) => {
                    self.link_status = "Phone found".into();
                    if self.set.phone_addr != addr {
                        self.set.phone_addr = addr;
                        self.set.save();
                    }
                }
                LinkEvent::Down => {
                    self.phone = None;
                    self.need_pairing = false;
                    self.link_status = "Looking for your phone…".into();
                    if let Some(l) = self.live.as_mut() {
                        l.connected = false;
                        l.error = Some("The phone is out of reach".into());
                    }
                }
                LinkEvent::Resp(r) => self.on_resp(r),
            }
        }
        // the PC list while it is shown (and its state: the phone looks which are on)
        if self.phone.is_some() && self.page == Page::Home && self.last_list.elapsed() > Duration::from_secs(8) {
            self.last_list = Instant::now();
            self.ask(WatchReq::List);
        }
        // the numbers once a second, only while they are shown
        let want_stats = self.tab == 2;
        if let Some(l) = self.live.as_mut() {
            if l.connected && want_stats && l.last_stats.elapsed() > Duration::from_secs(1) {
                l.last_stats = Instant::now();
                let _ = self.link.tx.send(WatchReq::Send { msg: ToPc::Stats });
                ctx.request_repaint_after(Duration::from_millis(1000));
            }
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
    }

    fn on_resp(&mut self, r: WatchResp) {
        match r {
            WatchResp::Welcome { phone } => {
                self.phone = Some(phone);
                self.need_pairing = false;
                if self.page == Page::Pair {
                    self.page = Page::Home;
                }
                self.last_list = Instant::now();
                self.ask(WatchReq::List);
            }
            WatchResp::NeedPairing => {
                self.phone = None;
                self.need_pairing = true;
                self.link_status = "Pair this watch: on the phone open Oxide Remote > Settings > My watch".into();
                // the token is no longer good (the phone forgot this watch)
                if !self.set.token.is_empty() && self.page != Page::Pair {
                    self.set.token.clear();
                    *self.token.lock().unwrap() = String::new();
                    self.set.save();
                }
            }
            WatchResp::Paired { token } => {
                self.set.token = token.clone();
                *self.token.lock().unwrap() = token;
                self.set.save();
                self.pairing = Pairing::default();
                self.say("Paired with your phone");
            }
            WatchResp::PairFailed { reason } => {
                self.pairing.waiting = false;
                self.pairing.code.clear();
                self.say(reason);
            }
            WatchResp::Pcs { pcs } => {
                // the palette of a PC comes with the list: remember it for the "PC" theme
                self.pcs = pcs;
                if self.set.theme == "pc" {
                    self.last_theme = Instant::now() - Duration::from_secs(10);
                }
            }
            WatchResp::Opened { .. } => {
                if let Some(l) = self.live.as_mut() {
                    l.connected = true;
                    l.error = None;
                    self.ask(WatchReq::Send { msg: ToPc::Info });
                    self.ask(WatchReq::Send { msg: ToPc::Stats });
                }
            }
            WatchResp::OpenFailed { error } => {
                if let Some(l) = self.live.as_mut() {
                    l.error = Some(error);
                }
            }
            WatchResp::Closed { reason } => {
                if let Some(l) = self.live.as_mut() {
                    l.connected = false;
                    l.error = Some(reason);
                }
            }
            WatchResp::Msg { msg } => {
                if let Some(l) = self.live.as_mut() {
                    match msg {
                        ToPhone::Info(info) => l.info = Some(info),
                        ToPhone::Stats(s) => {
                            if let Some(v) = s.volume {
                                l.volume = v;
                            }
                            l.stats = Some(s);
                        }
                        ToPhone::Done { ok, message } => {
                            if !ok || !message.is_empty() {
                                self.toast = Some((if message.is_empty() { "Done".into() } else { message }, Instant::now()));
                            }
                        }
                        _ => {}
                    }
                }
            }
            WatchResp::Toast { text } => self.say(text),
            WatchResp::Pong => {}
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
}

mod screens;
