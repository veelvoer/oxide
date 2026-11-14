//! Best-effort import of a `hyprland.conf` into an Oxide [`Config`].
use std::collections::HashMap;

use crate::{Action, Bind, Config, Dir, LayoutKind, Monitor, MouseAction, MouseBind, MouseButton, WindowRule};

/// Result of an import: the config plus lines that could not be mapped.
pub struct Imported {
    pub config: Config,
    pub skipped: Vec<String>,
}

fn color(v: &str) -> Option<String> {
    // rgba(33ccffee) / rgb(33ccff) / 0xeeRRGGBB; gradients: take the first color
    let first = v.split_whitespace().next()?;
    if let Some(h) = first.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        return Some(format!("#{h}"));
    }
    if let Some(h) = first.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        return Some(format!("#{h}"));
    }
    if let Some(h) = first.strip_prefix("0x") {
        if h.len() == 8 {
            return Some(format!("#{}{}", &h[2..], &h[..2]));
        }
    }
    None
}

fn boolean(v: &str) -> bool {
    matches!(v.trim(), "true" | "yes" | "on" | "1")
}

fn dir(s: &str) -> Option<Dir> {
    match s.trim() {
        "l" | "left" => Some(Dir::Left),
        "r" | "right" => Some(Dir::Right),
        "u" | "up" => Some(Dir::Up),
        "d" | "down" => Some(Dir::Down),
        _ => None,
    }
}

fn mods(s: &str) -> String {
    s.split(|c: char| c == '+' || c.is_whitespace())
        .filter(|m| !m.is_empty())
        .map(|m| match m.to_ascii_uppercase().as_str() {
            "SUPER" | "WIN" | "MOD4" | "LOGO" => "SUPER",
            "CTRL" | "CONTROL" => "CTRL",
            "ALT" | "MOD1" => "ALT",
            "SHIFT" => "SHIFT",
            _ => "",
        })
        .filter(|m| !m.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

fn dispatcher(name: &str, arg: &str) -> Option<Action> {
    let arg = arg.trim();
    Some(match name.trim() {
        "exec" | "execr" => Action::Exec(arg.to_string()),
        "killactive" => Action::Close,
        "togglefloating" => Action::ToggleFloating,
        "fullscreen" => Action::ToggleFullscreen,
        "movefocus" => Action::FocusDir(dir(arg)?),
        "movewindow" => Action::MoveDir(dir(arg)?),
        "workspace" => match arg {
            "+1" => Action::WorkspaceRel(1),
            "-1" => Action::WorkspaceRel(-1),
            "e+1" => Action::WorkspaceExisting(1),
            "e-1" => Action::WorkspaceExisting(-1),
            _ => Action::Workspace(arg.parse().ok()?),
        },
        "togglespecialworkspace" => Action::ToggleSpecial,
        "movetoworkspace" | "movetoworkspacesilent" => match arg {
            "+1" => Action::MoveToWorkspaceRel(1),
            "-1" => Action::MoveToWorkspaceRel(-1),
            a if a.starts_with("special") => Action::MoveToSpecial,
            _ => Action::MoveToWorkspace(arg.parse().ok()?),
        },
        "exit" => Action::Quit,
        "togglesplit" | "layoutmsg" => Action::CycleLayout,
        _ => return None,
    })
}

pub fn import(text: &str) -> Imported {
    let mut cfg = Config::default();
    cfg.binds.clear();
    cfg.mouse_binds.clear();
    cfg.exec_once.clear();
    cfg.monitors.clear();
    cfg.rules.clear();
    let mut skipped = vec![];
    let mut vars: HashMap<String, String> = HashMap::new();
    let mut path: Vec<String> = vec![];

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_suffix('{') {
            path.push(name.trim().to_string());
            continue;
        }
        if line == "}" {
            path.pop();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, mut v) = (k.trim().to_string(), v.trim().to_string());
        for (name, val) in &vars {
            v = v.replace(name.as_str(), val);
        }
        if k.starts_with('$') {
            vars.insert(k, v);
            continue;
        }
        let section = path.join(".");
        let ok = match (section.as_str(), k.as_str()) {
            ("general", "gaps_in") => v.parse().map(|n| cfg.general.gaps_in = n).is_ok(),
            ("general", "gaps_out") => v.parse().map(|n| cfg.general.gaps_out = n).is_ok(),
            ("general", "border_size") => v.parse().map(|n| cfg.general.border_size = n).is_ok(),
            ("general", "col.active_border") => {
                // every color of a gradient, then an optional "45deg"
                let cols: Vec<String> = v.split_whitespace().filter_map(color).collect();
                if let Some(deg) = v.split_whitespace().find_map(|w| w.strip_suffix("deg").and_then(|d| d.parse().ok())) {
                    cfg.general.border_angle = deg;
                }
                if cols.is_empty() {
                    false
                } else {
                    cfg.general.active_border = cols;
                    true
                }
            }
            ("general", "col.inactive_border") => color(&v).map(|c| cfg.general.inactive_border = c).is_some(),
            ("general", "layout") => {
                cfg.general.layout = if v == "master" { LayoutKind::Master } else { LayoutKind::Dwindle };
                true
            }
            ("decoration", "rounding") => v.parse().map(|n| cfg.decoration.rounding = n).is_ok(),
            ("decoration", "active_opacity") => v.parse().map(|n| cfg.decoration.active_opacity = n).is_ok(),
            ("decoration", "inactive_opacity") => v.parse().map(|n| cfg.decoration.inactive_opacity = n).is_ok(),
            ("decoration", "dim_inactive") => {
                if boolean(&v) {
                    cfg.decoration.dim_inactive = 0.3;
                }
                true
            }
            ("decoration.blur", "enabled") => {
                cfg.decoration.blur.enabled = boolean(&v);
                true
            }
            ("decoration.blur", "size") => v.parse().map(|n| cfg.decoration.blur.size = n).is_ok(),
            ("decoration.blur", "passes") => v.parse().map(|n| cfg.decoration.blur.passes = n).is_ok(),
            ("decoration.shadow", "enabled") => {
                cfg.decoration.shadow.enabled = boolean(&v);
                true
            }
            ("decoration.shadow", "range") => v.parse().map(|n| cfg.decoration.shadow.range = n).is_ok(),
            ("animations", "enabled") => {
                cfg.animations.enabled = boolean(&v);
                true
            }
            ("input", "kb_layout") => {
                cfg.input.kb_layout = v;
                true
            }
            ("input", "kb_variant") => {
                cfg.input.kb_variant = v;
                true
            }
            ("input", "kb_options") => {
                cfg.input.kb_options = v;
                true
            }
            ("input", "repeat_rate") => v.parse().map(|n| cfg.input.repeat_rate = n).is_ok(),
            ("input", "repeat_delay") => v.parse().map(|n| cfg.input.repeat_delay = n).is_ok(),
            ("input", "sensitivity") => v.parse().map(|n| cfg.input.sensitivity = n).is_ok(),
            ("input", "follow_mouse") => {
                cfg.general.focus_follows_mouse = v != "0";
                true
            }
            ("input.touchpad", "natural_scroll") | ("input", "natural_scroll") => {
                cfg.input.natural_scroll = boolean(&v);
                true
            }
            ("input.touchpad", "tap-to-click") => {
                cfg.input.tap_to_click = boolean(&v);
                true
            }
            ("input.touchpad", "disable_while_typing") => {
                cfg.input.disable_while_typing = boolean(&v);
                true
            }
            ("", "exec-once") => {
                cfg.exec_once.push(v);
                true
            }
            ("", "env") => match v.split_once(',') {
                Some((n, val)) => {
                    cfg.env.push((n.trim().into(), val.trim().into()));
                    true
                }
                None => false,
            },
            ("", "monitor") => parse_monitor(&v).map(|m| cfg.monitors.push(m)).is_some(),
            ("", "bind") | ("", "binde") | ("", "bindl") | ("", "bindr") => {
                let p: Vec<&str> = v.splitn(4, ',').collect();
                if p.len() >= 3 {
                    dispatcher(p[2], p.get(3).copied().unwrap_or(""))
                        .map(|action| {
                            cfg.binds.push(Bind { mods: mods(p[0]), key: p[1].trim().to_string(), action });
                        })
                        .is_some()
                } else {
                    false
                }
            }
            ("", "bindm") => {
                let p: Vec<&str> = v.splitn(3, ',').collect();
                if p.len() == 3 {
                    let button = match p[1].trim() {
                        "mouse:272" => Some(MouseButton::Left),
                        "mouse:273" => Some(MouseButton::Right),
                        "mouse:274" => Some(MouseButton::Middle),
                        _ => None,
                    };
                    let action = match p[2].trim() {
                        "movewindow" => Some(MouseAction::MoveWindow),
                        "resizewindow" => Some(MouseAction::ResizeWindow),
                        _ => None,
                    };
                    match (button, action) {
                        (Some(button), Some(action)) => {
                            cfg.mouse_binds.push(MouseBind { mods: mods(p[0]), button, action });
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            ("", "windowrulev2") | ("", "windowrule") => parse_rule(&v).map(|r| cfg.rules.push(r)).is_some(),
            _ => false,
        };
        if !ok {
            skipped.push(line.to_string());
        }
    }
    Imported { config: cfg, skipped }
}

fn parse_monitor(v: &str) -> Option<Monitor> {
    // name,1920x1080@60,0x0,1
    let p: Vec<&str> = v.split(',').map(str::trim).collect();
    if p.len() < 4 || p[1] == "disable" {
        return None;
    }
    let (res, refresh) = match p[1].split_once('@') {
        Some((r, hz)) => (r, hz.trim_end_matches("Hz").parse().ok()),
        None => (p[1], None),
    };
    let (w, h) = res.split_once('x')?;
    let (x, y) = p[2].split_once('x')?;
    Some(Monitor {
        name: p[0].to_string(),
        width: w.parse().ok()?,
        height: h.parse().ok()?,
        refresh,
        x: x.parse().ok(),
        y: y.parse().ok(),
        scale: p[3].parse().ok()?,
    })
}

fn parse_rule(v: &str) -> Option<WindowRule> {
    // float, class:^(foo)$      |   workspace 2, class:^(foo)$    |   opacity 0.9, title:bar
    let (what, matcher) = v.split_once(',')?;
    let mut rule = WindowRule::default();
    let (key, pat) = matcher.trim().split_once(':')?;
    let pat = pat.trim().trim_start_matches('^').trim_end_matches('$').trim_start_matches('(').trim_end_matches(')');
    match key.trim() {
        "class" | "initialClass" => rule.app_id = Some(pat.to_string()),
        "title" | "initialTitle" => rule.title = Some(pat.to_string()),
        _ => return None,
    }
    let mut w = what.split_whitespace();
    match (w.next()?, w.next()) {
        ("float", _) => rule.floating = Some(true),
        ("tile", _) => rule.floating = Some(false),
        ("workspace", Some(n)) => rule.workspace = Some(n.parse().ok()?),
        ("opacity", Some(o)) => rule.opacity = Some(o.parse().ok()?),
        _ => return None,
    }
    Some(rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_common_settings() {
        let conf = r#"
$mod = SUPER
monitor = eDP-1,1920x1080@60,0x0,1.25
general {
    gaps_in = 3
    gaps_out = 8
    col.active_border = rgba(33ccffee) rgba(00ff99ee) 45deg
    layout = master
}
decoration {
    rounding = 12
    blur {
        enabled = true
        size = 4
    }
}
input {
    kb_layout = be
    touchpad {
        natural_scroll = yes
    }
}
exec-once = waybar
bind = $mod, Return, exec, kitty
bind = $mod SHIFT, 3, movetoworkspace, 3
bindm = $mod, mouse:272, movewindow
windowrulev2 = float, class:^(pavucontrol)$
unsupported = thing
"#;
        let i = import(conf);
        let c = i.config;
        assert_eq!(c.general.gaps_in, 3);
        assert_eq!(c.general.layout, LayoutKind::Master);
        assert_eq!(c.general.active_border, vec!["#33ccffee", "#00ff99ee"]);
        assert_eq!(c.general.border_angle, 45.0);
        assert_eq!(c.decoration.rounding, 12);
        assert!(c.decoration.blur.enabled);
        assert_eq!(c.input.kb_layout, "be");
        assert!(c.input.natural_scroll);
        assert_eq!(c.exec_once, vec!["waybar"]);
        assert_eq!(c.monitors[0].scale, 1.25);
        assert_eq!(c.binds.len(), 2);
        assert_eq!(c.binds[0].mods, "SUPER");
        assert_eq!(c.mouse_binds.len(), 1);
        assert_eq!(c.rules[0].app_id.as_deref(), Some("pavucontrol"));
        assert_eq!(i.skipped, vec!["unsupported = thing"]);
    }
}
