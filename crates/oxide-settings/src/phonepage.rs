//! Phone: lets the Oxide Remote app on your phone see this PC's screen, its numbers and control it.
//! Talks to oxide-remoted over its local control socket (via the `oxide-remoted` command).
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText, Ui};

use crate::{theme::*, widgets::*, App};

#[derive(Default)]
pub struct PhoneState {
    status: Option<serde_json::Value>,
    last: Option<Instant>,
    code: Option<(String, Instant)>,
    phones_before: usize,
    name: String,
    message: Option<String>,
}

fn remoted(args: &[&str]) -> Option<serde_json::Value> {
    let out = std::process::Command::new("oxide-remoted").args(args).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "cellphone", "Phone", "See and control this PC from your phone, at home or away.");
    let st = &mut app.phone;
    // refresh the status every few seconds while the page is open
    if app.cfg.remote.enabled && st.last.is_none_or(|l| l.elapsed() > Duration::from_secs(3)) {
        st.last = Some(Instant::now());
        st.status = remoted(&["status"]);
        if let Some(n) = st.status.as_ref().and_then(|s| s["name"].as_str()) {
            if st.name.is_empty() {
                st.name = n.to_string();
            }
        }
    }
    ui.ctx().request_repaint_after(Duration::from_secs(1));

    card(ui, "", |ui| {
        let mut on = app.cfg.remote.enabled;
        toggle_row(ui, "Let my phone connect", "Runs oxide-remoted. Only phones you pair can connect; everything is encrypted.", true, &mut on);
        if on != app.cfg.remote.enabled {
            app.cfg.remote.enabled = on;
            st.last = None;
        }
    });
    if !app.cfg.remote.enabled {
        crate::pages::note(ui, "Get the Oxide Remote app on your phone (phone/ in the Oxide source, see docs/PHONE.md), then switch this on.");
        return;
    }
    let running = st.status.is_some();

    card(ui, "This PC", |ui| {
        row_full(ui, None, "Name", "What your phone shows for this PC.", "", false, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(220.0));
            if r.lost_focus() && !st.name.trim().is_empty() {
                remoted(&["rename", st.name.trim()]);
                st.last = None;
            }
        });
        let macs: Vec<String> = st.status.as_ref().and_then(|s| s["macs"].as_array().cloned()).unwrap_or_default().iter().filter_map(|m| m.as_str().map(String::from)).collect();
        let sub = if macs.is_empty() { "No wired or Wi-Fi card found.".to_string() } else { format!("{}  ·  must also be on in the BIOS", macs.join("  ")) };
        row_full(ui, None, "Wake-on-LAN", &sub, "", true, |_| {});
    });

    card(ui, "Phone screen", |ui| {
        let ready = ["scrcpy", "adb"].iter().all(|b| std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(b).is_file())));
        let sub = if ready {
            "Your phone's screen in a window, with almost no delay. It works while the phone is locked in your pocket: its own screen stays dark."
        } else {
            "Needs scrcpy and adb (packages scrcpy and android-tools)."
        };
        row_sub(ui, "cellphone", "Show my phone", sub, false, |ui| {
            if ready && primary_button(ui, "Show  (Super+P)").clicked() {
                let _ = std::process::Command::new("oxide-ctl").arg("phone").spawn();
            }
        });
        row_sub(ui, "lan-connect", "Set up over Wi-Fi", "Once, with the phone on USB (USB debugging on). After that no cable is needed.", true, |ui| {
            if ready && ghost_button(ui, "Set up phone screen").clicked() {
                let _ = std::process::Command::new("oxide-ctl").args(["phone", "setup"]).spawn();
            }
        });
    });

    card(ui, "Other networks", |ui| {
        row_sub(
            ui,
            "lan-connect",
            "Find this PC on any network",
            "Paired phones find it at school or work too. Its current address goes into a note only they can read.",
            true,
            |ui| {
                switch(ui, &mut app.cfg.remote.notes);
            },
        );
    });

    card(ui, "Away from home", |ui| {
        let r = &app.cfg.remote;
        let mut on = r.anywhere;
        row_sub(
            ui,
            "web",
            "Reach this PC from anywhere",
            "Your router forwards one port to this PC (UPnP); paired phones find it through an encrypted note.",
            !on,
            |ui| {
                switch(ui, &mut on);
            },
        );
        if on != r.anywhere {
            app.cfg.remote.anywhere = on;
            st.last = None;
        }
        if on {
            let reach = st.status.as_ref().map(|s| s["reach"].clone()).unwrap_or_default();
            let (icon, text, color) = if let (Some(ip), Some(port)) = (reach["public_ip"].as_str(), reach["upnp_port"].as_u64()) {
                ("check", format!("Reachable at {ip}:{port}. Open the PC once on the same network, so the phone learns the way."), GOOD())
            } else if let Some(p) = reach["problem"].as_str() {
                let others = reach["addrs"].as_array().map_or(0, |a| a.len());
                let extra = if others > 0 { format!(" Still tried: {others} IPv6/VPN address(es).") } else { String::new() };
                ("alert-circle-outline", format!("{p}{extra}"), WARN())
            } else if reach["on"].as_bool() == Some(true) {
                ("information-outline", "Asking the router…".to_string(), MUTED())
            } else {
                ("information-outline", "Starting…".to_string(), MUTED())
            };
            row_full(ui, Some(icon), "Status", &text, "", true, |ui| {
                dot(ui, color);
            });
        }
    });

    card(ui, "Pairing", |ui| {
        if !running {
            ui.add_space(10.0);
            ui.label(RichText::new("Starting oxide-remoted…").color(MUTED()));
            ui.add_space(10.0);
            return;
        }
        let waiting = st.code.as_ref().filter(|(_, until)| Instant::now() < *until).cloned();
        match waiting {
            Some((code, until)) => {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("On your phone: Add PC, pick this PC, then type").color(MUTED()));
                    ui.add_space(4.0);
                    let spaced = format!("{}  {}", &code[..3], &code[3..]);
                    ui.label(RichText::new(spaced).monospace().size(44.0).color(ACCENT()));
                    let left = until.saturating_duration_since(Instant::now()).as_secs();
                    ui.label(RichText::new(format!("valid for {left} s")).color(MUTED()).small());
                    ui.add_space(6.0);
                    if ghost_button(ui, "Cancel").clicked() {
                        remoted(&["cancel"]);
                        st.code = None;
                    }
                });
                ui.add_space(12.0);
                // paired: the status shows one more phone
                let now = st.status.as_ref().and_then(|s| s["phones"].as_array().map(|a| a.len())).unwrap_or(0);
                if now > st.phones_before {
                    st.code = None;
                    st.message = Some("Paired.".into());
                }
            }
            None => {
                row_full(ui, None, "Pair a phone", "Shows a code for two minutes. Phone and PC must be on the same network.", "", true, |ui| {
                    if primary_button(ui, "Pair a phone").clicked() {
                        if let Some(code) = remoted(&["pair"]).and_then(|v| v["code"].as_str().map(String::from)) {
                            st.code = Some((code, Instant::now() + Duration::from_secs(120)));
                            st.message = None;
                            st.phones_before = st.status.as_ref().and_then(|s| s["phones"].as_array().map(|a| a.len())).unwrap_or(0);
                            st.last = None;
                        }
                    }
                });
            }
        }
        if let Some(m) = &st.message {
            ui.label(RichText::new(m).color(GOOD()));
        }
    });

    let phones: Vec<serde_json::Value> = st.status.as_ref().and_then(|s| s["phones"].as_array().cloned()).unwrap_or_default();
    card(ui, "Paired phones", |ui| {
        if phones.is_empty() {
            ui.add_space(10.0);
            ui.label(RichText::new("None yet.").color(MUTED()));
            ui.add_space(10.0);
        }
        let n = phones.len();
        for (i, p) in phones.iter().enumerate() {
            let name = p["name"].as_str().unwrap_or("Phone").to_string();
            let id = p["id"].as_str().unwrap_or("").to_string();
            let seen = p["last_seen"].as_u64().unwrap_or(0);
            let ago = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).saturating_sub(seen);
            let sub = if ago < 120 { "connected recently".to_string() } else if ago < 3600 { format!("last seen {} min ago", ago / 60) } else if ago < 86400 { format!("last seen {} h ago", ago / 3600) } else { format!("last seen {} days ago", ago / 86400) };
            row_full(ui, Some("cellphone"), &name, &sub, "", i + 1 == n, |ui| {
                if ghost_button(ui, "Forget").clicked() {
                    remoted(&["forget", &id]);
                    st.last = None;
                }
            });
        }
    });
    crate::pages::note(
        ui,
        "If the phone cannot find this PC, your firewall may block it: allow TCP port 47820 and UDP port 47821 (for example `sudo firewall-cmd --permanent --add-port=47820/tcp --add-port=47821/udp && sudo firewall-cmd --reload`).",
    );
}
