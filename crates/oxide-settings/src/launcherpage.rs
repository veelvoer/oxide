//! Launcher: what the Oxide launcher searches, and which keys open it.
use eframe::egui::{RichText, Ui};
use oxide_config::Action;
use serde_json::{json, Value};

use crate::{theme::*, widgets::*, App};

fn path() -> std::path::PathBuf {
    oxide_config::Config::path().with_file_name("launcher.json")
}

const ENGINES: &[(&str, &str)] = &[
    ("DuckDuckGo", "https://duckduckgo.com/?q=%s"),
    ("Google", "https://www.google.com/search?q=%s"),
    ("Brave", "https://search.brave.com/search?q=%s"),
    ("Startpage", "https://www.startpage.com/do/search?q=%s"),
];

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "", "Launcher", "Search apps, do quick math, run commands. Opens with Super+R, Alt+Space or Super+D.");

    let mut v: Value = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    let before = v.clone();
    let flag = |v: &Value, k: &str| v[k].as_bool().unwrap_or(true);

    card(ui, "Search", |ui| {
        for (key, name, help) in [
            ("calculator", "Calculator", "Type 12*(3+4) and press Enter to copy the result."),
            ("history", "Remember what I start", "Needed for the order below; turn off to list apps alphabetically."),
            ("web", "Web search", "Offer a web search when nothing else matches."),
        ] {
            let mut on = flag(&v, key);
            toggle_row(ui, name, help, false, &mut on);
            v[key] = json!(on);
        }
        let mut sort = v["sort"].as_str().unwrap_or("used").to_string();
        choice_row(ui, "Order", "Most used puts the apps you start most at the top; most recent puts the last ones first.", false, &mut sort, &[("used".to_string(), "Most used"), ("recent".to_string(), "Most recent")]);
        v["sort"] = json!(sort);
        let url = v["searchUrl"].as_str().unwrap_or(ENGINES[0].1).to_string();
        let mut cur = ENGINES.iter().find(|e| e.1 == url).map(|e| e.1.to_string()).unwrap_or(url);
        let opts: Vec<(String, &str)> = ENGINES.iter().map(|(n, u)| (u.to_string(), *n)).collect();
        choice_row(ui, "Search engine", "", false, &mut cur, &opts);
        v["searchUrl"] = json!(cur);
        let mut n = v["maxResults"].as_i64().unwrap_or(7);
        slider_row(ui, "Results shown", "", true, &mut n, 3..=12, "");
        v["maxResults"] = json!(n);
    });

    card(ui, "Keys", |ui| {
        let uses = |a: &Action| matches!(a, Action::Exec(c) if c.trim() == "oxide-ctl launcher");
        let mut on = app.cfg.binds.iter().any(|b| uses(&b.action));
        let old = on;
        toggle_row(ui, "Use it for Super+R and Alt+Space", "Turn off to go back to rofi for those keys.", false, &mut on);
        if on != old {
            for b in app.cfg.binds.iter_mut() {
                let rofi = matches!(&b.action, Action::Exec(c) if c.trim() == "rofi -show drun");
                if on && rofi || !on && uses(&b.action) {
                    b.action = Action::Exec(if on { "oxide-ctl launcher" } else { "rofi -show drun" }.into());
                }
            }
        }
        row_full(ui, None, "Recent history", "Forget which apps you started.", "", true, |ui| {
            if ghost_button(ui, "Clear").clicked() {
                let _ = std::fs::remove_file(path().with_file_name("launcher-history.json"));
                app.say("History cleared");
            }
        });
    });

    ui.add_space(8.0);
    ui.label(RichText::new("Start a line with > to run a shell command.").color(MUTED()).size(12.0));

    if v != before {
        if let Some(d) = path().parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(path(), serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}
