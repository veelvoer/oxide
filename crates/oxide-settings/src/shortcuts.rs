//! The Shortcuts page: readable key combinations grouped by what they do.
use std::mem::discriminant;

use eframe::egui::{self, Color32, RichText, Ui};
use oxide_config::{Action, Bind, Dir, MouseAction, MouseBind, MouseButton};

use crate::{theme::*, widgets::*, App};

#[derive(Default, Clone, Copy, PartialEq)]
enum Rec {
    #[default]
    Off,
    /// recording the keys that trigger the shortcut
    Trigger,
    /// recording the key a send-key shortcut presses
    Send,
}

#[derive(Default)]
pub struct State {
    editing: Option<usize>,
    rec: Rec,
    filter: String,
    /// the compositor is doing the capturing (otherwise this window listens itself)
    live_capture: bool,
}

struct Kind {
    label: &'static str,
    group: &'static str,
    make: fn() -> Action,
}

const KINDS: &[Kind] = &[
    Kind { label: "Run a command", group: "Apps and media", make: || Action::Exec(String::new()) },
    Kind { label: "Close window", group: "Windows", make: || Action::Close },
    Kind { label: "Float / tile window", group: "Windows", make: || Action::ToggleFloating },
    Kind { label: "Fullscreen window", group: "Windows", make: || Action::ToggleFullscreen },
    Kind { label: "Focus a window", group: "Windows", make: || Action::FocusDir(Dir::Left) },
    Kind { label: "Move window", group: "Windows", make: || Action::MoveDir(Dir::Left) },
    Kind { label: "Go to workspace number", group: "Workspaces", make: || Action::Workspace(1) },
    Kind { label: "Send window to workspace number", group: "Workspaces", make: || Action::MoveToWorkspace(1) },
    Kind { label: "Next / previous workspace", group: "Workspaces", make: || Action::WorkspaceRel(1) },
    Kind { label: "Next / previous workspace with windows", group: "Workspaces", make: || Action::WorkspaceExisting(1) },
    Kind { label: "Send window to next / previous workspace", group: "Workspaces", make: || Action::MoveToWorkspaceRel(1) },
    Kind { label: "Show / hide scratchpad", group: "Workspaces", make: || Action::ToggleSpecial },
    Kind { label: "Send window to scratchpad", group: "Workspaces", make: || Action::MoveToSpecial },
    Kind { label: "Show / hide a group of apps", group: "Workspaces", make: || Action::ToggleScratchpad(String::new()) },
    Kind { label: "Overview of all workspaces", group: "Workspaces", make: || Action::ToggleOverview },
    Kind { label: "Lock the screen", group: "Power", make: || Action::Lock },
    Kind { label: "Switch performance profile", group: "Power", make: || Action::SetProfile("performance".into()) },
    Kind { label: "Focus another screen", group: "Screens", make: || Action::FocusMonitor(Dir::Right) },
    Kind { label: "Send window to another screen", group: "Screens", make: || Action::MoveToMonitor(Dir::Right) },
    Kind { label: "Switch tiling layout", group: "Layout", make: || Action::CycleLayout },
    Kind { label: "Make main area bigger", group: "Layout", make: || Action::GrowMaster },
    Kind { label: "Make main area smaller", group: "Layout", make: || Action::ShrinkMaster },
    Kind { label: "Press another key", group: "Keys", make: || Action::SendKey(String::new()) },
    Kind { label: "Reload settings", group: "System", make: || Action::ReloadConfig },
    Kind { label: "Quit Oxide", group: "System", make: || Action::Quit },
];

const GROUPS: [&str; 7] = ["Apps and media", "Windows", "Workspaces", "Screens", "Layout", "Keys", "System"];

fn kind_of(a: &Action) -> usize {
    KINDS.iter().position(|k| discriminant(&(k.make)()) == discriminant(a)).unwrap_or(0)
}

fn dir_name(d: Dir) -> &'static str {
    match d {
        Dir::Left => "left",
        Dir::Right => "right",
        Dir::Up => "up",
        Dir::Down => "down",
    }
}

/// Plain-language name and icon for commands that Oxide itself ships binds for.
fn friendly_exec(c: &str) -> Option<(&'static str, &'static str)> {
    let table: &[(&str, &str, &str)] = &[
        ("oxide-ctl ipc ai toggle", "Open the assistant", "robot-outline"),
        ("oxide-ctl ipc shell toggleControl", "Open quick settings", "tune-variant"),
        ("oxide-ctl ipc shell toggleLauncher", "Open the app launcher", "apps"),
        ("oxide-ctl ipc shell toggleWindow", "Options for this window", "application-outline"),
        ("oxide-ctl ipc shell toggleClipboard", "Open clipboard history", "clipboard-text-outline"),
        ("oxide-ctl launcher", "Open the app launcher", "apps"),
        ("oxide-settings", "Open Settings", "cog-outline"),
        ("rofi -show drun", "App launcher (rofi)", "apps"),
        ("hyprlock", "Lock the screen", "lock-outline"),
        ("oxide-ctl lock", "Lock the screen", "lock-outline"),
        ("grim", "Screenshot of an area", "monitor-screenshot"),
        ("set-volume -l 1", "Volume up", "volume-high"),
        ("@DEFAULT_AUDIO_SINK@ 5%-", "Volume down", "volume-low"),
        ("set-mute @DEFAULT_AUDIO_SINK@", "Mute", "volume-off"),
        ("set-mute @DEFAULT_AUDIO_SOURCE@", "Mute the microphone", "microphone-off"),
        ("brightnessctl -e4 -n2 set 5%+", "Brightness up", "brightness-6"),
        ("brightnessctl -e4 -n2 set 5%-", "Brightness down", "brightness-6"),
        ("playerctl next", "Next track", "skip-next"),
        ("playerctl previous", "Previous track", "skip-previous"),
        ("playerctl play-pause", "Play or pause", "play"),
        ("oxide-ctl files", "Files", "folder-outline"),
        ("oxide-ctl terminal", "Terminal", "console-line"),
        ("kitty -e yazi", "Files (yazi in kitty)", "folder-open-outline"),
        ("kitty", "Terminal (kitty)", "console-line"),
    ];
    table.iter().find(|(k, _, _)| c.contains(k)).map(|(_, n, i)| (*n, *i))
}

fn action_icon(a: &Action) -> &'static str {
    match a {
        Action::Exec(c) => friendly_exec(c).map_or("console-line", |f| f.1),
        Action::Close => "close",
        Action::ToggleFloating => "application-outline",
        Action::ToggleFullscreen => "monitor",
        Action::FocusDir(_) => "cursor-default-click-outline",
        Action::MoveDir(_) => "arrow-expand-horizontal",
        Action::Workspace(_) | Action::MoveToWorkspace(_) | Action::WorkspaceRel(_) | Action::WorkspaceExisting(_) | Action::MoveToWorkspaceRel(_) => "view-grid-outline",
        Action::ToggleSpecial | Action::MoveToSpecial => "leaf",
        Action::ToggleScratchpad(_) => "layers-triple-outline",
        Action::ToggleOverview => "view-grid-outline",
        Action::SetProfile(_) => "speedometer",
        Action::Lock => "lock-outline",
        Action::FocusMonitor(_) | Action::MoveToMonitor(_) => "monitor-multiple",
        Action::CycleLayout | Action::GrowMaster | Action::ShrinkMaster => "view-dashboard-outline",
        Action::ReloadConfig => "refresh",
        Action::SendKey(_) => "keyboard-outline",
        Action::Quit => "power",
    }
}

fn describe(a: &Action) -> String {
    match a {
        Action::Exec(c) if c.is_empty() => "Run a command (not set)".into(),
        Action::Exec(c) => match friendly_exec(c) {
            Some((name, _)) => name.to_string(),
            None => format!("Run  {c}"),
        },
        Action::Close => "Close the window".into(),
        Action::ToggleFloating => "Float or tile the window".into(),
        Action::ToggleFullscreen => "Fullscreen the window".into(),
        Action::FocusDir(d) => format!("Focus the window to the {}", dir_name(*d)),
        Action::MoveDir(d) => format!("Move the window {}", dir_name(*d)),
        Action::Workspace(n) => format!("Go to workspace {n}"),
        Action::MoveToWorkspace(n) => format!("Send window to workspace {n}"),
        Action::WorkspaceRel(d) => if *d >= 0 { "Next workspace".into() } else { "Previous workspace".into() },
        Action::WorkspaceExisting(d) => {
            if *d >= 0 { "Next workspace with windows".into() } else { "Previous workspace with windows".into() }
        }
        Action::MoveToWorkspaceRel(d) => {
            if *d >= 0 { "Send window to next workspace".into() } else { "Send window to previous workspace".into() }
        }
        Action::ToggleSpecial => "Show or hide the scratchpad".into(),
        Action::MoveToSpecial => "Send window to the scratchpad".into(),
        Action::ToggleScratchpad(n) if n.is_empty() => "Show or hide a group (not set)".into(),
        Action::ToggleScratchpad(n) => format!("Show or hide the {n} group"),
        Action::ToggleOverview => "Overview of all workspaces".into(),
        Action::SetProfile(p) => format!("Performance profile: {p}"),
        Action::Lock => "Lock the screen".into(),
        Action::FocusMonitor(d) => format!("Focus the screen to the {}", dir_name(*d)),
        Action::MoveToMonitor(d) => format!("Send window to the screen to the {}", dir_name(*d)),
        Action::CycleLayout => "Switch tiling layout".into(),
        Action::GrowMaster => "Make the main area bigger".into(),
        Action::ShrinkMaster => "Make the main area smaller".into(),
        Action::ReloadConfig => "Reload settings".into(),
        Action::SendKey(c) if c.is_empty() => "Press another key (not set)".into(),
        Action::SendKey(c) => format!("Press {}", combo_pretty(c)),
        Action::Quit => "Quit Oxide".into(),
    }
}

/// "ctrl+shift+Left" -> "Ctrl + Shift + Left"
fn combo_pretty(c: &str) -> String {
    c.split('+')
        .map(|p| match p.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "super" | "logo" | "win" => "Super".to_string(),
            _ => pretty_key(p.trim()),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Ask Oxide for the next key combination (it holds it back from binds and apps, so Super works too).
fn start_capture(app: &mut App, what: Rec) {
    use oxide_ipc::{call, Request};
    app.shortcuts.rec = what;
    app.shortcuts.live_capture = call(&Request::CaptureKey).is_ok();
}

fn stop_capture(app: &mut App) {
    if app.shortcuts.live_capture {
        let _ = oxide_ipc::call(&oxide_ipc::Request::CancelCapture);
    }
    app.shortcuts.rec = Rec::Off;
    app.shortcuts.live_capture = false;
}

fn pretty_key(k: &str) -> String {
    match k {
        "Return" => "Enter".into(),
        "space" => "Space".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "minus" => "-".into(),
        "equal" => "=".into(),
        "slash" => "/".into(),
        "Print" => "PrtSc".into(),
        "Prior" => "PgUp".into(),
        "Next" => "PgDn".into(),
        "BackSpace" => "Backspace".into(),
        "XF86AudioRaiseVolume" => "Volume up".into(),
        "XF86AudioLowerVolume" => "Volume down".into(),
        "XF86AudioMute" => "Mute".into(),
        "XF86AudioMicMute" => "Mic mute".into(),
        "XF86MonBrightnessUp" => "Brightness up".into(),
        "XF86MonBrightnessDown" => "Brightness down".into(),
        "XF86AudioNext" => "Next track".into(),
        "XF86AudioPrev" => "Previous track".into(),
        "XF86AudioPlay" => "Play".into(),
        "XF86AudioPause" => "Pause".into(),
        s if s.len() == 1 => s.to_uppercase(),
        s => s.to_string(),
    }
}

fn mod_names(mods: &str) -> Vec<&'static str> {
    let has = |n: &str| mods.split('+').any(|p| p.trim().eq_ignore_ascii_case(n));
    let mut v = vec![];
    for (id, label) in [("SUPER", "Super"), ("CTRL", "Ctrl"), ("ALT", "Alt"), ("SHIFT", "Shift")] {
        if has(id) {
            v.push(label);
        }
    }
    v
}

/// Map an egui key to its xkb keysym name.
fn xkb_name(key: egui::Key) -> String {
    let n = format!("{key:?}");
    match n.as_str() {
        "Enter" => "Return",
        "Space" => "space",
        "Backspace" => "BackSpace",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "PageUp" => "Prior",
        "PageDown" => "Next",
        "Minus" => "minus",
        "Equals" => "equal",
        "Comma" => "comma",
        "Period" => "period",
        "Slash" => "slash",
        "Backslash" => "backslash",
        "Semicolon" => "semicolon",
        "Colon" => "colon",
        "OpenBracket" => "bracketleft",
        "CloseBracket" => "bracketright",
        "Backtick" => "grave",
        "Quote" => "apostrophe",
        s if s.len() == 1 => return s.to_lowercase(),
        s if s.starts_with("Num") && s.len() == 4 => return s[3..].to_string(),
        s => return s.to_string(),
    }
    .to_string()
}

fn chips(ui: &mut Ui, mods: &str, key: &str) {
    for m in mod_names(mods) {
        key_chip(ui, m, false);
        ui.label(RichText::new("+").color(MUTED()));
    }
    if key.is_empty() {
        key_chip(ui, "?", true);
    } else {
        key_chip(ui, &pretty_key(key), true);
    }
}

fn mods_editor(ui: &mut Ui, mods: &mut String) {
    let mut set = ["SUPER", "CTRL", "ALT", "SHIFT"].map(|n| mods.split('+').any(|p| p.trim().eq_ignore_ascii_case(n)));
    ui.horizontal(|ui| {
        for (i, n) in ["Super", "Ctrl", "Alt", "Shift"].iter().enumerate() {
            let on = set[i];
            let btn = egui::Button::new(RichText::new(*n).color(if on { ON_ACCENT() } else { TEXT() }))
                .fill(if on { ACCENT() } else { FIELD() });
            if ui.add(btn).clicked() {
                set[i] = !on;
            }
        }
    });
    *mods = ["SUPER", "CTRL", "ALT", "SHIFT"]
        .iter()
        .zip(set)
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join("+");
}

fn dir_combo(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, d: &mut Dir) {
    egui::ComboBox::from_id_salt(id).selected_text(dir_name(*d)).width(90.0).show_ui(ui, |ui| {
        for v in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
            ui.selectable_value(d, v, dir_name(v));
        }
    });
}

fn action_editor(ui: &mut Ui, id: usize, a: &mut Action) {
    let mut kind = kind_of(a);
    egui::ComboBox::from_id_salt(("kind", id)).selected_text(KINDS[kind].label).width(280.0).show_ui(ui, |ui| {
        for g in GROUPS {
            ui.label(RichText::new(g).small().color(MUTED()));
            for (k, info) in KINDS.iter().enumerate().filter(|(_, i)| i.group == g) {
                ui.selectable_value(&mut kind, k, info.label);
            }
        }
    });
    if kind != kind_of(a) {
        *a = (KINDS[kind].make)();
    }
    match a {
        Action::Exec(c) => {
            ui.add(egui::TextEdit::singleline(c).desired_width(320.0).hint_text("e.g. firefox"));
        }
        Action::ToggleScratchpad(n) => {
            ui.add(egui::TextEdit::singleline(n).desired_width(200.0).hint_text("group name (Settings > Apps)"));
        }
        Action::FocusDir(d) | Action::MoveDir(d) | Action::FocusMonitor(d) | Action::MoveToMonitor(d) => dir_combo(ui, ("dir", id), d),
        Action::Workspace(n) | Action::MoveToWorkspace(n) => {
            ui.add(egui::DragValue::new(n).range(1..=99).prefix("number "));
        }
        Action::WorkspaceRel(d) | Action::WorkspaceExisting(d) | Action::MoveToWorkspaceRel(d) => {
            let mut next = *d >= 0;
            egui::ComboBox::from_id_salt(("rel", id)).selected_text(if next { "next" } else { "previous" }).width(100.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut next, true, "next");
                ui.selectable_value(&mut next, false, "previous");
            });
            *d = if next { 1 } else { -1 };
        }
        _ => {}
    }
}

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "keyboard-variant", "Shortcuts", "Click one to change it");

    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.shortcuts.filter).hint_text("Search shortcuts").desired_width(260.0));
        if primary_button(ui, "New shortcut").clicked() {
            app.cfg.binds.push(Bind { mods: "SUPER".into(), key: String::new(), action: Action::Exec(String::new()) });
            app.shortcuts.editing = Some(app.cfg.binds.len() - 1);
            start_capture(app, Rec::Trigger);
            app.shortcuts.filter.clear();
        }
    });

    // key recording
    if app.shortcuts.rec != Rec::Off {
        let mut got: Option<(String, String)> = None;
        if app.shortcuts.live_capture {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
            match oxide_ipc::call(&oxide_ipc::Request::TakeCapture) {
                Ok(oxide_ipc::Response::Captured { combo }) => got = combo,
                Ok(oxide_ipc::Response::Error { .. }) => {
                    // cancelled with Escape
                    app.shortcuts.rec = Rec::Off;
                    app.shortcuts.live_capture = false;
                }
                _ => app.shortcuts.live_capture = false,
            }
        } else {
            // not connected to Oxide: listen to this window (no Super key here)
            got = ui.ctx().input(|inp| {
                inp.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } if *key != egui::Key::Escape => {
                        let mut m = vec![];
                        if modifiers.ctrl { m.push("CTRL"); }
                        if modifiers.alt { m.push("ALT"); }
                        if modifiers.shift { m.push("SHIFT"); }
                        Some((m.join("+"), xkb_name(*key)))
                    }
                    _ => None,
                })
            });
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                app.shortcuts.rec = Rec::Off;
            }
        }
        if let Some((mods, key)) = got {
            if let Some(b) = app.shortcuts.editing.and_then(|i| app.cfg.binds.get_mut(i)) {
                match app.shortcuts.rec {
                    Rec::Trigger => {
                        b.mods = mods;
                        b.key = key;
                    }
                    Rec::Send => {
                        let combo = if mods.is_empty() { key } else { format!("{mods}+{key}") };
                        b.action = Action::SendKey(combo);
                    }
                    Rec::Off => {}
                }
            }
            app.shortcuts.rec = Rec::Off;
            app.shortcuts.live_capture = false;
        }
    }

    let filter = app.shortcuts.filter.to_lowercase();
    let mut remove = None;
    let mut duplicate = None;
    let mut editing = app.shortcuts.editing;
    let mut rec = app.shortcuts.rec;
    let mut want_rec: Option<Rec> = None;

    for group in GROUPS {
        let members: Vec<usize> = app
            .cfg
            .binds
            .iter()
            .enumerate()
            .filter(|(_, b)| KINDS[kind_of(&b.action)].group == group)
            .filter(|(_, b)| {
                filter.is_empty() || describe(&b.action).to_lowercase().contains(&filter) || pretty_key(&b.key).to_lowercase().contains(&filter)
            })
            .map(|(i, _)| i)
            .collect();
        if members.is_empty() {
            continue;
        }
        card(ui, group, |ui| {
            for (n, &i) in members.iter().enumerate() {
                let last = n + 1 == members.len();
                let open = editing == Some(i);
                let b = &mut app.cfg.binds[i];
                // the whole row is clickable
                let resp = ui
                    .scope(|ui| {
                        ui.set_min_height(54.0);
                        ui.horizontal(|ui| {
                            ui.set_min_height(54.0);
                            // fixed-width key column so descriptions line up
                            ui.allocate_ui(egui::vec2(270.0, 54.0), |ui| {
                                ui.set_min_size(egui::vec2(270.0, 54.0));
                                ui.vertical(|ui| {
                                    ui.add_space(15.0);
                                    ui.horizontal(|ui| chips(ui, &b.mods, &b.key));
                                });
                            });
                            ui.add_space(4.0);
                            ui.vertical(|ui| {
                                ui.add_space(18.0);
                                crate::icons::icon(ui, action_icon(&b.action), 17.0, tint_for(action_icon(&b.action)));
                            });
                            ui.vertical(|ui| {
                                ui.add_space(15.0);
                                ui.label(describe(&b.action));
                            });
                        });
                    })
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    editing = if open { None } else { Some(i) };
                    rec = Rec::Off;
                }
                let t = tween(ui.ctx(), ("bind_open", i), open as u8 as f32, 0.2);
                if t > 0.01 {
                    ui.set_opacity(t);
                    egui::Frame::new().fill(SIDEBAR()).corner_radius(radius(10)).inner_margin(12).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Keys").small().color(MUTED()));
                        mods_editor(ui, &mut b.mods);
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut b.key).desired_width(120.0).hint_text("key"));
                            let on = rec == Rec::Trigger && open;
                            if ghost_button(ui, if on { "Press the keys... (Esc cancels)" } else { "Record keys" }).clicked() {
                                want_rec = Some(if on { Rec::Off } else { Rec::Trigger });
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(RichText::new("Does").small().color(MUTED()));
                        ui.horizontal_wrapped(|ui| action_editor(ui, i, &mut b.action));
                        if let Action::SendKey(combo) = &mut b.action {
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(combo).desired_width(180.0).hint_text("e.g. alt+Left"));
                                let on = rec == Rec::Send && open;
                                if ghost_button(ui, if on { "Press the keys... (Esc cancels)" } else { "Record the key to press" }).clicked() {
                                    want_rec = Some(if on { Rec::Off } else { Rec::Send });
                                }
                            });
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ghost_button(ui, "Duplicate").clicked() {
                                duplicate = Some(i);
                            }
                            if ghost_button(ui, "Delete").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.set_opacity(1.0);
                }
                if !last {
                    let y = ui.cursor().top();
                    ui.painter().hline(ui.max_rect().x_range(), y, egui::Stroke::new(1.0, BORDER().linear_multiply(0.7)));
                }
            }
        });
    }

    if let Some(i) = duplicate {
        let b = app.cfg.binds[i].clone();
        app.cfg.binds.insert(i + 1, b);
        editing = Some(i + 1);
    }
    if let Some(i) = remove {
        app.cfg.binds.remove(i);
        editing = None;
        rec = Rec::Off;
    }
    app.shortcuts.editing = editing;
    app.shortcuts.rec = rec;
    if rec == Rec::Off && app.shortcuts.live_capture {
        stop_capture(app);
    }
    match want_rec {
        Some(Rec::Off) => stop_capture(app),
        Some(r) => start_capture(app, r),
        None => {}
    }

    mouse_section(app, ui);
}

fn mouse_section(app: &mut App, ui: &mut Ui) {
    card(ui, "Mouse", |ui| {
        let mut remove = None;
        let n = app.cfg.mouse_binds.len();
        for (i, b) in app.cfg.mouse_binds.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.set_min_height(54.0);
                mods_editor(ui, &mut b.mods);
                ui.label(RichText::new("+ hold").color(MUTED()));
                egui::ComboBox::from_id_salt(("mbtn", i)).selected_text(format!("{:?} button", b.button)).width(120.0).show_ui(ui, |ui| {
                    for v in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
                        ui.selectable_value(&mut b.button, v, format!("{v:?} button"));
                    }
                });
                ui.label(RichText::new("to").color(MUTED()));
                let name = |a: MouseAction| match a {
                    MouseAction::MoveWindow => "move the window",
                    MouseAction::ResizeWindow => "resize the window",
                };
                egui::ComboBox::from_id_salt(("mact", i)).selected_text(name(b.action)).width(150.0).show_ui(ui, |ui| {
                    for v in [MouseAction::MoveWindow, MouseAction::ResizeWindow] {
                        ui.selectable_value(&mut b.action, v, name(v));
                    }
                });
                if ghost_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
            });
            if i + 1 < n {
                let y = ui.cursor().top();
                ui.painter().hline(ui.max_rect().x_range(), y, egui::Stroke::new(1.0, BORDER().linear_multiply(0.7)));
            }
        }
        if let Some(i) = remove {
            app.cfg.mouse_binds.remove(i);
        }
        ui.add_space(10.0);
        if ghost_button(ui, "Add mouse shortcut").clicked() {
            app.cfg.mouse_binds.push(MouseBind { mods: "SUPER".into(), button: MouseButton::Left, action: MouseAction::MoveWindow });
        }
        ui.add_space(10.0);
    });
}
