//! Oxide's standard terminal setup. Out of the box kitty and fish start with Oxide's own configuration (made from
//! Settings > Terminal and the theme); in Advanced mode each of them can be switched to the user's own files.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{theme, Config};

/// Which parts of the system use the user's own configuration instead of Oxide's standard one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct OwnCfg {
    /// A custom Quickshell shell (otherwise the standard Oxide shell).
    pub shell: bool,
    /// ~/.config/kitty/kitty.conf
    pub kitty: bool,
    /// ~/.config/fish/config.fish (inside kitty)
    pub fish: bool,
    /// GTK settings and style sheets (otherwise Oxide colors them with the theme).
    pub gtk: bool,
}

/// Settings of the standard terminal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TerminalCfg {
    pub font: String,
    pub font_size: f32,
    pub opacity: f32,
    pub padding: u32,
    /// "block", "beam" or "underline"
    pub cursor: String,
    pub scrollback: u32,
    /// Show fish's welcome text when a terminal opens.
    pub greeting: bool,
}

impl Default for TerminalCfg {
    fn default() -> Self {
        Self { font: "monospace".into(), font_size: 12.0, opacity: 0.95, padding: 10, cursor: "beam".into(), scrollback: 10_000, greeting: false }
    }
}

fn config_dir() -> PathBuf {
    Config::path().parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

pub fn kitty_dir() -> PathBuf {
    config_dir().join("defaults/kitty")
}

pub fn kitty_file() -> PathBuf {
    kitty_dir().join("kitty.conf")
}

pub fn fish_file() -> PathBuf {
    config_dir().join("defaults/config.fish")
}

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

pub fn kitty_default(cfg: &Config) -> String {
    let t = &cfg.terminal;
    let mut o = String::from("# Oxide's standard kitty configuration. Made by Oxide: change it in Settings > Terminal.\n# To use your own kitty.conf, turn on Advanced mode > Own configs.\n");
    o.push_str(&format!("font_family {}\nfont_size {:.1}\nbackground_opacity {:.2}\nwindow_padding_width {}\ncursor_shape {}\nscrollback_lines {}\n", t.font, t.font_size, t.opacity.clamp(0.1, 1.0), t.padding, t.cursor, t.scrollback));
    o.push_str("confirm_os_window_close 0\nenable_audio_bell no\ncopy_on_select clipboard\nremember_window_size no\ninitial_window_width 110c\ninitial_window_height 32c\n");
    if on_path("fish") && !cfg.own.fish {
        o.push_str(&format!("shell fish --no-config --init-command \"source {}\"\n", fish_file().display()));
    }
    if cfg.theme.kitty {
        o.push_str(&format!("include {}\n", theme::kitty_theme_path().display()));
    }
    o
}

pub fn fish_default(cfg: &Config) -> String {
    let p = theme::palette(&cfg.theme);
    let c = |h: &str| h.trim_start_matches('#').to_string();
    let mut o = String::from("# Oxide's standard fish configuration. Made by Oxide (Settings > Terminal). Your own config.fish is used\n# when you turn on Advanced mode > Own configs.\n");
    if !cfg.terminal.greeting {
        o.push_str("set -g fish_greeting\n");
    }
    for (k, v) in [
        ("normal", &p.text), ("command", &p.blue), ("keyword", &p.accent), ("quote", &p.good), ("redirection", &p.accent_soft), ("end", &p.accent),
        ("error", &p.bad), ("param", &p.text), ("comment", &p.muted), ("operator", &p.accent_soft), ("escape", &p.warn), ("autosuggestion", &p.muted),
        ("cwd", &p.blue), ("user", &p.good), ("host", &p.accent_soft), ("valid_path", &p.text),
    ] {
        o.push_str(&format!("set -g fish_color_{k} {}\n", c(v)));
    }
    o.push_str(&format!("set -g fish_color_selection --background={}\nset -g fish_pager_color_selected_background --background={}\n", c(&p.raised), c(&p.raised)));
    o.push_str("\nalias ll 'ls -la'\nalias .. 'cd ..'\nalias ... 'cd ../..'\n");
    o
}

/// (Re)write the standard files and point kitty at them unless the user uses their own.
pub fn ensure(cfg: &Config) {
    let _ = std::fs::create_dir_all(kitty_dir());
    let _ = std::fs::write(kitty_file(), kitty_default(cfg));
    let _ = std::fs::write(fish_file(), fish_default(cfg));
    apply_env(cfg);
}

/// New kitty windows (started from the shell, a key bind or the launcher) read the right configuration.
pub fn apply_env(cfg: &Config) {
    if cfg.own.kitty {
        std::env::remove_var("KITTY_CONFIG_DIRECTORY");
    } else {
        std::env::set_var("KITTY_CONFIG_DIRECTORY", kitty_dir());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_files_follow_the_settings() {
        let mut c = Config::default();
        c.terminal.font_size = 15.0;
        c.terminal.greeting = false;
        let k = kitty_default(&c);
        assert!(k.contains("font_size 15.0") && k.contains("kitty-theme.conf"));
        let f = fish_default(&c);
        assert!(f.contains("set -g fish_greeting\n") && f.contains("fish_color_command"));
        c.terminal.greeting = true;
        assert!(!fish_default(&c).contains("set -g fish_greeting\n"));
    }

    #[test]
    fn own_config_is_off_by_default() {
        let c = Config::default();
        assert!(!c.own.shell && !c.own.kitty && !c.own.fish && !c.own.gtk);
    }
}
