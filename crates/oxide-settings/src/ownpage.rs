//! Own configs (Advanced mode): choose which parts use your own files instead of Oxide's standard setup, import
//! files and edit them in the built-in editor.
use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, Ui};
use oxide_config::{defaults, Config};

use crate::{editor::Event, theme::*, widgets::*, App};

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn xdg_config() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

/// The files of one config group, with whether they may be edited.
fn files(kind: &str, cfg: &Config) -> Vec<(PathBuf, bool)> {
    let own = |b: bool| !b; // readonly when it is Oxide's generated file
    match kind {
        "kitty" => {
            if cfg.own.kitty {
                let dir = xdg_config().join("kitty");
                let mut v = vec![(dir.join("kitty.conf"), false)];
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    let mut extra: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "conf") && p.file_name().is_some_and(|n| n != "kitty.conf")).collect();
                    extra.sort();
                    v.extend(extra.into_iter().map(|p| (p, false)));
                }
                v
            } else {
                vec![(defaults::kitty_file(), own(false))]
            }
        }
        "fish" => {
            if cfg.own.fish {
                let dir = xdg_config().join("fish");
                let mut v = vec![(dir.join("config.fish"), false)];
                if let Ok(rd) = std::fs::read_dir(dir.join("conf.d")) {
                    let mut extra: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "fish")).collect();
                    extra.sort();
                    v.extend(extra.into_iter().map(|p| (p, false)));
                }
                v
            } else {
                vec![(defaults::fish_file(), own(false))]
            }
        }
        "gtk" => vec![
            (xdg_config().join("gtk-3.0/gtk.css"), false),
            (xdg_config().join("gtk-3.0/settings.ini"), false),
            (xdg_config().join("gtk-4.0/gtk.css"), false),
            (xdg_config().join("gtk-4.0/settings.ini"), false),
        ],
        "shell" => {
            let dir = oxide_config::shells_dir().join(&cfg.shell.active);
            let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| matches!(e.to_str(), Some("qml" | "js" | "json"))) || p.file_name().is_some_and(|n| n == "qmldir"))
                .collect();
            v.sort();
            v.into_iter().map(|p| (p, false)).collect()
        }
        "oxide" => vec![(Config::path(), false)],
        _ => vec![],
    }
}

/// Ask the user for a file with whatever picker is installed.
fn pick_file() -> Option<PathBuf> {
    for (bin, args) in [("kdialog", vec!["--getopenfilename", ".", "*"]), ("zenity", vec!["--file-selection"])] {
        if let Ok(o) = std::process::Command::new(bin).args(&args).output() {
            if o.status.success() {
                let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(PathBuf::from(p));
                }
            }
        }
    }
    None
}

/// Put `src` in place of `dest` (the old file is kept as .oxide-bak).
fn import(src: &Path, dest: &Path) -> Result<(), String> {
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        let bak = dest.with_extension(format!("{}.oxide-bak", dest.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()));
        let _ = std::fs::copy(dest, bak);
    }
    std::fs::copy(src, dest).map(|_| ()).map_err(|e| e.to_string())
}

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "", "Own configs", "Everything starts with Oxide's standard setup. Switch a part to your own files here, then edit or import them.");

    let mut open: Option<(PathBuf, bool)> = None;
    let mut imported: Option<String> = None;

    struct Item {
        kind: &'static str,
        name: &'static str,
        standard: &'static str,
        own: &'static str,
    }
    let items = [
        Item { kind: "shell", name: "Shell (bar, launcher, panels)", standard: "The standard Oxide shell.", own: "A Quickshell setup of your own (pick it on the Shells page)." },
        Item { kind: "kitty", name: "Terminal (kitty)", standard: "Oxide's kitty setup (Settings > Terminal).", own: "~/.config/kitty/kitty.conf" },
        Item { kind: "fish", name: "Shell prompt (fish)", standard: "Oxide's fish setup with the theme's colors.", own: "~/.config/fish/config.fish" },
        Item { kind: "gtk", name: "GTK apps", standard: "Colored by Oxide to match the theme.", own: "Your own gtk.css and settings.ini." },
    ];

    card(ui, "Use my own", |ui| {
        let n = items.len();
        for (i, it) in items.iter().enumerate() {
            let flag: &mut bool = match it.kind {
                "shell" => &mut app.cfg.own.shell,
                "kitty" => &mut app.cfg.own.kitty,
                "fish" => &mut app.cfg.own.fish,
                _ => &mut app.cfg.own.gtk,
            };
            let on = *flag;
            let sub = if on { it.own } else { it.standard };
            row_full(ui, None, it.name, sub, "", i + 1 == n, |ui| {
                switch(ui, flag);
            });
            let _ = on;
        }
    });

    let cfg = app.cfg.clone();
    card(ui, "Files", |ui| {
        let groups: [(&str, &str, bool); 5] = [
            ("oxide", "Oxide settings (oxide.toml)", true),
            ("kitty", "kitty", true),
            ("fish", "fish", true),
            ("gtk", "GTK", cfg.own.gtk),
            ("shell", "Shell files", cfg.own.shell),
        ];
        for (kind, label, show) in groups {
            if !show {
                continue;
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(label).family(medium()).color(MUTED()));
                for (path, readonly) in files(kind, &cfg) {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let exists = path.exists();
                    let text = if exists { name } else { format!("{name} (new)") };
                    if ui.add(egui::Button::new(RichText::new(text).size(13.0)).fill(FIELD()).corner_radius(radius(4))).on_hover_text(path.display().to_string()).clicked() {
                        open = Some((path.clone(), readonly));
                    }
                }
                // import a file over the main one of this group (the standard setups are generated, not imported)
                let importable = match kind {
                    "kitty" => cfg.own.kitty,
                    "fish" => cfg.own.fish,
                    "gtk" => cfg.own.gtk,
                    _ => false,
                };
                if importable && ghost_button(ui, "Import a file...").clicked() {
                    if let (Some(src), Some((dest, _))) = (pick_file(), files(kind, &cfg).into_iter().next()) {
                        imported = Some(match import(&src, &dest) {
                            Ok(()) => {
                                open = Some((dest.clone(), false));
                                format!("Imported {} to {}", src.display(), dest.display())
                            }
                            Err(e) => format!("Could not import: {e}"),
                        });
                    }
                }
            });
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Any other file").color(MUTED()));
            ui.add(egui::TextEdit::singleline(&mut app.own_path).desired_width(360.0).hint_text("/home/you/.config/something.conf"));
            if ghost_button(ui, "Open").clicked() && !app.own_path.trim().is_empty() {
                let p = if let Some(rest) = app.own_path.trim().strip_prefix("~/") { home().join(rest) } else { PathBuf::from(app.own_path.trim()) };
                open = Some((p, false));
            }
        });
        ui.add_space(6.0);
    });

    let just_opened = open.is_some();
    if let Some((p, ro)) = open {
        app.editor.open(&p, ro);
    }
    if let Some(m) = imported {
        app.say(m);
    }

    ui.add_space(12.0);
    let h = (ui.available_height() - 20.0).max(420.0);
    if just_opened {
        let top = ui.cursor().min;
        ui.scroll_to_rect(egui::Rect::from_min_size(top, egui::vec2(10.0, h)), Some(egui::Align::TOP));
    }
    let ev = app.editor.ui(ui, h);
    if let Some(m) = app.editor.message.take() {
        app.say(m);
    }
    if let Event::Saved(path) = ev {
        after_save(app, &path);
    }
    ui.add_space(20.0);
}

/// Make a saved file take effect.
fn after_save(app: &mut App, path: &Path) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if path == Config::path() {
        // the file was edited by hand: take it over instead of overwriting it with the old state
        app.adopt_config_from_disk();
    } else if path.starts_with(xdg_config().join("kitty")) || path.starts_with(oxide_config::defaults::kitty_dir()) {
        let _ = std::process::Command::new("pkill").args(["-USR1", "-x", "kitty"]).status();
    } else if path.starts_with(oxide_config::shells_dir()) {
        let _ = std::process::Command::new("oxide-ctl").arg("restart-shell").status();
    } else if name.ends_with(".css") || name.ends_with(".ini") {
        app.say("Saved. GTK apps read this when they start.");
    }
}
