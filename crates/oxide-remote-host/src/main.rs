//! Oxide Remote host: lets the Oxide Remote phone app see this PC's screen with very little delay, control it and read
//! its numbers. Made for Windows (any PC without Oxide), runs in a small window and keeps working when it is minimized.
//!
//!   oxide-remote-host              open the window
//!   oxide-remote-host --minimized  start minimized (that is what "start with Windows" uses)
mod platform;
mod reach;
mod server;
mod stats;
mod store;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use eframe::egui::{self, Color32, RichText};
use oxide_remote_proto::{DISCOVERY_PORT, TCP_PORT};

const ACCENT: Color32 = Color32::from_rgb(0x8f, 0xb0, 0xff);

struct App {
    shared: server::Shared,
    name: String,
    autostart: bool,
    message: Option<(String, Instant)>,
    listening: Arc<Mutex<Option<String>>>,
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x14, 0x15, 0x1c);
    v.window_fill = Color32::from_rgb(0x1d, 0x1f, 0x29);
    v.widgets.inactive.corner_radius = egui::CornerRadius::same(12);
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(12);
    v.widgets.active.corner_radius = egui::CornerRadius::same(12);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x27, 0x2a, 0x37);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.5);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(10.0, 10.0);
        s.spacing.button_padding = egui::vec2(16.0, 9.0);
    });
}

fn card<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new().fill(Color32::from_rgb(0x1d, 0x1f, 0x29)).corner_radius(egui::CornerRadius::same(20)).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui)
    }).inner
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).color(ACCENT).size(14.5));
}

fn ago(secs: u64) -> String {
    let d = store::now().saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86399 => format!("{} h ago", d / 3600),
        _ => format!("{} days ago", d / 86400),
    }
}

impl App {
    fn say(&mut self, t: impl Into<String>) {
        self.message = Some((t.into(), Instant::now()));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(500));
        // closing the window only minimizes it: the phones stay connected
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let st = self.shared.store.lock().map(|s| s.clone()).unwrap_or_default();
        let reach = self.shared.reach.lock().map(|r| r.clone()).unwrap_or_default();
        let code = self.shared.pairing.lock().ok().and_then(|p| p.code.clone()).filter(|(_, until)| Instant::now() < *until);
        let problem = self.listening.lock().ok().and_then(|l| l.clone());

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Oxide Remote").size(26.0).strong());
                ui.label(RichText::new("See and control this PC from your phone").color(Color32::GRAY));
                ui.add_space(6.0);

                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (dot, color, text) = match &problem {
                            Some(e) => ("●", Color32::from_rgb(0xff, 0x8a, 0x8a), e.clone()),
                            None => ("●", Color32::from_rgb(0x7f, 0xd1, 0x8b), format!("Ready. Phones on this network find “{}”.", st.pc_name)),
                        };
                        {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 18.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 5.0, color);
                            let _ = dot;
                        }
                        ui.add(egui::Label::new(text).wrap());
                    });
                });

                heading(ui, "Pair a phone");
                card(ui, |ui| match &code {
                    Some((c, until)) => {
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("In the app: Add PC, pick this PC, type").color(Color32::GRAY));
                            ui.label(RichText::new(format!("{} {}", &c[..3], &c[3..])).size(46.0).color(ACCENT).monospace());
                            ui.label(RichText::new(format!("valid for {} s", until.saturating_duration_since(Instant::now()).as_secs())).size(13.0).color(Color32::GRAY));
                            if ui.button("Cancel").clicked() {
                                if let Ok(mut p) = self.shared.pairing.lock() {
                                    p.code = None;
                                }
                            }
                        });
                    }
                    None => {
                        ui.label("A phone needs a code from here, once. Both must be on the same network.");
                        if ui.add(egui::Button::new(RichText::new("Show a code").color(Color32::from_rgb(0x0e, 0x1a, 0x3a)).strong()).fill(ACCENT)).clicked() {
                            let n = u32::from_be_bytes(oxide_remote_proto::random::<4>()) % 1_000_000;
                            if std::env::var_os("OXIDE_REMOTE_DEBUG").is_some() {
                                eprintln!("pairing code {n:06}");
                            }
                            if let Ok(mut p) = self.shared.pairing.lock() {
                                p.code = Some((format!("{n:06}"), Instant::now() + Duration::from_secs(120)));
                                p.failures = 0;
                            }
                        }
                    }
                });

                heading(ui, "Paired phones");
                card(ui, |ui| {
                    if st.phones.is_empty() {
                        ui.label(RichText::new("None yet.").color(Color32::GRAY));
                    }
                    let mut forget = None;
                    for p in &st.phones {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&p.name).strong());
                                ui.label(RichText::new(format!("last seen {}", ago(p.last_seen))).size(12.5).color(Color32::GRAY));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Forget").clicked() {
                                    forget = Some(p.id.clone());
                                }
                            });
                        });
                    }
                    if let Some(id) = forget {
                        if let Ok(mut s) = self.shared.store.lock() {
                            s.phones.retain(|p| p.id != id);
                            s.save();
                        }
                    }
                });

                heading(ui, "This PC");
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name on the phone");
                        let r = ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(200.0));
                        if r.lost_focus() && !self.name.trim().is_empty() {
                            if let Ok(mut s) = self.shared.store.lock() {
                                s.pc_name = self.name.trim().to_string();
                                s.save();
                            }
                        }
                    });
                    #[cfg(windows)]
                    {
                        let mut on = self.autostart;
                        if ui.checkbox(&mut on, "Start with Windows (minimized)").changed() {
                            match platform::set_autostart(on) {
                                Ok(()) => self.autostart = on,
                                Err(e) => self.say(e),
                            }
                        }
                    }
                    let macs = store::macs();
                    if !macs.is_empty() {
                        ui.label(RichText::new(format!("Wake-on-LAN address: {}", macs.join("  "))).size(12.5).color(Color32::GRAY));
                    }
                });

                heading(ui, "Away from home");
                card(ui, |ui| {
                    let mut notes = st.notes;
                    if ui.checkbox(&mut notes, "Find this PC on any network (school, work, a friend's)").changed() {
                        if let Ok(mut s) = self.shared.store.lock() {
                            s.notes = notes;
                            s.save();
                        }
                    }
                    ui.label(RichText::new("Its current address goes into a note only your paired phones can read.").size(12.5).color(Color32::GRAY));
                    let mut any = st.anywhere;
                    if ui.checkbox(&mut any, "Reach this PC from the internet").changed() {
                        if let Ok(mut s) = self.shared.store.lock() {
                            s.anywhere = any;
                            s.save();
                        }
                    }
                    if any {
                        let text = if let (Some(ip), Some(p)) = (&reach.public_ip, reach.upnp_port) {
                            format!("Reachable at {ip}:{p}. Connect once at home so the phone learns the way.")
                        } else if let Some(p) = &reach.problem {
                            p.clone()
                        } else {
                            "Asking the router…".into()
                        };
                        ui.label(RichText::new(text).size(12.5).color(Color32::GRAY));
                    }
                });

                heading(ui, "If the phone cannot find this PC");
                card(ui, |ui| {
                    ui.label(format!("Windows Firewall has to let in TCP {TCP_PORT} and UDP {DISCOVERY_PORT}. Windows usually asks the first time; this does it for you."));
                    #[cfg(windows)]
                    if ui.button("Allow in Windows Firewall").clicked() {
                        let script = format!(
                            "netsh advfirewall firewall add rule name=\"Oxide Remote\" dir=in action=allow protocol=TCP localport={TCP_PORT} profile=private,public; netsh advfirewall firewall add rule name=\"Oxide Remote discovery\" dir=in action=allow protocol=UDP localport={DISCOVERY_PORT} profile=private,public"
                        );
                        let _ = std::process::Command::new("powershell.exe")
                            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!("Start-Process powershell -Verb RunAs -WindowStyle Hidden -ArgumentList '-NoProfile','-Command','{}'", script.replace('\'', "''"))])
                            .spawn();
                        self.say("Confirm the Windows prompt to finish.");
                    }
                });

                if let Ok(log) = self.shared.log.lock() {
                    for l in log.iter().rev().take(3) {
                        ui.label(RichText::new(l).size(12.5).color(Color32::GRAY));
                    }
                }
                if let Some((m, at)) = &self.message {
                    if at.elapsed() < Duration::from_secs(6) {
                        ui.label(RichText::new(m).color(Color32::from_rgb(0xe8, 0xb4, 0x6a)));
                    }
                }
                ui.add_space(8.0);
                if ui.button("Quit (phones disconnect)").clicked() {
                    let r = self.shared.reach.clone();
                    reach::stop(&r);
                    std::process::exit(0);
                }
                ui.add_space(12.0);
            });
        });
    }
}

fn main() -> eframe::Result {
    platform::dpi_aware();
    let minimized = std::env::args().any(|a| a == "--minimized");
    let store = Arc::new(Mutex::new(store::Store::load()));
    let shared = server::Shared {
        store: store.clone(),
        pairing: Arc::new(Mutex::new(server::Pairing::default())),
        sampler: stats::Sampler::default(),
        reach: Default::default(),
        log: Default::default(),
        remote_handshakes: Default::default(),
    };
    shared.sampler.start();
    server::discovery(shared.clone());
    reach::start(shared.reach.clone(), store.clone());
    let listening: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    {
        let (sh, l) = (shared.clone(), listening.clone());
        std::thread::spawn(move || {
            if let Err(e) = server::listen(sh) {
                if let Ok(mut l) = l.lock() {
                    *l = Some(format!("Cannot listen on port {TCP_PORT}: {e}. Is Oxide Remote already running?"));
                }
            }
        });
    }
    let name = store.lock().map(|s| s.pc_name.clone()).unwrap_or_default();
    let start_minimized = minimized;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([460.0, 760.0]).with_min_inner_size([380.0, 480.0]).with_title("Oxide Remote"),
        ..Default::default()
    };
    eframe::run_native(
        "Oxide Remote",
        options,
        Box::new(move |cc| {
            setup_style(&cc.egui_ctx);
            if start_minimized {
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            Ok(Box::new(App { shared, name, autostart: platform::autostart(), message: None, listening }))
        }),
    )
}
