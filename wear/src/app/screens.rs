//! The watch's screens, drawn for the round display (see `round.rs`): lists that follow the circle, a keypad of round keys,
//! and for a PC three pages side by side whose controls and numbers sit on rings along the rim.
use std::f32::consts::PI;

use super::*;
use crate::{
    gyro::GyroCfg,
    round::{arc, round_button, smooth},
    settings::ACCENTS,
};

/// What tapping a list entry does.
#[derive(Clone)]
enum Act {
    None,
    Page(Page),
    OpenPc(usize),
    WakePc(usize),
    Back(Page),
    Run(&'static str),
    Option(&'static str, usize),
}

fn deg(d: f32) -> f32 {
    d * PI / 180.0
}

impl App {
    // ------------------------------------------------------------ the frame

    pub fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.refresh_theme();
        self.poll(&ctx);
        // how smooth is it really: the frame rate while something moves goes to the log every two seconds
        self.fps.1 += 1;
        if self.fps.0.elapsed() > Duration::from_secs(2) {
            log::info!("watch ui: {:.0} frames per second", self.fps.1 as f32 / self.fps.0.elapsed().as_secs_f32());
            self.fps = (Instant::now(), 0);
        }
        let p = pal();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0, p.bg);
        let geo = Geo::new(full);
        // the gyro mouse runs on its own thread, also with the screen off, for as long as a PC is open with it (and for the
        // pinch test in the settings): it ends when the PC is left or the app is closed
        let on_pc = matches!(self.page, Page::Pc(_));
        let want = (on_pc && self.set.cursor == "gyro") || (self.page == Page::Cursor && self.set.cursor == "gyro");
        self.gyro.set(self.gyro_cfg(on_pc && self.gyro_live));
        if want {
            let wake_ctx = ctx.clone();
            self.gyro.start(self.link.tx.clone(), move || wake_ctx.request_repaint());
        } else {
            self.gyro.stop();
        }
        match self.page {
            Page::Home => self.home(ui, geo, full),
            Page::Pair => self.pair_page(ui, geo),
            Page::Phone => self.phone_page(ui, geo, full),
            Page::Pc(i) => self.pc(ui, geo, full, i),
            Page::Settings => self.settings(ui, geo, full),
            Page::Theme => self.theme_page(ui, geo, full),
            Page::Cursor => self.cursor_page(ui, geo, full),
            Page::Updates => self.updates_page(ui, geo, full),
        }
        // swipe right from the left edge: one step back (as on any watch)
        if !matches!(self.page, Page::Home | Page::Pc(_)) && self.edge_swipe(&ctx, full) {
            self.page = match self.page {
                Page::Theme | Page::Cursor | Page::Updates | Page::Phone => Page::Settings,
                _ => Page::Home,
            };
        }
        if let Some((t, at)) = &self.toast {
            if at.elapsed() < Duration::from_millis(2200) {
                let r = Rect::from_center_size(Pos2::new(full.center().x, full.bottom() - 34.0), vec2(150.0, 28.0));
                ui.painter().rect_filled(r, 14, mix(p.raised, p.text, 0.1));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, t, medium(11.5), p.text);
                ctx.request_repaint_after(Duration::from_millis(300));
            } else {
                self.toast = None;
            }
        }
    }

    /// A finger that went from the left edge to the right, far enough: true once when it lifts.
    fn edge_swipe(&self, ctx: &egui::Context, full: Rect) -> bool {
        ctx.input(|i| {
            i.pointer.primary_released()
                && match (i.pointer.press_origin(), i.pointer.latest_pos()) {
                    (Some(o), Some(e)) => o.x < full.left() + full.width() * 0.2 && e.x - o.x > 50.0 && (e.y - o.y).abs() < 60.0,
                    _ => false,
                }
        })
    }

    /// Draw a list and run what was tapped.
    fn run_list(&mut self, ui: &mut Ui, geo: Geo, full: Rect, key: &'static str, cells: Vec<Cell>, acts: Vec<Act>) -> Option<(Act, usize)> {
        let mut sc = self.scroll.remove(key).unwrap_or_default();
        let hit: Option<Hit> = crate::round::list(ui, geo, full, &mut sc, &cells);
        self.scroll.insert(key, sc);
        hit.map(|h| (acts.get(h.index).cloned().unwrap_or(Act::None), h.zone))
    }

    fn act(&mut self, act: Act, zone: usize) {
        match act {
            Act::None => {}
            Act::Page(p) => {
                if p == Page::Pair {
                    self.pairing = Pairing::default();
                }
                self.page = p;
            }
            Act::Back(p) => self.page = p,
            Act::OpenPc(i) => self.open_pc(i),
            Act::WakePc(i) => self.wake(i),
            Act::Option(key, n) => self.set_option(key, n, zone),
            Act::Run(what) => self.run_action(what),
        }
    }

    // ------------------------------------------------------------ home

    fn home(&mut self, ui: &mut Ui, geo: Geo, full: Rect) {
        let p = pal();
        let mut cells = vec![Cell::Title("Your PCs".into())];
        let mut acts = vec![Act::None];
        if self.phone.is_none() {
            cells.push(Cell::Note(self.link_status.clone()));
            acts.push(Act::None);
            if self.need_pairing {
                cells.push(Cell::Button { icon: Some(ic::PHONE), text: "Enter code".into(), filled: true });
                acts.push(Act::Page(Page::Pair));
            }
        } else if self.pcs.is_empty() {
            cells.push(Cell::Note("No PC on your phone yet. Add one in the phone app.".into()));
            acts.push(Act::None);
        }
        if self.phone.is_some() {
            for (i, pc) in self.pcs.iter().enumerate() {
                let (sub, dot) = match pc.state {
                    1 => ("Online", p.good),
                    2 => ("Online · internet", p.good),
                    3 => ("Offline", p.muted),
                    _ => ("Looking…", p.muted),
                };
                cells.push(Cell::Pc { name: pc.name.clone(), sub: sub.into(), dot, wake_accent: pc.state == 3 });
                acts.push(Act::OpenPc(i));
            }
        }
        let update = self.update.latest.is_some();
        cells.push(Cell::Button { icon: Some(ic::GEAR), text: if update { "Update".into() } else { "Setup".into() }, filled: update });
        acts.push(Act::Page(Page::Settings));
        if let Some((a, zone)) = self.run_list(ui, geo, full, "home", cells, acts) {
            // the wake button is a part of a PC's cell
            let a = match (a, zone) {
                (Act::OpenPc(i), 3) => Act::WakePc(i),
                (a, _) => a,
            };
            self.act(a, zone);
        }
        ui.ctx().request_repaint_after(Duration::from_secs(2));
    }

    // ------------------------------------------------------------ pairing with the phone: a keypad of round keys

    fn pair_page(&mut self, ui: &mut Ui, geo: Geo) {
        let p = pal();
        let waiting = self.pairing.waiting;
        let code = self.pairing.code.clone();
        // the code on top, then 4 rows of 3 round keys: all inside the circle
        let shown: String = (0..6).map(|i| code.chars().nth(i).map(|c| c.to_string()).unwrap_or_else(|| "·".into())).collect::<Vec<_>>().join(" ");
        ui.painter().text(Pos2::new(geo.c.x, geo.c.y - geo.r * 0.72), Align2::CENTER_CENTER, shown, semibold(18.0), if waiting { p.muted } else { p.text });
        let step = geo.r * 0.40;
        let mut digit = None;
        let (mut backspace, mut submit) = (false, false);
        for k in 0..12usize {
            let (cx, cy) = ((k % 3) as f32 - 1.0, (k / 3) as f32 - 1.5);
            let c = Pos2::new(geo.c.x + cx * step, geo.c.y + cy * step * 0.92 + geo.r * 0.1);
            match k {
                9 => {
                    if round_button(ui, ("key", k), c, 17.0, ic::BACKSPACE, false) {
                        backspace = true;
                    }
                }
                11 => {
                    if round_button(ui, ("key", k), c, 17.0, ic::CHECK, true) {
                        submit = true;
                    }
                }
                _ => {
                    let d = if k == 10 { 0 } else { k + 1 };
                    let id = ui.make_persistent_id(("key", k));
                    let rect = Rect::from_center_size(c, vec2(40.0, 40.0));
                    let resp = ui.interact(rect, id, egui::Sense::click());
                    let down = ui.ctx().animate_bool_with_time(id.with("d"), resp.is_pointer_button_down_on(), 0.08);
                    ui.painter().circle_filled(c, 17.0 - down * 2.0, mix(p.card, p.text, 0.04 + down * 0.1));
                    ui.painter().text(c, Align2::CENTER_CENTER, d.to_string(), semibold(15.0), p.text);
                    if resp.clicked() && !waiting {
                        digit = Some(char::from(b'0' + d as u8));
                    }
                }
            }
        }
        if let Some(d) = digit {
            if self.pairing.code.len() < 6 {
                self.pairing.code.push(d);
            }
        }
        if backspace && !waiting {
            self.pairing.code.pop();
        }
        if submit && !waiting && self.pairing.code.len() == 6 {
            self.pairing.waiting = true;
            let code = self.pairing.code.clone();
            self.ask(WatchReq::Pair { code });
        }
        // leaving: the dot at the top-left is the back key
        let back = Pos2::new(geo.c.x - geo.r * 0.74, geo.c.y - geo.r * 0.56);
        if round_button(ui, "pair-back", back, 13.0, ic::BACK, false) {
            self.page = Page::Home;
        }
    }

    // ------------------------------------------------------------ settings

    fn phone_page(&mut self, ui: &mut Ui, geo: Geo, full: Rect) {
        let paired = !self.set.token.is_empty();
        let mut cells = vec![Cell::Back, Cell::Title("Phone".into())];
        let mut acts = vec![Act::Back(Page::Settings), Act::None];
        let text = match &self.phone {
            Some(n) => format!("{n}: connected over Bluetooth"),
            None => self.link_status.clone(),
        };
        cells.push(Cell::Note(text));
        acts.push(Act::None);
        if paired {
            cells.push(Cell::Button { icon: None, text: "Forget this phone".into(), filled: false });
            acts.push(Act::Run("forget"));
        } else {
            cells.push(Cell::Button { icon: Some(ic::PHONE), text: "Pair with phone".into(), filled: true });
            acts.push(Act::Page(Page::Pair));
        }
        cells.push(Cell::Note("The watch talks to your PCs through the phone: no Wi-Fi needed.".into()));
        acts.push(Act::None);
        if let Some((a, z)) = self.run_list(ui, geo, full, "phone", cells, acts) {
            self.act(a, z);
        }
    }

    fn settings(&mut self, ui: &mut Ui, geo: Geo, full: Rect) {
        let phone_sub = self.phone.clone().unwrap_or_else(|| "Not connected".into());
        let theme = match self.set.theme.as_str() {
            "pc" => "Like my PC",
            "custom" => "Your color",
            _ => "Watch",
        };
        let cursor = if self.set.cursor == "gyro" { if self.set.gyro_mode == "point" { "Gyro: point" } else { "Gyro: move" } } else { "Touchpad" };
        let upd = match &self.update.latest {
            Some((v, _)) => format!("{v} available"),
            None => format!("Version {VERSION}"),
        };
        let cells = vec![
            Cell::Back,
            Cell::Title("Setup".into()),
            Cell::Row { icon: ic::PHONE, title: "Phone".into(), sub: phone_sub, caret: true },
            Cell::Row { icon: ic::SLIDERS, title: "Theme".into(), sub: theme.into(), caret: true },
            Cell::Row { icon: ic::MOUSE, title: "Cursor".into(), sub: cursor.into(), caret: true },
            Cell::Row { icon: ic::UPDATE, title: "Updates".into(), sub: upd, caret: true },
        ];
        let acts = vec![Act::Back(Page::Home), Act::None, Act::Page(Page::Phone), Act::Page(Page::Theme), Act::Page(Page::Cursor), Act::Page(Page::Updates)];
        if let Some((a, z)) = self.run_list(ui, geo, full, "settings", cells, acts) {
            self.act(a, z);
        }
    }

    fn theme_page(&mut self, ui: &mut Ui, geo: Geo, full: Rect) {
        let sel = match self.set.theme.as_str() {
            "custom" => 1,
            "pc" => 2,
            _ => 0,
        };
        let mut cells = vec![Cell::Back, Cell::Title("Theme".into()), Cell::Choice { options: vec!["Watch".into(), "Color".into(), "PC".into()], selected: sel }];
        let mut acts = vec![Act::Back(Page::Settings), Act::None, Act::Option("theme", 0)];
        let hint = match self.set.theme.as_str() {
            "custom" => "Pick the accent color",
            "pc" => "The colors of your PC's Oxide theme (open a PC once)",
            _ => "Oxide's colors on black",
        };
        cells.push(Cell::Note(hint.into()));
        acts.push(Act::None);
        if self.set.theme == "custom" {
            for half in 0..2 {
                let colors: Vec<Color32> = ACCENTS[half * 4..half * 4 + 4].iter().map(|(_, c)| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, *c as u8)).collect();
                let selected = if self.set.accent / 4 == half { Some(self.set.accent % 4) } else { None };
                cells.push(Cell::Swatches { colors, selected });
                acts.push(Act::Option(if half == 0 { "accent0" } else { "accent1" }, 0));
            }
        }
        if let Some((a, z)) = self.run_list(ui, geo, full, "theme", cells, acts) {
            self.act(a, z);
        }
    }

    fn cursor_page(&mut self, ui: &mut Ui, geo: Geo, full: Rect) {
        let gyro = self.set.cursor == "gyro";
        let mut cells = vec![Cell::Back, Cell::Title("Cursor".into()), Cell::Choice { options: vec!["Touchpad".into(), "Gyro".into()], selected: usize::from(gyro) }];
        let mut acts = vec![Act::Back(Page::Settings), Act::None, Act::Option("cursor", 0)];
        cells.push(Cell::Note(if gyro { "Works with the screen off while the app is open" } else { "Drag to move, tap to click, hold for right click" }.into()));
        acts.push(Act::None);
        if gyro {
            let point = self.set.gyro_mode == "point";
            cells.push(Cell::Choice { options: vec!["Move".into(), "Point".into()], selected: usize::from(point) });
            acts.push(Act::Option("gyro_mode", 0));
            cells.push(Cell::Note(
                if point { "Point at the screen with your index finger and thumb, then move your hand: the pointer follows. Pinch to click. Lower your arm to pause." } else { "Like a mouse: turn your wrist and the pointer moves; rest your arm to stop. Pinch to click." }.into(),
            ));
            acts.push(Act::None);
        }
        cells.push(Cell::Stepper { title: "Speed".into(), value: format!("{:.2}×", self.set.speed).replace(".00", "").replace(".50", ".5") });
        acts.push(Act::Option("speed", 0));
        cells.push(Cell::Toggle { title: "Flip left/right".into(), on: self.set.invert_x });
        acts.push(Act::Option("invert_x", 0));
        cells.push(Cell::Toggle { title: "Flip up/down".into(), on: self.set.invert_y });
        acts.push(Act::Option("invert_y", 0));
        if gyro {
            cells.push(Cell::Toggle { title: "Worn on right wrist".into(), on: self.set.right_wrist });
            acts.push(Act::Option("right_wrist", 0));
            cells.push(Cell::Toggle { title: "Click with a pinch".into(), on: self.set.pinch });
            acts.push(Act::Option("pinch", 0));
            if self.set.pinch {
                cells.push(Cell::Stepper { title: "Pinch sensitivity".into(), value: format!("{:.1}", self.set.pinch_level) });
                acts.push(Act::Option("pinch_level", 0));
                cells.push(Cell::Note(format!("Test: {} pinch{} seen", self.test_pinches, if self.test_pinches == 1 { "" } else { "es" })));
                acts.push(Act::None);
            }
        }
        // the sensors run here for the pinch test
        if self.gyro.running() {
            self.test_pinches = self.gyro.shared.pinches.load(std::sync::atomic::Ordering::SeqCst);
            ui.ctx().request_repaint_after(Duration::from_millis(120));
        }
        if let Some((a, z)) = self.run_list(ui, geo, full, "cursor", cells, acts) {
            self.act(a, z);
        }
    }

    fn updates_page(&mut self, ui: &mut Ui, geo: Geo, full: Rect) {
        let (state, prog) = platform::update_status();
        let checking = self.update.rx.is_some();
        let status = match (state, &self.update.latest) {
            (1, _) => format!("Downloading… {prog}%"),
            (2, _) => "Installing… confirm if Android asks".to_string(),
            (3, _) => platform::update_error(),
            (4, _) => "Installed".to_string(),
            (_, Some((v, _))) => format!("Version {v} is available"),
            _ if checking => "Looking…".to_string(),
            _ if self.update.error.is_some() => self.update.error.clone().unwrap_or_default(),
            _ if self.update.checked => "You have the newest version".to_string(),
            _ => "Not checked yet".to_string(),
        };
        let mut cells = vec![Cell::Back, Cell::Title(format!("Oxide Remote {VERSION}")), Cell::Note(status)];
        let mut acts = vec![Act::Back(Page::Settings), Act::None, Act::None];
        if self.update.latest.is_some() {
            if state != 1 {
                cells.push(Cell::Button { icon: Some(ic::UPDATE), text: "Update now".into(), filled: true });
                acts.push(Act::Run("install"));
            }
        } else if !checking {
            cells.push(Cell::Button { icon: Some(ic::REFRESH), text: "Check".into(), filled: false });
            acts.push(Act::Run("check"));
        }
        cells.push(Cell::Toggle { title: "Check by itself".into(), on: self.set.auto_update });
        acts.push(Act::Option("auto_update", 0));
        if state == 1 || state == 2 || checking {
            ui.ctx().request_repaint_after(Duration::from_millis(150));
        }
        if let Some((a, z)) = self.run_list(ui, geo, full, "updates", cells, acts) {
            self.act(a, z);
        }
    }

    /// A setting was tapped.
    fn set_option(&mut self, key: &'static str, _n: usize, zone: usize) {
        let before = self.set.clone();
        match key {
            "theme" => self.set.theme = ["watch", "custom", "pc"][zone.min(2)].into(),
            "accent0" => self.set.accent = zone.min(3),
            "accent1" => self.set.accent = 4 + zone.min(3),
            "cursor" => {
                self.set.cursor = ["pad", "gyro"][zone.min(1)].into();
            }
            "gyro_mode" => self.set.gyro_mode = ["move", "point"][zone.min(1)].into(),
            "speed" => {
                if zone == 1 {
                    self.set.speed = (self.set.speed - 0.25).max(0.5);
                } else if zone == 2 {
                    self.set.speed = (self.set.speed + 0.25).min(3.0);
                }
            }
            "pinch_level" => {
                if zone == 1 {
                    self.set.pinch_level = (self.set.pinch_level - 0.2).max(0.6);
                } else if zone == 2 {
                    self.set.pinch_level = (self.set.pinch_level + 0.2).min(4.0);
                }
            }
            "invert_x" => self.set.invert_x = !self.set.invert_x,
            "invert_y" => self.set.invert_y = !self.set.invert_y,
            "right_wrist" => {
                self.set.right_wrist = !self.set.right_wrist;
            }
            "pinch" => {
                self.set.pinch = !self.set.pinch;
            }
            "auto_update" => self.set.auto_update = !self.set.auto_update,
            _ => {}
        }
        if self.set != before {
            self.set.save();
            self.last_theme = Instant::now() - Duration::from_secs(10);
        }
    }

    fn run_action(&mut self, what: &'static str) {
        match what {
            "check" => self.check_update(),
            "install" => {
                if let Some((_, url)) = self.update.latest.clone() {
                    platform::install_update(&url);
                }
            }
            "forget" => {
                self.set.token.clear();
                *self.token.lock().unwrap() = String::new();
                self.set.save();
                self.phone = None;
                self.need_pairing = true;
                self.ask(WatchReq::Hello { name: "Watch".into(), token: String::new() });
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ one PC: three pages side by side

    fn leave_pc(&mut self) {
        self.ask(WatchReq::Close);
        self.live = None;
        self.page = Page::Home;
        self.last_list = Instant::now() - Duration::from_secs(10);
        self.last_theme = Instant::now() - Duration::from_secs(10);
        if let Some(s) = self.scroll.get_mut("home") {
            s.reset();
        }
    }

    /// The left and right click keys: two arcs low on the rim of the touchpad page.
    fn click_arcs(geo: Geo) -> [(f32, f32, &'static str); 2] {
        let _ = geo;
        [(deg(100.0), deg(160.0), "left"), (deg(20.0), deg(80.0), "right")]
    }

    fn arc_hit(geo: Geo, pos: Pos2) -> Option<&'static str> {
        let d = pos - geo.c;
        let r = d.length();
        if r < geo.r - 32.0 || r > geo.r + 2.0 {
            return None;
        }
        let ang = d.y.atan2(d.x);
        Self::click_arcs(geo).into_iter().find(|(a, b, _)| ang >= *a && ang <= *b).map(|(_, _, n)| n)
    }

    fn pc(&mut self, ui: &mut Ui, geo: Geo, full: Rect, i: usize) {
        let p = pal();
        let ctx = ui.ctx().clone();
        let w = full.width();
        let gyro = self.set.cursor == "gyro";
        // ---- the finger: it moves the pointer, or it pages between the screens
        let (pressed, down, released, pos, vel, delta) = ctx.input(|inp| (inp.pointer.primary_pressed(), inp.pointer.primary_down(), inp.pointer.primary_released(), inp.pointer.latest_pos(), inp.pointer.velocity(), inp.pointer.delta()));
        if pressed {
            if let Some(pos) = pos {
                let edge = pos.x > full.right() - 0.26 * w || pos.x < full.left() + 0.26 * w;
                let on_key = self.tab == 0 && Self::arc_hit(geo, pos).is_some();
                // on the touchpad swipes page from the edges (its middle is the pointer's), elsewhere from anywhere
                self.touch = Touch { start: if on_key { None } else { Some(pos) }, started_at: Some(Instant::now()), paging: false, pager0: self.pager, moved: 0.0, long_sent: false, can_page: !on_key && (self.tab != 0 || gyro || edge) };
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
                    self.pager = (self.touch.pager0 - d.x / w).clamp(-0.3, 2.15);
                    ctx.request_repaint();
                } else if self.tab == 0 && !gyro {
                    // the pointer: every frame's movement, sent as one message (the link joins them when it is busy)
                    let k = 2.4 * self.set.speed;
                    let fast = (delta.length() * 60.0 / 450.0).clamp(0.0, 1.0);
                    let gain = k * (1.0 + fast * 0.9);
                    let (mut dx, mut dy) = (delta.x * gain, delta.y * gain);
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
                ctx.request_repaint_after(Duration::from_millis(60));
            }
        }
        if released {
            if self.touch.paging {
                // dragged right past the first page: leave the PC
                if self.tab == 0 && self.pager < -0.12 {
                    self.leave_pc();
                    return;
                }
                let target = (self.pager - vel.x / w * 0.18).round().clamp(0.0, 2.0) as usize;
                if target != self.tab {
                    self.tab = target;
                    platform::haptic(0);
                }
            } else if self.tab == 0 {
                // a tap: the pad clicks, a click arc clicks
                if let Some(pos) = pos {
                    if let Some(btn) = Self::arc_hit(geo, pos).filter(|_| self.touch.start.is_none()) {
                        self.send(ToPc::Click { button: btn.into() });
                        platform::haptic(0);
                    } else if !gyro && self.touch.start.is_some() && self.touch.moved < 8.0 && !self.touch.long_sent && self.touch.started_at.is_some_and(|t| t.elapsed() < Duration::from_millis(350)) {
                        self.send(ToPc::Click { button: "left".into() });
                        platform::haptic(0);
                    }
                }
            }
            self.touch.start = None;
            self.touch.paging = false;
        }
        // ---- the pager glides to its page
        if !self.touch.paging {
            let dt = ctx.input(|inp| inp.stable_dt).min(0.05);
            let diff = self.tab as f32 - self.pager;
            if diff.abs() > 0.001 {
                self.pager += diff * (1.0 - (-dt * 15.0).exp());
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
            let g = Geo { c: geo.c + vec2(off, 0.0), r: geo.r };
            // the page fades toward the side it leaves to
            let _ = smooth(1.0 - off.abs() / w);
            match k {
                0 => self.pad_page(ui, g, i),
                1 => self.control_page(ui, g),
                _ => self.stats_page(ui, g),
            }
        }
        // ---- the page dots: low on the rim, between the click arcs
        for k in 0..3 {
            let near = (1.0 - (k as f32 - self.pager).abs()).clamp(0.0, 1.0);
            let c = geo.at(geo.r - 11.0, deg(90.0 + (k as f32 - 1.0) * -9.0));
            ui.painter().circle_filled(c, 2.0 + near * 1.8, mix(p.raised, p.accent, near));
        }
        let _ = p;
    }

    /// The first page: a touchpad filling the circle, or the gyro mouse.
    fn pad_page(&mut self, ui: &mut Ui, g: Geo, i: usize) {
        let p = pal();
        let gyro = self.set.cursor == "gyro";
        // the pad
        ui.painter().circle_filled(g.c, g.r - 26.0, p.card);
        // the PC, on top
        let (name, color) = match &self.live {
            Some(l) if l.connected => (self.pcs.get(i).map(|p| p.name.clone()).unwrap_or_default(), p.good),
            Some(l) if l.error.is_some() => ("Reconnecting…".to_string(), p.warn),
            _ => ("Connecting…".to_string(), p.muted),
        };
        let ty = g.c.y - g.r + 17.0;
        ui.painter().text(Pos2::new(g.c.x + 4.0, ty), Align2::CENTER_CENTER, &name, medium(11.0), p.muted);
        ui.painter().circle_filled(Pos2::new(g.c.x - 6.0 - name.chars().count() as f32 * 3.1, ty), 3.0, color);
        // the click keys
        for (a, b, which) in Self::click_arcs(g) {
            let held = ui.ctx().input(|inp| inp.pointer.primary_down() && inp.pointer.latest_pos().is_some_and(|q| Self::arc_hit(g, q) == Some(which)));
            let col = mix(p.raised, p.text, if held { 0.18 } else { 0.0 });
            arc(ui, g.c, g.r - 12.0, a + 0.05, b - 0.05, Stroke::new(18.0, col));
            let mid = g.at(g.r - 12.0, (a + b) / 2.0);
            ui.painter().text(mid, Align2::CENTER_CENTER, if which == "left" { "L" } else { "R" }, semibold(11.0), p.muted);
        }
        if gyro {
            self.gyro_pad(ui, g);
        } else {
            ui.painter().text(g.c, Align2::CENTER_CENTER, "Touchpad\ntap · hold = right click", FontId::proportional(10.5), mix(p.card, p.muted, 0.75));
        }
    }

    /// The settings of the gyro mouse, as the thread wants them.
    fn gyro_cfg(&self, send: bool) -> GyroCfg {
        GyroCfg {
            point: self.set.gyro_mode == "point",
            speed: self.set.speed,
            invert_x: self.set.invert_x,
            invert_y: self.set.invert_y,
            pinch: self.set.pinch,
            pinch_level: self.set.pinch_level,
            right_wrist: self.set.right_wrist,
            send,
        }
    }

    /// The gyro mouse's page: what it does runs on its own thread (gyro.rs); this shows it and takes the taps.
    fn gyro_pad(&mut self, ui: &mut Ui, g: Geo) {
        let p = pal();
        let ctx = ui.ctx().clone();
        let point = self.set.gyro_mode == "point";
        let c = g.c;
        // the middle: in the pointing mode it centers the pointer, otherwise it pauses (so the arm can rest)
        let resp = ui.interact(Rect::from_center_size(c, vec2(g.r, g.r)), ui.make_persistent_id("gyro-toggle"), egui::Sense::click());
        if resp.clicked() {
            if point {
                self.gyro.recenter();
            } else {
                self.gyro_live = !self.gyro_live;
            }
            platform::haptic(0);
        }
        let arm_up = !point || self.gyro.shared.arm_up.load(std::sync::atomic::Ordering::Relaxed);
        let live = self.gyro_live && arm_up;
        let live_t = ctx.animate_bool_with_time(resp.id.with("live"), live, 0.18);
        let last = self.gyro.shared.last_pinch.lock().ok().and_then(|l| *l);
        let flash = last.map(|(_, t)| (1.0 - t.elapsed().as_secs_f32() / 0.4).clamp(0.0, 1.0)).unwrap_or(0.0);
        ui.painter().circle_filled(c, 26.0 + flash * 8.0, mix(p.raised, p.accent, live_t * 0.25 + flash * 0.6));
        ui.painter().circle_stroke(c, 26.0 + flash * 8.0, Stroke::new(2.0, mix(p.muted, p.accent, live_t)));
        crate::round::icon(ui, c, ic::MOUSE, 22.0, mix(p.muted, p.text, live_t));
        let hint = if !self.gyro_ok {
            "No gyro sensor".to_string()
        } else if !self.gyro_live {
            "Paused · tap to move".to_string()
        } else if point && !arm_up {
            "Raise your arm".to_string()
        } else {
            match last {
                Some((1, t)) if t.elapsed() < Duration::from_millis(900) => "Click".into(),
                Some((3, t)) if t.elapsed() < Duration::from_millis(900) => "Right click".into(),
                Some((n, t)) if t.elapsed() < Duration::from_millis(900) => format!("{n}× pinch"),
                _ if point => "Point at the screen · tap = center".into(),
                _ => "Turn your wrist · pinch".into(),
            }
        };
        ui.painter().text(Pos2::new(c.x, c.y + 44.0), Align2::CENTER_CENTER, hint, FontId::proportional(10.0), p.muted);
        if flash > 0.0 {
            ctx.request_repaint();
        }
    }

    /// The second page: what you do from a distance, on a ring; the volume runs around the rim.
    fn control_page(&mut self, ui: &mut Ui, g: Geo) {
        let p = pal();
        let ctx = ui.ctx().clone();
        // the rotary bezel (or a two-finger scroll) sets the volume
        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.5 {
            let mut set = None;
            if let Some(l) = self.live.as_mut() {
                l.volume = (l.volume + scroll * 0.0025).clamp(0.0, 1.0);
                set = Some(format!("{:.0}", l.volume * 100.0));
            }
            if let Some(v) = set {
                self.send(ToPc::Action { name: "volume".into(), arg: Some(v) });
            }
        }
        let vol = self.live.as_ref().map_or(0.0, |l| l.volume);
        // the gauge: 270 degrees around the rim, open at the bottom
        let (a0, a1) = (deg(135.0), deg(405.0));
        arc(ui, g.c, g.r - 7.0, a0, a1, Stroke::new(5.0, p.raised));
        if vol > 0.01 {
            arc(ui, g.c, g.r - 7.0, a0, a0 + (a1 - a0) * vol.clamp(0.0, 1.0), Stroke::new(5.0, p.accent));
        }
        // eight keys on a ring, play in the middle
        let ring = g.r * 0.62;
        let keys: [(&str, &str, Option<&str>); 8] = [
            (ic::LOCK, "lock", None),
            (ic::MOON, "suspend", None),
            (ic::NEXT, "media_next", None),
            (ic::SPEAKER, "volume_up", None),
            (ic::GRID, "dispatch", Some("{\"type\":\"toggle_overview\"}")),
            (ic::MINUS, "volume_down", None),
            (ic::PREV, "media_prev", None),
            (ic::MUTE, "mute", None),
        ];
        let mut send = None;
        for (n, (icon, name, arg)) in keys.iter().enumerate() {
            let c = g.at(ring, deg(-90.0 + 45.0 * n as f32));
            if round_button(ui, ("ctl", n, g.c.x as i32 / 1000), c, 16.0, icon, false) {
                send = Some((name.to_string(), arg.map(|s| s.to_string())));
            }
        }
        if round_button(ui, "ctl-play", g.c, 22.0, ic::PLAY, true) {
            send = Some(("media_play_pause".into(), None));
        }
        if let Some((name, arg)) = send {
            platform::haptic(0);
            self.send(ToPc::Action { name, arg });
        }
    }

    /// The third page: how the PC is doing, as rings.
    fn stats_page(&mut self, ui: &mut Ui, g: Geo) {
        let p = pal();
        let Some(s) = self.live.as_ref().and_then(|l| l.stats.clone()) else {
            ui.painter().text(g.c, Align2::CENTER_CENTER, "Reading the numbers…", FontId::proportional(11.5), p.muted);
            return;
        };
        let (a0, a1) = (deg(135.0), deg(405.0));
        let ring = |ui: &Ui, radius: f32, frac: f32, color: Color32| {
            arc(ui, g.c, radius, a0, a1, Stroke::new(9.0, p.raised));
            if frac > 0.01 {
                arc(ui, g.c, radius, a0, a0 + (a1 - a0) * frac.clamp(0.0, 1.0), Stroke::new(9.0, color));
            }
        };
        let cpu = s.cpu.clamp(0.0, 1.0);
        let mem = s.mem_used as f32 / s.mem_total.max(1) as f32;
        let hot = |f: f32| if f > 0.85 { p.bad } else if f > 0.6 { p.warn } else { p.accent };
        ring(ui, g.r - 10.0, cpu, hot(cpu));
        ring(ui, g.r - 25.0, mem, hot(mem));
        ui.painter().text(g.c + vec2(0.0, -14.0), Align2::CENTER_CENTER, format!("{:.0}%", cpu * 100.0), semibold(26.0), p.text);
        ui.painter().text(g.c + vec2(0.0, 7.0), Align2::CENTER_CENTER, "processor", FontId::proportional(10.0), p.muted);
        let gb = |b: u64| b as f32 / 1_073_741_824.0;
        ui.painter().text(g.c + vec2(0.0, 24.0), Align2::CENTER_CENTER, format!("memory {:.1}/{:.0} GB", gb(s.mem_used), gb(s.mem_total)), FontId::proportional(10.5), p.text);
        let mut extra = vec![];
        if let Some(t) = s.temps.iter().map(|t| t.celsius).fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b)))) {
            extra.push(format!("{t:.0}°C"));
        }
        if let Some(b) = s.battery {
            extra.push(format!("{}{:.0}%", if s.charging { "⚡" } else { "battery " }, b * 100.0));
        }
        if !extra.is_empty() {
            ui.painter().text(g.c + vec2(0.0, 40.0), Align2::CENTER_CENTER, extra.join(" · "), FontId::proportional(10.0), p.muted);
        }
    }
}

