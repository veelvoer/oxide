//! Presets: save the whole configuration under a name, bring it back, and keep it on GitHub.
use std::sync::{Arc, Mutex};

use eframe::egui::{self, RichText, Ui};
use oxide_config::{
    presets::{self, Preset},
    sync,
};

use crate::{theme::*, widgets::*, App};

/// The GitHub sign-in in progress (a thread fills it in).
#[derive(Default, Clone)]
enum Flow {
    #[default]
    Idle,
    /// type this code at the address
    Code { user_code: String, url: String },
    Busy(String),
    /// it did not work: the reason, and a way to start again
    Failed(String),
}

#[derive(Default)]
pub struct PresetState {
    list: Vec<Preset>,
    loaded: bool,
    new_name: String,
    account: sync::Account,
    account_loaded: bool,
    flow: Arc<Mutex<Flow>>,
    confirm_delete: Option<String>,
    last_refresh: Option<std::time::Instant>,
    /// the GitHub CLI is installed and signed in (looked at once: asking costs a moment)
    gh_ok: Arc<Mutex<Option<bool>>>,
    gh_asked: bool,
}

fn time_text(secs: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let d = now.saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        _ => format!("{} days ago", d / 86_400),
    }
}

pub fn page(app: &mut App, ui: &mut Ui) {
    let ctx = app.ctx.clone();
    // the state is taken out while the page is drawn (the page also needs the rest of the app)
    let mut state = std::mem::take(&mut app.presets);
    let st = &mut state;
    if !st.loaded {
        st.list = presets::list();
        st.loaded = true;
    }
    if !st.account_loaded {
        st.account = sync::load_account();
        st.account_loaded = true;
    }
    page_header(ui, "tune-variant", "Presets", "Your whole setup under a name");
    let mut toast: Option<String> = None;
    let mut refresh = false;

    // ---- save the current setup
    card(ui, "Save the current setup", |ui| {
        text_row(ui, "Name", "Every setting there is goes into it: windows, animations, bar, launcher, theme, keys, the assistant (not its API key).", false, &mut st.new_name, "My setup");
        row(ui, "", "", true, |ui| {
            let ok = !st.new_name.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new("Save").fill(ACCENT()).corner_radius(radius(8))).clicked() {
                let name = st.new_name.trim().to_string();
                match presets::save(&presets::capture(&name)) {
                    Ok(()) => {
                        toast = Some(format!("Saved \"{name}\""));
                        st.new_name.clear();
                        refresh = true;
                    }
                    Err(e) => toast = Some(format!("Could not save: {e}")),
                }
            }
        });
    });

    // ---- the list
    let mut apply: Option<Preset> = None;
    let mut delete: Option<String> = None;
    let mut overwrite: Option<String> = None;
    card(ui, "Your presets", |ui| {
        if st.list.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("No presets yet. Save the current setup above.").color(MUTED()));
            ui.add_space(8.0);
            return;
        }
        let n = st.list.len();
        for (i, p) in st.list.iter().enumerate() {
            let current = presets::is_current(p);
            let sub = format!("{}{}", time_text(p.created), if current { " · in use" } else { "" });
            row_sub(ui, "tune-variant", &p.name, &sub, i + 1 == n, |ui| {
                if st.confirm_delete.as_deref() == Some(p.name.as_str()) {
                    if ui.add(egui::Button::new(RichText::new("Delete for good").color(BAD())).fill(FIELD()).corner_radius(radius(8))).clicked() {
                        delete = Some(p.name.clone());
                    }
                    if ui.add(egui::Button::new("Keep").fill(FIELD()).corner_radius(radius(8))).clicked() {
                        st.confirm_delete = None;
                    }
                } else {
                    if ui.add(egui::Button::new(RichText::new("Delete").color(MUTED())).fill(FIELD()).corner_radius(radius(8))).clicked() {
                        st.confirm_delete = Some(p.name.clone());
                    }
                    if p.name != presets::UNDO && ui.add(egui::Button::new("Replace with now").fill(FIELD()).corner_radius(radius(8))).clicked() {
                        overwrite = Some(p.name.clone());
                    }
                    if ui.add_enabled(!current, egui::Button::new(if p.name == presets::UNDO { "Undo" } else { "Use" }).fill(ACCENT()).corner_radius(radius(8))).clicked() {
                        apply = Some(p.clone());
                    }
                }
            });
        }
    });
    if let Some(p) = apply {
        match presets::apply(&p) {
            Ok(()) => {
                toast = Some(format!("\"{}\" is in use now", p.name));
                refresh = true;
                // the file watchers pick the files up; the shell and the compositor take the rest
                app.adopt_config_from_disk();
            }
            Err(e) => toast = Some(format!("Could not use it: {e}")),
        }
    }
    if let Some(name) = delete {
        presets::delete(&name);
        st.confirm_delete = None;
        refresh = true;
    }
    if let Some(name) = overwrite {
        let _ = presets::save(&presets::capture(&name));
        toast = Some(format!("\"{name}\" now holds the current setup"));
        refresh = true;
    }

    // ---- GitHub
    let flow = st.flow.lock().map(|f| f.clone()).unwrap_or_default();
    // is the GitHub CLI signed in? asked once, in the background: it may talk to the keyring and take a while
    if !st.gh_asked {
        st.gh_asked = true;
        let (slot, ctx) = (st.gh_ok.clone(), ctx.clone());
        std::thread::spawn(move || {
            *slot.lock().unwrap() = Some(sync::token_from_gh().is_some());
            ctx.request_repaint();
        });
    }
    let gh_ok = st.gh_ok.lock().map(|g| g.unwrap_or(false)).unwrap_or(false);
    let signed_in = !st.account.token.is_empty();
    card(ui, "GitHub", |ui| {
        if signed_in {
            row_sub(ui, "account-circle-outline", &format!("Signed in as {}", st.account.login), "Your presets and your setup are kept in a private gist of yours", false, |ui| {
                if ui.add(egui::Button::new("Sign out").fill(FIELD()).corner_radius(radius(8))).clicked() {
                    sync::sign_out();
                    st.account = sync::load_account();
                }
            });
            row(ui, "Save to GitHub", "All your presets and the current setup, to use on another computer. The assistant's API key is never saved.", false, |ui| {
                if ui.add(egui::Button::new("Save").fill(ACCENT()).corner_radius(radius(8))).clicked() {
                    app.jobs.spawn(&ctx, "GitHub", || sync::upload());
                }
            });
            row(ui, "Get from GitHub", "Your presets from GitHub are added here. Your current setup is changed only if you choose so.", true, |ui| {
                if ui.add(egui::Button::new("Get presets and setup").fill(FIELD()).corner_radius(radius(8))).clicked() {
                    app.jobs.spawn(&ctx, "GitHub", || sync::download().and_then(|b| sync::import(&b, true)));
                }
                if ui.add(egui::Button::new("Get presets only").fill(FIELD()).corner_radius(radius(8))).clicked() {
                    app.jobs.spawn(&ctx, "GitHub", || sync::download().and_then(|b| sync::import(&b, false)));
                }
            });
        } else {
            match &flow {
                Flow::Code { user_code, url } => {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Open this page and type the code:").color(MUTED()));
                    ui.label(RichText::new(url).color(ACCENT()));
                    ui.add_space(4.0);
                    ui.label(RichText::new(user_code).size(30.0).strong());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new("Open the page").fill(ACCENT()).corner_radius(radius(8))).clicked() {
                            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                        }
                        ui.label(RichText::new("Waiting for you…").color(MUTED()));
                    });
                    ui.add_space(6.0);
                }
                Flow::Failed(why) => {
                    ui.add_space(6.0);
                    ui.label(RichText::new(why).color(BAD()));
                    if ui.add(egui::Button::new("Try again").fill(FIELD()).corner_radius(radius(8))).clicked() {
                        *st.flow.lock().unwrap() = Flow::Idle;
                    }
                    ui.add_space(6.0);
                }
                Flow::Busy(what) => {
                    ui.add_space(8.0);
                    ui.label(RichText::new(what).color(MUTED()));
                    ui.add_space(8.0);
                }
                Flow::Idle => {
                    row(ui, "Sign in with GitHub", "Keep your presets and your setup on your account, on any computer.", false, |ui| {
                        if ui.add_enabled(!st.account.client_id.trim().is_empty(), egui::Button::new("Sign in").fill(ACCENT()).corner_radius(radius(8))).clicked() {
                            let (flow, client, ctx) = (st.flow.clone(), st.account.client_id.trim().to_string(), ctx.clone());
                            let _ = sync::save_account(&st.account);
                            *flow.lock().unwrap() = Flow::Busy("Asking GitHub for a code…".into());
                            std::thread::spawn(move || {
                                let result = (|| -> Result<String, String> {
                                    let dc = sync::device_start(&client)?;
                                    *flow.lock().unwrap() = Flow::Code { user_code: dc.user_code.clone(), url: dc.url.clone() };
                                    ctx.request_repaint();
                                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(dc.expires_in);
                                    loop {
                                        std::thread::sleep(std::time::Duration::from_secs(dc.interval));
                                        if std::time::Instant::now() > deadline {
                                            return Err("The code ran out. Start again.".into());
                                        }
                                        if let Some(t) = sync::device_poll(&client, &dc.device_code)? {
                                            return Ok(t);
                                        }
                                    }
                                })();
                                let outcome = result.and_then(sync::sign_in);
                                *flow.lock().unwrap() = match outcome {
                                    Ok(a) => Flow::Busy(format!("Signed in as {}", a.login)),
                                    Err(e) => Flow::Failed(e),
                                };
                                ctx.request_repaint();
                            });
                        }
                        if gh_ok && ui.add(egui::Button::new("Use the GitHub CLI").fill(FIELD()).corner_radius(radius(8))).clicked() {
                            app.jobs.spawn(&ctx, "GitHub", || sync::token_from_gh().ok_or("The GitHub CLI is not signed in".to_string()).and_then(sync::sign_in).map(|a| format!("Signed in as {}", a.login)));
                        }
                    });
                    text_row(
                        ui,
                        "OAuth app client id",
                        "Once: on GitHub open Settings > Developer settings > OAuth Apps > New, tick \"Enable Device Flow\" and paste its client id here. Or sign in with the GitHub CLI instead.",
                        true,
                        &mut st.account.client_id,
                        "Ov23…",
                    );
                }
            }
        }
    });
    // after a sign-in the account is read again
    if matches!(flow, Flow::Busy(_)) && sync::load_account().token != st.account.token {
        st.account = sync::load_account();
        *st.flow.lock().unwrap() = Flow::Idle;
    }
    // what a job changed (the account after a sign-in, the presets after a download) shows within a moment
    if st.last_refresh.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(2)) {
        st.last_refresh = Some(std::time::Instant::now());
        st.account = sync::load_account();
        refresh = true;
    }
    ctx.request_repaint_after(std::time::Duration::from_secs(2));
    if refresh {
        st.list = presets::list();
    }
    app.presets = state;
    if let Some(t) = toast {
        app.say(t);
    }
}
