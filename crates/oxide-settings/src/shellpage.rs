//! Shell (Quickshell configs: install, switch, share) and Assistant (the AI) pages.
use std::path::{Path, PathBuf};

use eframe::egui::{self, vec2, Color32, RichText, Sense, Stroke, Ui};
use oxide_config::{ai::AiConfig, shells_dir};

use crate::{icons, system as sys, theme::*, widgets::*, App};

#[derive(Clone, Debug)]
pub struct ShellInfo {
    pub dir_name: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub preview: Option<PathBuf>,
}

#[derive(Default)]
pub struct ShellState {
    pub list: Vec<ShellInfo>,
    pub import_input: String,
    pub confirm_delete: Option<String>,
    pub drop_hover: bool,
    pub ai: Option<AiState>,
}

pub struct AiState {
    pub cfg: AiConfig,
    pub saved: AiConfig,
    pub show_key: bool,
    pub paste_failed: Option<String>,
}

pub fn scan() -> Vec<ShellInfo> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(shells_dir()) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if !p.join("shell.qml").is_file() {
            continue;
        }
        let dir_name = e.file_name().to_string_lossy().into_owned();
        let meta: serde_json::Value = std::fs::read_to_string(p.join("shell.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let preview = ["preview.png", "preview.jpg"].iter().map(|f| p.join(f)).find(|f| f.is_file());
        out.push(ShellInfo {
            name: meta["name"].as_str().unwrap_or(&dir_name).to_string(),
            author: meta["author"].as_str().unwrap_or("").to_string(),
            description: meta["description"].as_str().unwrap_or("").to_string(),
            dir_name,
            preview,
        });
    }
    out.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    out
}

// ------------------------------------------------------------------ importing

fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "shell".into() } else { s }
}

fn find_shell_root(dir: &Path, depth: u32) -> Option<PathBuf> {
    if dir.join("shell.qml").is_file() {
        return Some(dir.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    std::fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find_shell_root(&e.path(), depth - 1))
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(p),
    }
}

/// Install a shell from a folder, archive (.zip, .tar.gz), download URL or git repository.
pub fn import_shell(src: &str) -> Result<String, String> {
    let src = src.trim().trim_matches('\'').trim_matches('"');
    if src.is_empty() {
        return Err("Nothing to import".into());
    }
    let tmp = std::env::temp_dir().join(format!("oxide-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| -> Result<String, String> {
        let (root_candidates, stem): (PathBuf, String);
        if src.starts_with("http://") || src.starts_with("https://") {
            let url = src.trim_end_matches('/');
            let last = url.rsplit('/').next().unwrap_or("shell").to_string();
            let is_archive = [".zip", ".tar.gz", ".tgz", ".tar"].iter().any(|e| url.ends_with(e));
            if is_archive {
                let file = tmp.join(&last);
                sys::run("curl", &["-fsSL", "-o", &file.to_string_lossy(), url])?;
                let out = tmp.join("x");
                extract(&file, &out)?;
                stem = last.split('.').next().unwrap_or("shell").to_string();
                root_candidates = out;
            } else {
                let out = tmp.join("repo");
                sys::run("git", &["clone", "--depth", "1", url, &out.to_string_lossy()])?;
                let _ = std::fs::remove_dir_all(out.join(".git"));
                stem = last.trim_end_matches(".git").to_string();
                root_candidates = out;
            }
        } else {
            let p = expand(src);
            if p.is_dir() {
                stem = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "shell".into());
                root_candidates = p;
            } else if p.is_file() {
                let out = tmp.join("x");
                extract(&p, &out)?;
                stem = p.file_name().map(|n| n.to_string_lossy().split('.').next().unwrap_or("shell").to_string()).unwrap_or_else(|| "shell".into());
                root_candidates = out;
            } else {
                return Err(format!("{} does not exist", p.display()));
            }
        }
        let root = find_shell_root(&root_candidates, 2).ok_or("No shell.qml found in there. A shell is a folder with a shell.qml file.")?;
        // prefer the name from shell.json, then the folder / archive name
        let meta_name = std::fs::read_to_string(root.join("shell.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v["name"].as_str().map(str::to_string));
        let base = sanitize(&meta_name.unwrap_or(stem));
        let mut name = base.clone();
        let mut n = 2;
        while shells_dir().join(&name).exists() {
            name = format!("{base}-{n}");
            n += 1;
        }
        std::fs::create_dir_all(shells_dir()).map_err(|e| e.to_string())?;
        sys::run("cp", &["-a", &root.to_string_lossy(), &shells_dir().join(&name).to_string_lossy()])?;
        let _ = std::fs::remove_dir_all(shells_dir().join(&name).join(".git"));
        Ok(name)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

fn extract(file: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let name = file.to_string_lossy().to_lowercase();
    if name.ends_with(".zip") {
        sys::run("unzip", &["-q", &file.to_string_lossy(), "-d", &out.to_string_lossy()]).map(|_| ())
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") || name.ends_with(".tar") {
        sys::run("tar", &["-xf", &file.to_string_lossy(), "-C", &out.to_string_lossy()]).map(|_| ())
    } else {
        Err("Unsupported file type (use a folder, .zip or .tar.gz)".into())
    }
}

/// Export an installed shell as a zip next to the user's downloads.
fn export_shell(dir_name: &str) -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dest_dir = if Path::new(&format!("{home}/Downloads")).is_dir() { format!("{home}/Downloads") } else { home };
    let file = format!("{dest_dir}/oxide-shell-{dir_name}.zip");
    let _ = std::fs::remove_file(&file);
    let parent = shells_dir();
    let out = std::process::Command::new("zip").current_dir(&parent).args(["-qr", &file, dir_name]).output().map_err(|_| "zip is not installed".to_string())?;
    if out.status.success() { Ok(format!("Saved {file}")) } else { Err(String::from_utf8_lossy(&out.stderr).into_owned()) }
}

// ------------------------------------------------------------------ the Shell page

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "puzzle-outline", "Shell", "Bar, panels and assistant");
    let ctx = app.ctx.clone();

    // files dropped on the window
    app.shells.drop_hover = ctx.input(|i| !i.raw.hovered_files.is_empty());
    let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
    for p in dropped {
        let src = p.to_string_lossy().into_owned();
        app.jobs.spawn(&ctx, "Import", move || import_shell(&src).map(|n| format!("Added {n}")));
    }
    if app.shells.list.is_empty() {
        app.shells.list = scan();
    }

    card(ui, "", |ui| {
        toggle_row(ui, "Run a shell", "Starts your bar, panels and notifications with Oxide.", false, &mut app.cfg.shell.enabled);
        row_full(ui, Some("refresh"), "Restart shell", "", "Use after editing the shell's files.", true, |ui| {
            if ghost_button(ui, "Restart").clicked() {
                let _ = oxide_ipc::call(&oxide_ipc::Request::RestartShell);
                app.say("Shell restarted");
            }
        });
    });

    let active = app.cfg.shell.active.clone();
    let list = app.shells.list.clone();
    card(ui, "Installed", |ui| {
        if list.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new("No shells installed yet. Add one below.").color(MUTED()));
            ui.add_space(16.0);
        }
        for (n, s) in list.iter().enumerate() {
            let is_active = s.dir_name == active;
            let last = n + 1 == list.len();
            ui.horizontal(|ui| {
                ui.set_min_height(76.0);
                // thumbnail
                let (rect, _) = ui.allocate_exact_size(vec2(112.0, 63.0), Sense::hover());
                ui.painter().rect_filled(rect, radius(10), DEEP());
                match &s.preview {
                    Some(p) => {
                        ui.put(rect, egui::Image::new(format!("file://{}", p.display())).fit_to_exact_size(rect.size()).corner_radius(radius(10)));
                    }
                    None => icons::paint(ui, "puzzle-outline", egui::Rect::from_center_size(rect.center(), vec2(26.0, 26.0)), VIOLET()),
                }
                if is_active {
                    ui.painter().rect_stroke(rect, radius(10), Stroke::new(2.0, ACCENT()), egui::StrokeKind::Outside);
                }
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    // leave room for the buttons on the right
                    ui.set_max_width((ui.available_width() - 210.0).max(120.0));
                    ui.add_space(10.0);
                    ui.label(RichText::new(&s.name).family(medium()).size(16.0));
                    let sub = if s.author.is_empty() { s.description.clone() } else { format!("by {}  -  {}", s.author, s.description) };
                    ui.add(egui::Label::new(RichText::new(sub).small().color(MUTED())).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_active {
                        tag(ui, "active", GOOD());
                    } else if primary_button(ui, "Use").clicked() {
                        app.cfg.shell.active = s.dir_name.clone();
                        app.cfg.shell.enabled = true;
                        app.cfg.own.shell = s.dir_name != "oxide";
                        if app.cfg.own.shell && app.cfg.theme.preset != "auto" {
                            // a shell of your own brings its own wallpaper: the colors follow it
                            app.cfg.theme.preset = "auto".into();
                            app.cfg.theme.accent = None;
                        }
                        app.save();
                        app.say(format!("Switched to {}", s.name));
                    }
                    if app.shells.confirm_delete.as_deref() == Some(s.dir_name.as_str()) {
                        if ghost_button(ui, "Cancel").clicked() {
                            app.shells.confirm_delete = None;
                        }
                        if ui.add(egui::Button::new(RichText::new("Delete").color(ON_ACCENT()).family(medium())).fill(BAD())).clicked() {
                            let _ = std::fs::remove_dir_all(shells_dir().join(&s.dir_name));
                            app.shells.confirm_delete = None;
                            app.shells.list = scan();
                        }
                    } else if !is_active {
                        let del = ui.add(egui::Button::new("").fill(Color32::TRANSPARENT).min_size(vec2(32.0, 30.0)));
                        icons::paint(ui, "delete-outline", egui::Rect::from_center_size(del.rect.center(), vec2(18.0, 18.0)), MUTED());
                        if del.on_hover_text("Delete").clicked() {
                            app.shells.confirm_delete = Some(s.dir_name.clone());
                        }
                    }
                    if app.advanced {
                        let ex = ui.add(egui::Button::new("").fill(Color32::TRANSPARENT).min_size(vec2(32.0, 30.0)));
                        icons::paint(ui, "upload", egui::Rect::from_center_size(ex.rect.center(), vec2(18.0, 18.0)), MUTED());
                        if ex.on_hover_text("Export as .zip to share").clicked() {
                            let name = s.dir_name.clone();
                            app.jobs.spawn(&ctx, "Export", move || export_shell(&name));
                        }
                    }
                    let fo = ui.add(egui::Button::new("").fill(Color32::TRANSPARENT).min_size(vec2(32.0, 30.0)));
                    icons::paint(ui, "folder-open-outline", egui::Rect::from_center_size(fo.rect.center(), vec2(18.0, 18.0)), MUTED());
                    if fo.on_hover_text("Open the folder").clicked() {
                        let p = shells_dir().join(&s.dir_name);
                        let _ = std::process::Command::new("xdg-open").arg(p).spawn();
                    }
                });
            });
            if !last {
                let y = ui.cursor().top();
                let x = ui.max_rect().x_range();
                ui.painter().hline(x, y, Stroke::new(1.0, BORDER().linear_multiply(0.7)));
            }
        }
    });

    card(ui, "Add a shell", |ui| {
        ui.add_space(12.0);
        // drop zone
        let h = tween(ui.ctx(), "dropzone", if app.shells.drop_hover { 1.0 } else { 0.0 }, 0.15);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
        ui.painter().rect_filled(rect, radius(14), lerp_color(DEEP(), ACCENT().linear_multiply(0.22), h));
        let stroke_col = lerp_color(BORDER(), ACCENT(), h);
        // dashed outline
        let r = rect.shrink(1.0);
        let dash = |a: egui::Pos2, b: egui::Pos2| {
            ui.painter().add(egui::Shape::dashed_line(&[a, b], Stroke::new(1.5, stroke_col), 7.0, 5.0));
        };
        dash(r.left_top(), r.right_top());
        dash(r.right_top(), r.right_bottom());
        dash(r.right_bottom(), r.left_bottom());
        dash(r.left_bottom(), r.left_top());
        icons::paint(ui, "upload", egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.center().y - 14.0), vec2(30.0, 30.0)), lerp_color(MUTED(), ACCENT(), h));
        ui.painter().text(egui::pos2(rect.center().x, rect.center().y + 20.0), egui::Align2::CENTER_CENTER, "Drop a folder or .zip here", egui::FontId::proportional(14.5), lerp_color(MUTED(), TEXT(), h));
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            let w = ui.available_width() - 110.0;
            let r = ui.add(egui::TextEdit::singleline(&mut app.shells.import_input).hint_text("Folder, .zip, download link or git address").desired_width(w));
            let go = primary_button(ui, "Import").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if go && !app.shells.import_input.trim().is_empty() {
                let src = std::mem::take(&mut app.shells.import_input);
                app.jobs.spawn(&ctx, "Import", move || import_shell(&src).map(|n| format!("Added {n}")));
            }
        });
        ui.add_space(8.0);
        let own = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/quickshell/shell.qml");
        if own.is_file() {
            row_full(ui, Some("import"), "Your Quickshell config", "", "Copies ~/.config/quickshell so you can try it with Oxide.", true, |ui| {
                if ghost_button(ui, "Import").clicked() {
                    let src = own.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    app.jobs.spawn(&ctx, "Import", move || import_shell(&src).map(|n| format!("Added {n}")));
                }
            });
        }
        row_full(ui, Some("folder-open-outline"), "Shells folder", &shells_dir().display().to_string(), "", true, |ui| {
            if ghost_button(ui, "Open").clicked() {
                let _ = std::fs::create_dir_all(shells_dir());
                let _ = std::process::Command::new("xdg-open").arg(shells_dir()).spawn();
            }
        });
    });
    if app.jobs.busy() {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Working...").color(MUTED()));
        });
    }
}

// ------------------------------------------------------------------ the Assistant page

const MODELS: [(&str, &str, &str); 4] = [
    ("claude-haiku-5-5", "Haiku 5.5", "Fastest and cheapest. The default."),
    ("claude-sonnet-5-5", "Sonnet 5.5", "Smarter, a bit slower."),
    ("claude-opus-5-5", "Opus 5.5", "Most capable for hard tasks."),
    ("claude-fable-5-1", "Fable 5.1", "Most powerful, slower, pricier."),
];

pub fn assistant(app: &mut App, ui: &mut Ui) {
    page_header(ui, "robot-outline", "Assistant", "AI that can control this computer");
    let ctx = app.ctx.clone();
    let adv = app.advanced;
    let mut toast: Option<String> = None;
    let st = app.shells.ai.get_or_insert_with(|| {
        let c = AiConfig::load();
        AiState { saved: c.clone(), cfg: c, show_key: false, paste_failed: None }
    });
    let has_key = st.cfg.key().is_some();

    card(ui, "", |ui| {
        row_full(ui, Some("robot-outline"), "Press Super+A to talk to it", "", "Opens the assistant panel in the shell. It can change settings, start apps, adjust volume, manage Wi-Fi and more.", true, |ui| {
            tag(ui, if has_key { "ready" } else { "needs a key" }, if has_key { GOOD() } else { WARN() });
        });
    });

    card(ui, "Name", |ui| {
        text_row(ui, "Its name", "What you call it. It answers to it and shows it in the chat.", true, &mut st.cfg.name, "Oxide");
    });

    card(ui, "Anthropic account", |ui| {
        row_full(ui, Some("key-variant"), "API key", "", "Stored in ~/.config/oxide/ai.toml, readable only by you. Or set ANTHROPIC_API_KEY.", false, |ui| {
            // Ctrl+V depends on the toolkit's clipboard; this reads the clipboard directly
            if ghost_button(ui, "Paste").on_hover_text("Fill in the key from your clipboard").clicked() {
                st.paste_failed = None;
                let run = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success() && !o.stdout.is_empty() && o.stdout.len() < 1000 && std::str::from_utf8(&o.stdout).is_ok());
                // the live clipboard first; if its owner is gone, fall back to the newest entry of the clipboard history
                let got = run("wl-paste", &["--no-newline"]).or_else(|| run("oxide-ctl", &["clip", "latest"]));
                match got {
                    Some(o) => st.cfg.api_key = String::from_utf8_lossy(&o.stdout).trim().to_string(),
                    None => st.paste_failed = Some("Nothing to paste: copy the key first.".into()),
                }
            }
            let eye = ui.add(egui::Button::new("").fill(Color32::TRANSPARENT).min_size(vec2(30.0, 28.0)));
            icons::paint(ui, if st.show_key { "eye-off-outline" } else { "eye-outline" }, egui::Rect::from_center_size(eye.rect.center(), vec2(18.0, 18.0)), MUTED());
            if eye.clicked() {
                st.show_key = !st.show_key;
            }
            ui.add(egui::TextEdit::singleline(&mut st.cfg.api_key).password(!st.show_key).hint_text("sk-ant-...").desired_width(260.0));
        });
        if let Some(m) = &st.paste_failed {
            ui.label(RichText::new(m.as_str()).color(WARN()).size(12.0));
        }
        row_full(ui, Some("lock-outline"), "Get a key", "", "Create one in the Anthropic console.", true, |ui| {
            if ghost_button(ui, "Open console").clicked() {
                let _ = std::process::Command::new("xdg-open").arg("https://console.anthropic.com/settings/keys").spawn();
            }
        });
    });

    card(ui, "Model", |ui| {
        for (i, (id, name, blurb)) in MODELS.iter().enumerate() {
            let selected = st.cfg.model == *id;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
            let t = tween(ui.ctx(), ("model", i), if selected { 1.0 } else if resp.hovered() { 0.4 } else { 0.0 }, 0.14);
            ui.painter().rect_filled(rect.shrink2(vec2(0.0, 3.0)), radius(12), lerp_color(Color32::TRANSPARENT, ACCENT().linear_multiply(0.18), t));
            ui.painter().text(egui::pos2(rect.left() + 14.0, rect.center().y - 8.0), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(15.0), TEXT());
            ui.painter().text(egui::pos2(rect.left() + 14.0, rect.center().y + 10.0), egui::Align2::LEFT_CENTER, *blurb, egui::FontId::proportional(12.0), MUTED());
            if selected {
                icons::paint(ui, "check", egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, rect.center().y), vec2(20.0, 20.0)), ACCENT());
            }
            if resp.clicked() {
                st.cfg.model = id.to_string();
            }
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        if adv && !st.cfg.model.contains("haiku") {
            let opts = [("low".to_string(), "Quick"), ("medium".to_string(), "Balanced"), ("high".to_string(), "Thorough")];
            choice_row(ui, "Thinking", "How hard it thinks before answering.", true, &mut st.cfg.effort, &opts);
        }
    });

    card(ui, "What it may do without asking", |ui| {
        let opts = [("ask".to_string(), "Ask every time"), ("safe".to_string(), "Ask for risky"), ("full".to_string(), "Never ask")];
        choice_row(ui, "Permission", "Ask every time: you approve each action. Ask for risky: reading and simple window actions run directly. Never ask: full control.", !adv, &mut st.cfg.permission, &opts);
        if st.cfg.permission == "full" {
            row_full(ui, Some("alert-circle-outline"), "Full control is on", "The assistant can run any command without asking. Only use this if you trust it.", "", !adv, |_| {});
        }
        if adv {
            toggle_row(ui, "Run commands", "Lets it use the shell.", false, &mut st.cfg.allow_shell);
            toggle_row(ui, "Use mouse and keyboard", "Lets it click and type for you.", false, &mut st.cfg.allow_input);
            toggle_row(ui, "Look at the screen", "Lets it take screenshots.", false, &mut st.cfg.allow_screenshot);
            toggle_row(ui, "Change Oxide settings", "Lets it edit your desktop settings.", true, &mut st.cfg.allow_config);
        }
    });

    if adv {
        card(ui, "Cost and memory", |ui| {
        let mut turns = st.cfg.history_turns as i64;
        slider_row(ui, "Conversation memory", "How many of your last requests it keeps in mind. Fewer is cheaper.", false, &mut turns, 1..=20, "");
        st.cfg.history_turns = turns as u32;
        let mut cap = st.cfg.cost_cap;
        slider_row(ui, "Cost limit per request", "It stops when one request has cost this much.", false, &mut cap, 0.05..=5.0, " $");
        st.cfg.cost_cap = (cap * 100.0).round() / 100.0;
        toggle_row(ui, "Learn about me", "Remembers lasting facts, like the apps you prefer. They live in ~/.config/oxide/ai-memory.json.", false, &mut st.cfg.learn);
        toggle_row(ui, "Web search", "Lets it look things up online.", true, &mut st.cfg.allow_web);
    });

    card(ui, "Advanced", |ui| {
            row_full(ui, Some("text-box-outline"), "Extra instructions", "", "Added to its built-in instructions, e.g. your preferred apps.", false, |_| {});
            ui.add(egui::TextEdit::multiline(&mut st.cfg.instructions).desired_rows(3).desired_width(f32::INFINITY).hint_text("Example: Always use firefox for browsing."));
            ui.add_space(10.0);
            text_row(ui, "API address", "Change only for a proxy or compatible service.", false, &mut st.cfg.base_url, "https://api.anthropic.com");
            toggle_row(ui, "Retry on another model", "If a safety check declines a request, let the API try another model.", true, &mut st.cfg.fallbacks);
        });
    }

    card(ui, "Use it in your own Quickshell", |ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("The assistant is two small files you can copy into any Quickshell config:").small().color(MUTED()));
        ui.add_space(6.0);
        let snippet = "AIClient { id: ai }            // AIClient.qml\nAIPanel { ai: ai; shown: open } // AIPanel.qml (optional UI)\nai.send(\"turn the volume down\")";
        egui::Frame::new().fill(DEEP()).corner_radius(radius(10)).inner_margin(10).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(snippet).monospace().small().color(COPPER()));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ghost_button(ui, "Copy snippet").clicked() {
                ui.ctx().copy_text(snippet.to_string());
                toast = Some("Snippet copied".into());
            }
            if ghost_button(ui, "Open the files").clicked() {
                let _ = std::process::Command::new("xdg-open").arg(shells_dir().join(&app.cfg.shell.active)).spawn();
            }
        });
        ui.add_space(10.0);
    });

    // save / test bar for this page
    let st = app.shells.ai.as_mut().unwrap();
    // changes are kept as you make them (like the other pages), once a slider or drag is released
    if st.cfg != st.saved && !ctx.input(|i| i.pointer.any_down()) && st.cfg.save().is_ok() {
        st.saved = st.cfg.clone();
    }
    let changed = st.cfg != st.saved;
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(changed, egui::Button::new(RichText::new("Save").color(ON_ACCENT()).family(medium())).fill(ACCENT())).clicked() {
            match st.cfg.save() {
                Ok(()) => {
                    st.saved = st.cfg.clone();
                    toast = Some("Assistant settings saved".into());
                }
                Err(e) => toast = Some(format!("Could not save: {e}")),
            }
        }
        if ghost_button(ui, "Test connection").clicked() {
            let cfg = st.cfg.clone();
            let _ = cfg.save();
            st.saved = st.cfg.clone();
            app.jobs.spawn(&ctx, "Assistant", || {
                let out = std::process::Command::new("oxide-ai").arg("--check").output().map_err(|_| "oxide-ai is not installed".to_string())?;
                let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| "no answer".to_string())?;
                if v["ok"].as_bool() == Some(true) { Ok(v["message"].as_str().unwrap_or("Connected").to_string()) } else { Err(v["message"].as_str().unwrap_or("failed").to_string()) }
            });
        }
        if changed {
            ui.label(RichText::new("unsaved").color(WARN()));
        }
    });
    if let Some(t) = toast {
        app.say(t);
    }
}
