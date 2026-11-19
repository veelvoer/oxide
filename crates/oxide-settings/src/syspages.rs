//! System settings: Wi-Fi, Bluetooth, Sound, Power. Basic controls always, details in advanced mode.
use eframe::egui::{self, vec2, RichText, Ui};

use crate::{system as sys, theme::*, widgets::*, App};

fn loading(ui: &mut Ui, what: &str) {
    card(ui, "", |ui| {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(format!("Looking for {what}...")).color(MUTED()));
        });
        ui.add_space(18.0);
    });
}

fn unavailable(ui: &mut Ui, text: &str) {
    card(ui, "", |ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(text).color(MUTED()));
        ui.add_space(16.0);
    });
}

// ------------------------------------------------------------------ Wi-Fi

pub fn wifi(app: &mut App, ui: &mut Ui) {
    page_header(ui, "wifi", "Wi-Fi", "Wireless networks");
    let ctx = app.ctx.clone();
    let Some(st) = app.wifi.value.clone() else { return loading(ui, "Wi-Fi") };
    if !st.available {
        return unavailable(ui, "NetworkManager (nmcli) was not found, so Wi-Fi cannot be managed here.");
    }

    card(ui, "", |ui| {
        let mut on = st.enabled;
        toggle_row(ui, "Wi-Fi", "Turn the wireless radio on or off.", true, &mut on);
        if on != st.enabled {
            let arg = if on { "on" } else { "off" };
            app.jobs.spawn(&ctx, "Wi-Fi", move || sys::run("nmcli", &["radio", "wifi", arg]).map(|_| format!("Wi-Fi turned {arg}")));
        }
    });
    if !st.enabled {
        return;
    }

    card(ui, "Networks", |ui| {
        if st.nets.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("No networks found yet. Press Scan.").color(MUTED()));
            ui.add_space(14.0);
        }
        for (n, net) in st.nets.iter().enumerate() {
            let last = n + 1 == st.nets.len();
            let help = format!(
                "{}{}",
                if net.active { "Connected  -  " } else if net.saved { "Saved  -  " } else { "" },
                if net.secured { "Secured" } else { "Open" },
            );
            let wifi_icon = match net.signal { 0..=25 => "wifi-strength-1", 26..=50 => "wifi-strength-2", 51..=75 => "wifi-strength-3", _ => "wifi-strength-4" };
            let prompting = app.wifi_target.as_deref() == Some(net.ssid.as_str());
            row_sub(ui, if net.secured && !net.active { "wifi-lock" } else { wifi_icon }, &net.ssid, &help, last && !prompting, |ui| {
                if net.active {
                    if ghost_button(ui, "Disconnect").clicked() {
                        let dev = st.device.clone();
                        app.jobs.spawn(&ctx, "Disconnect", move || sys::run("nmcli", &["dev", "disconnect", &dev]).map(|_| "Disconnected".into()));
                    }
                } else if primary_button(ui, "Connect").clicked() {
                    if net.saved || !net.secured {
                        let ssid = net.ssid.clone();
                        let saved = net.saved;
                        app.jobs.spawn(&ctx, &format!("Connecting to {}", net.ssid), move || {
                            if saved {
                                sys::run("nmcli", &["connection", "up", "id", &ssid]).map(|_| format!("Connected to {ssid}"))
                            } else {
                                sys::run("nmcli", &["dev", "wifi", "connect", &ssid]).map(|_| format!("Connected to {ssid}"))
                            }
                        });
                    } else {
                        app.wifi_target = Some(net.ssid.clone());
                        app.wifi_password.clear();
                    }
                }
                signal_icon(ui, net.signal, if net.active { GOOD() } else { TEXT() });
            });
            if prompting {
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    let r = ui.add(egui::TextEdit::singleline(&mut app.wifi_password).password(true).hint_text("Password").desired_width(240.0));
                    let go = ui.add(egui::Button::new("Join")).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    if ghost_button(ui, "Cancel").clicked() {
                        app.wifi_target = None;
                    }
                    if go && !app.wifi_password.is_empty() {
                        let (ssid, pw) = (net.ssid.clone(), app.wifi_password.clone());
                        app.wifi_target = None;
                        app.jobs.spawn(&ctx, &format!("Connecting to {}", net.ssid), move || {
                            sys::run("nmcli", &["dev", "wifi", "connect", &ssid, "password", &pw]).map(|_| format!("Connected to {ssid}"))
                        });
                    }
                });
                ui.add_space(10.0);
                if !last {
                    ui.separator();
                }
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ghost_button(ui, "Scan").clicked() {
                app.jobs.spawn(&ctx, "Scan", || sys::run("nmcli", &["dev", "wifi", "rescan"]).map(|_| "Scanning...".into()));
            }
            if app.wifi.busy() {
                ui.spinner();
            }
        });
        ui.add_space(8.0);
    });

    if !app.advanced {
        return;
    }

    if let Some(d) = &st.details {
        card(ui, "Current connection (advanced)", |ui| {
            for (i, (k, v)) in [("IP address", &d.ip), ("Gateway", &d.gateway), ("DNS servers", &d.dns), ("Hardware address", &d.mac)].iter().enumerate() {
                row(ui, k, "", i == 3, |ui| {
                    ui.label(RichText::new(v.as_str()).monospace());
                });
            }
        });
    }

    card(ui, "Saved networks (advanced)", |ui| {
        if st.saved.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("No saved networks.").color(MUTED()));
            ui.add_space(14.0);
        }
        for (n, (name, auto)) in st.saved.iter().enumerate() {
            row(ui, name, "Join automatically when in range.", n + 1 == st.saved.len(), |ui| {
                if ghost_button(ui, "Forget").clicked() {
                    let name = name.clone();
                    app.jobs.spawn(&ctx, "Forget network", move || sys::run("nmcli", &["connection", "delete", "id", &name]).map(|_| format!("Forgot {name}")));
                }
                let mut a = *auto;
                if switch(ui, &mut a).changed() {
                    let name = name.clone();
                    app.jobs.spawn(&ctx, "Auto connect", move || {
                        sys::run("nmcli", &["connection", "modify", "id", &name, "connection.autoconnect", if a { "yes" } else { "no" }]).map(|_| "Updated".into())
                    });
                }
            });
        }
    });

    card(ui, "Hidden network (advanced)", |ui| {
        text_row(ui, "Network name", "For networks that do not show up in the list.", false, &mut app.wifi_hidden_ssid, "SSID");
        row(ui, "Password", "Leave empty for an open network.", true, |ui| {
            if primary_button(ui, "Connect").clicked() && !app.wifi_hidden_ssid.is_empty() {
                let (ssid, pw) = (app.wifi_hidden_ssid.clone(), app.wifi_hidden_pw.clone());
                app.jobs.spawn(&ctx, "Hidden network", move || {
                    let mut args = vec!["dev", "wifi", "connect", &ssid];
                    if !pw.is_empty() {
                        args.extend(["password", &pw]);
                    }
                    args.extend(["hidden", "yes"]);
                    sys::run("nmcli", &args).map(|_| format!("Connected to {ssid}"))
                });
            }
            ui.add(egui::TextEdit::singleline(&mut app.wifi_hidden_pw).password(true).desired_width(180.0));
        });
    });
}

// ------------------------------------------------------------------ Bluetooth

pub fn bluetooth(app: &mut App, ui: &mut Ui) {
    page_header(ui, "bluetooth", "Bluetooth", "Devices and pairing");
    let ctx = app.ctx.clone();
    let Some(st) = app.bt.value.clone() else { return loading(ui, "Bluetooth") };
    if !st.available {
        return unavailable(ui, "No Bluetooth adapter was found (or bluetoothctl is missing).");
    }

    card(ui, "", |ui| {
        let mut on = st.powered;
        toggle_row(ui, "Bluetooth", "Turn the adapter on or off.", true, &mut on);
        if on != st.powered {
            let arg = if on { "on" } else { "off" };
            app.jobs.spawn(&ctx, "Bluetooth", move || sys::run("bluetoothctl", &["power", arg]).map(|_| format!("Bluetooth turned {arg}")));
        }
    });
    if !st.powered {
        return;
    }

    card(ui, "Devices", |ui| {
        if st.devices.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("No devices yet. Put your device in pairing mode and press Scan.").color(MUTED()));
            ui.add_space(14.0);
        }
        for (n, d) in st.devices.iter().enumerate() {
            let help = if d.connected { "Connected" } else if d.paired { "Paired" } else { "Not paired" };
            let icon = match d.icon.as_str() {
                i if i.contains("audio") || i.contains("headset") || i.contains("headphone") => "headphones",
                i if i.contains("input-keyboard") => "keyboard-outline",
                i if i.contains("input-mouse") => "mouse",
                i if i.contains("phone") => "bluetooth-connect",
                _ => if d.connected { "bluetooth-connect" } else { "bluetooth" },
            };
            row_sub(ui, icon, if d.name.is_empty() { &d.mac } else { &d.name }, help, n + 1 == st.devices.len(), |ui| {
                let mac = d.mac.clone();
                if app.advanced && d.paired && ghost_button(ui, "Remove").clicked() {
                    let mac = mac.clone();
                    app.jobs.spawn(&ctx, "Remove device", move || sys::run("bluetoothctl", &["remove", &mac]).map(|_| "Removed".into()));
                }
                if d.connected {
                    if ghost_button(ui, "Disconnect").clicked() {
                        app.jobs.spawn(&ctx, "Disconnect", move || sys::run("bluetoothctl", &["disconnect", &mac]).map(|_| "Disconnected".into()));
                    }
                    tag(ui, "connected", GOOD());
                } else if d.paired {
                    if primary_button(ui, "Connect").clicked() {
                        app.jobs.spawn(&ctx, "Connecting", move || sys::run("bluetoothctl", &["connect", &mac]).map(|_| "Connected".into()));
                    }
                } else if primary_button(ui, "Pair").clicked() {
                    app.jobs.spawn(&ctx, "Pairing", move || {
                        sys::run("bluetoothctl", &["pair", &mac])?;
                        let _ = sys::run("bluetoothctl", &["trust", &mac]);
                        sys::run("bluetoothctl", &["connect", &mac]).map(|_| "Paired and connected".into())
                    });
                }
            });
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ghost_button(ui, "Scan for devices").clicked() {
                app.jobs.spawn(&ctx, "Scan", || sys::run("bluetoothctl", &["--timeout", "8", "scan", "on"]).map(|_| "Scan finished".into()));
            }
            if app.jobs.busy() {
                ui.spinner();
            }
        });
        ui.add_space(8.0);
    });

    if !app.advanced {
        return;
    }
    card(ui, "This computer (advanced)", |ui| {
        text_row(ui, "Name", "How other devices see this computer.", false, &mut app.bt_alias, &st.alias);
        row(ui, "", "", false, |ui| {
            if ghost_button(ui, "Rename").clicked() && !app.bt_alias.is_empty() {
                let alias = app.bt_alias.clone();
                app.jobs.spawn(&ctx, "Rename", move || sys::run("bluetoothctl", &["system-alias", &alias]).map(|_| "Renamed".into()));
            }
        });
        let mut disc = st.discoverable;
        toggle_row(ui, "Visible to other devices", "Lets other devices find this computer.", false, &mut disc);
        if disc != st.discoverable {
            app.jobs.spawn(&ctx, "Visibility", move || sys::run("bluetoothctl", &["discoverable", if disc { "on" } else { "off" }]).map(|_| "Updated".into()));
        }
        let mut pairable = st.pairable;
        toggle_row(ui, "Accept pairing requests", "", true, &mut pairable);
        if pairable != st.pairable {
            app.jobs.spawn(&ctx, "Pairable", move || sys::run("bluetoothctl", &["pairable", if pairable { "on" } else { "off" }]).map(|_| "Updated".into()));
        }
    });
}

// ------------------------------------------------------------------ Sound

fn device_list(app: &mut App, ui: &mut Ui, title: &str, devices: &[sys::AudioDev], input: bool) {
    let ctx = app.ctx.clone();
    card(ui, title, |ui| {
        for (n, d) in devices.iter().enumerate() {
            row_sub(ui, if input { "microphone" } else { "speaker" }, &d.name, if d.default { "In use" } else { "" }, n + 1 == devices.len(), |ui| {
                if d.default {
                    tag(ui, "in use", GOOD());
                } else if ghost_button(ui, "Use this").clicked() {
                    let id = d.id.to_string();
                    app.jobs.spawn(&ctx, "Switch device", move || sys::run("wpctl", &["set-default", &id]).map(|_| "Switched".into()));
                }
            });
        }
        if let Some(cur) = devices.iter().find(|d| d.default) {
            let mut pct = cur.volume * 100.0;
            let id = cur.id.to_string();
            if let Some(v) = commit_slider_row(ui, if input { "Microphone volume" } else { "Volume" }, "", false, &mut pct, 0.0..=100.0, "%") {
                let id = id.clone();
                app.jobs.spawn(&ctx, "Volume", move || sys::run("wpctl", &["set-volume", &id, &format!("{:.2}", v / 100.0)]).map(|_| String::new()));
            }
            let mut muted = cur.muted;
            toggle_row(ui, if input { "Mute microphone" } else { "Mute" }, "", true, &mut muted);
            if muted != cur.muted {
                app.jobs.spawn(&ctx, "Mute", move || sys::run("wpctl", &["set-mute", &id, if muted { "1" } else { "0" }]).map(|_| String::new()));
            }
        }
    });
}

pub fn sound(app: &mut App, ui: &mut Ui) {
    page_header(ui, "volume-high", "Sound", "Volume, speakers and microphones");
    let ctx = app.ctx.clone();
    let Some(st) = app.audio.value.clone() else { return loading(ui, "audio devices") };
    if !st.available {
        return unavailable(ui, "PipeWire (wpctl) was not found, so sound cannot be managed here.");
    }
    device_list(app, ui, "Output", &st.sinks, false);
    device_list(app, ui, "Input", &st.sources, true);

    if !app.advanced {
        return;
    }
    card(ui, "Volume per app (advanced)", |ui| {
        if st.streams.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("No app is playing sound right now.").color(MUTED()));
            ui.add_space(14.0);
        }
        for (n, s) in st.streams.iter().enumerate() {
            let mut pct = s.volume * 100.0;
            if let Some(v) = commit_slider_row(ui, &s.name, "", n + 1 == st.streams.len(), &mut pct, 0.0..=100.0, "%") {
                let idx = s.index.to_string();
                app.jobs.spawn(&ctx, "App volume", move || sys::run("pactl", &["set-sink-input-volume", &idx, &format!("{}%", v.round())]).map(|_| String::new()));
            }
        }
    });
    card(ui, "Sound cards (advanced)", |ui| {
        for (n, c) in st.cards.iter().enumerate() {
            row(ui, &c.description, "Profile decides which outputs and inputs exist.", n + 1 == st.cards.len(), |ui| {
                let current = c.profiles.iter().find(|(k, _)| *k == c.active).map(|(_, d)| d.clone()).unwrap_or(c.active.clone());
                egui::ComboBox::from_id_salt(("card", n)).selected_text(current).width(260.0).show_ui(ui, |ui| {
                    for (key, desc) in &c.profiles {
                        if ui.selectable_label(*key == c.active, desc).clicked() && *key != c.active {
                            let (card, key) = (c.name.clone(), key.clone());
                            app.jobs.spawn(&ctx, "Sound profile", move || sys::run("pactl", &["set-card-profile", &card, &key]).map(|_| "Profile changed".into()));
                        }
                    }
                });
            });
        }
    });
}

// ------------------------------------------------------------------ Power

pub fn power(app: &mut App, ui: &mut Ui) {
    page_header(ui, "battery-high", "Power", "Battery and performance");
    let ctx = app.ctx.clone();
    let Some(mut st) = app.power.value.clone() else { return loading(ui, "battery information") };

    if let Some((pct, status)) = &st.battery {
        card(ui, "Battery", |ui| {
            row_sub(ui, if status == "Charging" { "battery-charging" } else { "battery-high" }, &format!("{pct}%"), status, true, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(220.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, radius(6), FIELD());
                let color = if *pct < 20 { BAD() } else if *pct < 40 { WARN() } else { GOOD() };
                let fill = egui::Rect::from_min_size(rect.min, vec2(rect.width() * *pct as f32 / 100.0, rect.height()));
                ui.painter().rect_filled(fill, radius(6), color);
            });
        });
    }
    if let Some(b) = st.brightness {
        card(ui, "Screen", |ui| {
            let mut pct = b * 100.0;
            if let Some(v) = commit_slider_row(ui, "Brightness", "Built-in display.", true, &mut pct, 1.0..=100.0, "%") {
                app.jobs.spawn(&ctx, "Brightness", move || sys::run("brightnessctl", &["set", &format!("{}%", v.round())]).map(|_| String::new()));
                st.brightness = Some(v / 100.0);
            }
        });
    }
    {
        let p = &mut app.cfg.power;
        card(ui, "Performance", |ui| {
            choice_row(
                ui,
                "Profile",
                "Eco: no blur or shadows, short animations, the system saves power. Performance: everything on, the system at full speed.",
                false,
                &mut p.profile,
                &[("eco".to_string(), "Eco"), ("normal".to_string(), "Normal"), ("performance".to_string(), "Performance")],
            );
            toggle_row(ui, "Also set the system's power mode", "Switches power-profiles-daemon along with the profile (CPU speed, fan).", false, &mut p.system_profile);
            slider_row(ui, "Eco when the battery is below", "Switches to Eco on battery by itself; 0 turns it off.", true, &mut p.auto_eco_below, 0..=50, "%");
        });
        let lid = [
            ("lock_suspend".to_string(), "Lock and sleep"),
            ("lock".to_string(), "Lock"),
            ("suspend".to_string(), "Sleep"),
            ("nothing".to_string(), "Nothing"),
            ("system".to_string(), "System decides"),
        ];
        card(ui, "Closing the lid", |ui| {
            choice_row(ui, "On battery", "What closing the lid does when unplugged.", false, &mut p.lid_battery, &lid);
            choice_row(ui, "Plugged in", "What closing the lid does on mains power.", false, &mut p.lid_ac, &lid);
            toggle_row(ui, "Keep going with an external screen", "With another screen connected, closing the lid does nothing.", false, &mut p.lid_ignore_external);
            slider_row(ui, "Wait before acting", "Seconds the lid must stay closed; 0 acts at once.", false, &mut p.lid_delay, 0..=60, " s");
            toggle_row(ui, "Lock before sleeping", "Always lock first, so waking up asks for your password.", true, &mut p.lock_before_sleep);
        });
        card(ui, "When idle", |ui| {
            slider_row(ui, "Lock on battery after", "Minutes without input; 0 never.", false, &mut p.idle_lock_battery, 0..=60, " min");
            slider_row(ui, "Lock plugged in after", "Minutes without input; 0 never.", false, &mut p.idle_lock_ac, 0..=120, " min");
            slider_row(ui, "Sleep on battery after", "Minutes without input; 0 never.", false, &mut p.idle_suspend_battery, 0..=120, " min");
            slider_row(ui, "Sleep plugged in after", "Minutes without input; 0 never.", false, &mut p.idle_suspend_ac, 0..=240, " min");
            toggle_row(ui, "Not while a window is fullscreen", "Videos and games keep the screen awake.", true, &mut p.idle_inhibit_fullscreen);
        });
    }
    app.power.value = Some(st);
}
