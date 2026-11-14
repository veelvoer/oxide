#![allow(deprecated)]
mod editor;
mod icons;
mod ownpage;
mod syntax;
mod termpage;
mod pages;
mod barpage;
mod launcherpage;
mod shellpage;
mod themepage;
mod shortcuts;
mod syspages;
mod system;
mod theme;
mod widgets;

use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, Align2, Color32, RichText, Sense, Stroke};
use oxide_config::Config;
use oxide_ipc::{call, OutputInfo, Request, Response};

use theme::*;
use widgets::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Wifi,
    Bluetooth,
    Sound,
    Displays,
    Power,
    Theme,
    Appearance,
    Animations,
    Bar,
    Launcher,
    Assistant,
    Shell,
    Layout,
    Input,
    Shortcuts,
    Apps,
    Terminal,
    Own,
    Config,
}

struct NavItem {
    page: Page,
    name: &'static str,
    icon: &'static str,
    group: &'static str,
    /// extra words the search box matches
    words: &'static str,
    advanced_only: bool,
}

const fn nav(page: Page, name: &'static str, icon: &'static str, group: &'static str, words: &'static str, advanced_only: bool) -> NavItem {
    NavItem { page, name, icon, group, words, advanced_only }
}

impl Page {
    const ALL: [NavItem; 19] = [
        nav(Page::Wifi, "Wi-Fi", "wifi", "System", "network internet wireless password connect", false),
        nav(Page::Bluetooth, "Bluetooth", "bluetooth", "System", "headphones pair device speaker", false),
        nav(Page::Sound, "Sound", "volume-high", "System", "volume audio speaker microphone mute output input", false),
        nav(Page::Displays, "Displays", "monitor", "System", "screen monitor brightness resolution scale refresh", false),
        nav(Page::Power, "Power", "battery-high", "System", "battery performance saver charge", false),
        nav(Page::Theme, "Theme", "palette-outline", "Look", "colors accent dark kitty terminal nord gruvbox catppuccin preset", false),
        nav(Page::Appearance, "Windows", "application-outline", "Look", "border gap rounding blur shadow opacity transparent gradient", false),
        nav(Page::Animations, "Animations", "animation-play-outline", "Look", "motion spring bezier curve speed fade", false),
        nav(Page::Bar, "Bar", "view-dashboard-outline", "Shell", "panel modules clock workspaces tray drag position layout widgets", false),
        nav(Page::Launcher, "Launcher", "apps", "Shell", "rofi search apps calculator web run", false),
        nav(Page::Assistant, "Assistant", "robot-outline", "Shell", "ai claude model api key chat control", false),
        nav(Page::Shell, "Shells", "puzzle-outline", "Shell", "quickshell import upload switch config install", true),
        nav(Page::Layout, "Layout", "view-grid-outline", "Desktop", "tiling dwindle master workspace focus floating", false),
        nav(Page::Input, "Input", "keyboard-outline", "Desktop", "keyboard touchpad mouse scroll tap repeat layout language", false),
        nav(Page::Shortcuts, "Shortcuts", "keyboard-variant", "Desktop", "keys bind hotkey super mouse", false),
        nav(Page::Terminal, "Terminal", "console-line", "Desktop", "kitty fish font size opacity padding cursor shell prompt", false),
        nav(Page::Apps, "Apps", "console-line", "Oxide", "startup autostart rules environment variables xwayland", true),
        nav(Page::Own, "Own configs", "text-box-edit-outline", "Oxide", "own custom kitty fish gtk shell editor syntax file upload import", true),
        nav(Page::Config, "Config", "cog-outline", "Oxide", "import export reset file hyprland backup", false),
    ];
}

pub struct App {
    pub cfg: Config,
    /// What is currently stored on disk.
    saved: Config,
    /// Last config pushed to the compositor.
    sent: Config,
    pub page: Page,
    page_started: Instant,
    pub live: bool,
    pub connected: bool,
    pub outputs: Vec<OutputInfo>,
    toast: Option<(String, Instant)>,
    last_poll: Instant,
    pub shortcuts: shortcuts::State,
    pub confirm_reset: bool,
    pub ctx: egui::Context,
    pub advanced: bool,
    seed_stamp: Option<std::time::SystemTime>,
    pub jobs: system::Jobs,
    pub wifi: system::Bg<system::WifiState>,
    pub bt: system::Bg<system::BtState>,
    pub audio: system::Bg<system::AudioState>,
    pub power: system::Bg<system::PowerState>,
    last_sys_fetch: Instant,
    pub wifi_target: Option<String>,
    pub wifi_password: String,
    pub wifi_hidden_ssid: String,
    pub wifi_hidden_pw: String,
    pub bt_alias: String,
    nav_query: String,
    pub shells: shellpage::ShellState,
    pub theme_pending: bool,
    theme_applied: Option<oxide_config::theme::ThemeCfg>,
    pub editor: editor::Editor,
    pub own_path: String,
}

fn ui_state_path() -> std::path::PathBuf {
    Config::path().with_file_name("ui.toml")
}


impl App {
    fn new(ctx: egui::Context) -> Self {
        let (cfg, toast) = match Config::load() {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some((format!("Could not read your config ({e}); showing defaults"), Instant::now()))),
        };
        let arg = std::env::args().nth(1).unwrap_or_default().to_lowercase();
        let advanced = std::fs::read_to_string(ui_state_path()).map(|t| t.contains("advanced = true")).unwrap_or(false)
            || std::env::args().any(|a| a == "--advanced");
        let page = Page::ALL
            .iter()
            .find(|p| p.name.to_lowercase().replace('-', "") == arg.replace('-', ""))
            .map_or(Page::Wifi, |p| p.page);
        let mut app = Self {
            saved: cfg.clone(),
            sent: cfg.clone(),
            cfg,
            page,
            page_started: Instant::now(),
            live: true,
            connected: false,
            outputs: vec![],
            toast,
            last_poll: Instant::now() - Duration::from_secs(60),
            shortcuts: Default::default(),
            confirm_reset: false,
            ctx,
            advanced,
            seed_stamp: None,
            jobs: Default::default(),
            wifi: Default::default(),
            bt: Default::default(),
            audio: Default::default(),
            power: Default::default(),
            last_sys_fetch: Instant::now() - Duration::from_secs(60),
            wifi_target: None,
            wifi_password: String::new(),
            wifi_hidden_ssid: String::new(),
            wifi_hidden_pw: String::new(),
            bt_alias: String::new(),
            nav_query: String::new(),
            shells: Default::default(),
            theme_pending: false,
            theme_applied: None,
            editor: Default::default(),
            own_path: String::new(),
        };
        app.poll();
        app
    }

    fn set_advanced(&mut self, on: bool) {
        self.advanced = on;
        if !on && self.page == Page::Apps {
            self.page = Page::Appearance;
        }
        let _ = std::fs::create_dir_all(Config::path().parent().unwrap_or(std::path::Path::new(".")));
        let _ = std::fs::write(ui_state_path(), format!("advanced = {on}\n"));
    }

    /// Reload the data of the page that is on screen.
    fn refresh_page(&mut self) {
        self.last_sys_fetch = Instant::now();
        let ctx = self.ctx.clone();
        match self.page {
            Page::Wifi => self.wifi.fetch(&ctx, system::wifi_state),
            Page::Bluetooth => self.bt.fetch(&ctx, system::bt_state),
            Page::Sound => self.audio.fetch(&ctx, system::audio_state),
            Page::Displays | Page::Power => self.power.fetch(&ctx, system::power_state),
            Page::Shell => self.shells.list = shellpage::scan(),
            Page::Assistant => self.shells.ai = None,
            _ => {}
        }
    }

    /// Poll background work, show finished jobs, refresh data now and then.
    fn tick_system(&mut self) {
        self.wifi.poll();
        self.bt.poll();
        self.audio.poll();
        self.power.poll();
        let finished = self.jobs.finished();
        if !finished.is_empty() {
            for (label, res) in finished {
                match res {
                    Ok(msg) if !msg.is_empty() => self.say(msg),
                    Ok(_) => {}
                    Err(e) => self.say(format!("{label}: {e}")),
                }
            }
            self.refresh_page();
        }
        let system_page = matches!(self.page, Page::Wifi | Page::Bluetooth | Page::Sound | Page::Displays | Page::Power);
        if system_page && self.last_sys_fetch.elapsed() > Duration::from_secs(6) {
            self.refresh_page();
        }
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    fn poll(&mut self) {
        self.last_poll = Instant::now();
        match call(&Request::GetState) {
            Ok(Response::State { state }) => {
                self.connected = true;
                self.outputs = state.outputs;
            }
            _ => self.connected = false,
        }
    }

    fn push_live(&mut self) {
        if !self.live || self.cfg == self.sent {
            return;
        }
        match call(&Request::SetConfig { config: Box::new(self.cfg.clone()) }) {
            Ok(Response::Ok) => {
                self.connected = true;
                self.sent = self.cfg.clone();
            }
            Ok(Response::Error { message }) => self.say(format!("Oxide rejected the change: {message}")),
            Ok(_) => {}
            Err(_) => self.connected = false,
        }
    }

    pub fn save(&mut self) {
        match self.cfg.save() {
            Ok(()) => {
                self.saved = self.cfg.clone();
                self.say("Saved");
            }
            Err(e) => self.say(format!("Could not save: {e}")),
        }
    }

    /// Write to disk without a toast (used by apply-instantly).
    fn autosave(&mut self) {
        if let Err(e) = self.cfg.save() {
            self.say(format!("Could not save: {e}"));
        } else {
            self.saved = self.cfg.clone();
        }
    }

    /// oxide.toml was edited as text: load it and tell the compositor.
    pub fn adopt_config_from_disk(&mut self) {
        match Config::load() {
            Ok(c) => {
                self.cfg = c.clone();
                self.saved = c.clone();
                self.sent = c;
                let _ = call(&Request::Reload);
                self.say("Settings reloaded");
            }
            Err(e) => self.say(format!("The file has an error: {e}")),
        }
    }

    pub fn revert(&mut self) {
        self.cfg = self.saved.clone();
        self.say("Changes discarded");
    }

    pub fn dirty(&self) -> bool {
        self.cfg != self.saved
    }

    pub fn reload_compositor(&mut self) {
        match call(&Request::Reload) {
            Ok(Response::Ok) => self.say("Oxide reloaded the config file"),
            _ => self.say("Oxide is not running"),
        }
    }

    fn go(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.page_started = Instant::now();
            self.confirm_reset = false;
            start_page(&self.ctx);
            self.refresh_page();
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(egui::Image::from_bytes("bytes://oxide-logo.svg", include_bytes!("../assets/logo.svg").as_slice()).fit_to_exact_size(vec2(24.0, 24.0)));
            ui.label(RichText::new("Oxide").family(medium()).size(17.0));
            ui.label(RichText::new("settings").color(MUTED()).size(14.0));
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let w = ui.available_width() - 4.0;
            let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
            ui.painter().rect_filled(rect, radius(4), FIELD());
            ui.painter().rect_stroke(rect, radius(4), Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
            crate::icons::paint(ui, "magnify", egui::Rect::from_center_size(egui::pos2(rect.left() + 15.0, rect.center().y), vec2(14.0, 14.0)), MUTED());
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(egui::pos2(rect.left() + 28.0, rect.top()), rect.max)));
            child.add_sized(
                [w - 34.0, 30.0],
                egui::TextEdit::singleline(&mut self.nav_query).hint_text("Search").frame(egui::Frame::NONE).vertical_align(egui::Align::Center).margin(egui::Margin::symmetric(0, 0)),
            );
        });
        ui.add_space(4.0);

        let query = self.nav_query.trim().to_lowercase();
        let nav_height = (ui.available_height() - 128.0).max(120.0);
        egui::ScrollArea::vertical().max_height(nav_height).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            let mut group = "";
            for item in Page::ALL.iter() {
                if item.advanced_only && !self.advanced {
                    continue;
                }
                // your own shell draws its own bar and launcher: the pages for the bundled ones would do nothing
                if matches!(item.page, Page::Bar | Page::Launcher) && !self.cfg.default_shell_active() {
                    continue;
                }
                if !query.is_empty() && !item.name.to_lowercase().contains(&query) && !item.words.contains(&query) {
                    continue;
                }
                if item.group != group && query.is_empty() {
                    group = item.group;
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new(item.group.to_uppercase()).size(11.0).color(MUTED()).extra_letter_spacing(1.1));
                    });
                    ui.add_space(2.0);
                }
                let selected = self.page == item.page;
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
                let id = resp.id;
                let sel = tween(ui.ctx(), id.with("sel"), if selected { 1.0 } else { 0.0 }, 0.14);
                let hov = tween(ui.ctx(), id.with("hov"), if resp.hovered() { 1.0 } else { 0.0 }, 0.1);
                let inner = rect.shrink2(vec2(2.0, 0.0));
                let p = ui.painter();
                p.rect_filled(inner, radius(8), lerp_color(Color32::TRANSPARENT, CARD_HOVER(), (sel + hov * 0.6).min(1.0)));
                crate::icons::paint(ui, item.icon, egui::Rect::from_center_size(egui::pos2(inner.left() + 20.0, inner.center().y), vec2(16.0, 16.0)), lerp_color(MUTED(), TEXT(), sel));
                ui.painter().text(
                    egui::pos2(inner.left() + 38.0, inner.center().y),
                    Align2::LEFT_CENTER,
                    item.name,
                    egui::FontId::proportional(14.5),
                    lerp_color(lerp_color(MUTED(), TEXT(), 0.55), TEXT(), sel),
                );
                if resp.clicked() {
                    self.go(item.page);
                }
                resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
        });

        ui.add_space(6.0);
        {
            let y = ui.cursor().top();
            let (l, r) = (ui.min_rect().left(), ui.min_rect().right());
            ui.painter().line_segment([egui::pos2(l, y), egui::pos2(r, y)], Stroke::new(1.0, BORDER()));
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let mut adv = self.advanced;
            if switch(ui, &mut adv).changed() {
                self.set_advanced(adv);
            }
            ui.label(RichText::new("Advanced").size(13.5));
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            switch(ui, &mut self.live);
            ui.label(RichText::new("Apply instantly").size(13.5));
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            dot(ui, if self.connected { GOOD() } else { WARN() });
            ui.label(RichText::new(if self.connected { "Oxide running" } else { "Oxide not running" }).small().color(MUTED()));
        });
    }

    fn unsaved_bar(&mut self, ctx: &egui::Context) {
        let t = tween(ctx, "dirty_bar", self.dirty() as u8 as f32, 0.28);
        if t < 0.01 {
            return;
        }
        let slide = (1.0 - ease_out(t)) * 90.0;
        egui::Area::new(egui::Id::new("unsaved"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, vec2(105.0, -22.0 + slide))
            .show(ctx, |ui| {
                ui.set_opacity(t);
                egui::Frame::new()
                    .fill(CARD_HOVER())
                    .stroke(Stroke::new(1.0, ACCENT().linear_multiply(0.5)))
                    .corner_radius(radius(8))
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(120) })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            dot(ui, WARN());
                            ui.label("You have unsaved changes");
                            ui.add_space(8.0);
                            if ghost_button(ui, "Discard").clicked() {
                                self.revert();
                            }
                            if primary_button(ui, "Save").clicked() {
                                self.save();
                            }
                        });
                    });
            });
    }

    fn toast(&mut self, ctx: &egui::Context) {
        let Some((text, at)) = self.toast.clone() else { return };
        let age = at.elapsed().as_secs_f32();
        if age > 3.4 {
            self.toast = None;
            return;
        }
        let fade_in = ease_out(age / 0.2);
        let fade_out = ease_out((3.4 - age) / 0.4);
        let t = fade_in.min(fade_out);
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::RIGHT_TOP, vec2(-22.0, 20.0 - (1.0 - t) * 14.0))
            .show(ctx, |ui| {
                ui.set_opacity(t);
                egui::Frame::new()
                    .fill(CARD_HOVER())
                    .stroke(Stroke::new(1.0, BORDER()))
                    .corner_radius(radius(8))
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.label(text);
                    });
            });
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.dirty() && self.live {
            self.autosave();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // the palette that follows the wallpaper changes by itself when the wallpaper does
        let seed_stamp = (self.cfg.theme.preset == "auto").then(|| std::fs::metadata(oxide_config::autotheme::seed_path()).and_then(|m| m.modified()).ok()).flatten();
        if seed_stamp != self.seed_stamp {
            self.seed_stamp = seed_stamp;
            self.theme_applied = None;
        }
        if self.theme_applied.as_ref() != Some(&self.cfg.theme) {
            theme::set_theme(&ctx, &oxide_config::theme::palette(&self.cfg.theme));
            self.theme_applied = Some(self.cfg.theme.clone());
        }
        theme::tick(&ctx);
        if self.last_poll.elapsed() > Duration::from_secs(3) {
            self.poll();
        }
        ctx.request_repaint_after(Duration::from_secs(3));
        self.tick_system();

        egui::Panel::left("nav")
            .exact_size(236.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(SIDEBAR()).inner_margin(egui::Margin::symmetric(10, 0)))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG())).show(ui, |ui| {
            // fade and slide the page in whenever it changes
            let elapsed = self.page_started.elapsed().as_secs_f32();
            let t = ease_out(elapsed / 0.24);
            if t < 1.0 {
                ctx.request_repaint();
            }
            ui.set_opacity(t);
            egui::ScrollArea::vertical().id_salt(format!("page-{:?}", self.page)).auto_shrink([false, false]).show(ui, |ui| {
                let width = ui.available_width().min(820.0);
                let side = ((ui.available_width() - width) / 2.0).max(24.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - side).max(300.0).min(width));
                        // the page rises into place while it fades in
                        ui.add_space(22.0 + (1.0 - t) * 18.0);
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("card_idx"), 0u32));
                        match self.page {
                            Page::Shell => shellpage::page(self, ui),
                            Page::Theme => themepage::page(self, ui),
                            Page::Bar => barpage::page(self, ui),
                            Page::Launcher => launcherpage::page(self, ui),
                            Page::Assistant => shellpage::assistant(self, ui),
                            Page::Wifi => syspages::wifi(self, ui),
                            Page::Bluetooth => syspages::bluetooth(self, ui),
                            Page::Sound => syspages::sound(self, ui),
                            Page::Power => syspages::power(self, ui),
                            Page::Appearance => pages::appearance(self, ui),
                            Page::Layout => pages::layout(self, ui),
                            Page::Animations => pages::animations(self, ui),
                            Page::Input => pages::input(self, ui),
                            Page::Displays => pages::displays(self, ui),
                            Page::Shortcuts => shortcuts::page(self, ui),
                            Page::Apps => pages::apps(self, ui),
                            Page::Terminal => termpage::page(self, ui),
                            Page::Own => ownpage::page(self, ui),
                            Page::Config => pages::config(self, ui),
                        }
                        ui.add_space(110.0);
                    });
                });
            });
        });

        self.push_live();
        // with "Apply instantly" on, every change is also kept: nothing is lost when the window closes.
        // (not while a slider or drag is in progress, to avoid writing the file on every frame)
        if self.live && self.dirty() && (self.cfg == self.sent || !self.connected) && !ctx.input(|i| i.pointer.any_down()) {
            self.autosave();
        }
        self.unsaved_bar(&ctx);
        self.toast(&ctx);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 720.0])
            .with_min_inner_size([820.0, 520.0])
            .with_app_id("oxide-settings"),
        ..Default::default()
    };
    eframe::run_native(
        "Oxide settings",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            {
                let mut app = App::new(cc.egui_ctx.clone());
                app.refresh_page();
                Ok(Box::new(app))
            }
        }),
    )
}
