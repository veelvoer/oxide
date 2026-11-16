//! The Oxide theme: one palette that the shell, the terminal (kitty) and optionally the window borders follow.
//!
//! `write_all` produces files other programs read:
//!   ~/.config/oxide/theme.json        read live by the Quickshell shell
//!   ~/.config/oxide/kitty-theme.conf  included from kitty.conf, running kitties are told to reload
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ThemeCfg {
    /// Name of a preset from `presets()`.
    pub preset: String,
    /// Replaces the preset's accent color ("#rrggbb").
    pub accent: Option<String>,
    /// Make kitty use the theme colors.
    pub kitty: bool,
    /// "dark" or "light".
    pub mode: String,
    /// Corner radius for the shell, settings and windows, 0 to 20.
    pub roundness: u32,
    /// Also recolor GTK and Qt apps (color scheme, accent, window colors).
    pub apps: bool,
    /// "oxide" (flat and calm) or "expressive" (Material 3 Expressive: tonal colors, big round shapes, springy
    /// motion) for the whole desktop: shell, settings, windows, terminal and apps.
    pub style: String,
}

impl Default for ThemeCfg {
    fn default() -> Self {
        Self { preset: "oxide".into(), accent: None, kitty: true, mode: "dark".into(), roundness: 8, apps: true, style: "oxide".into() }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Palette {
    pub name: String,
    pub bg: String,
    pub surface: String,
    pub raised: String,
    pub line: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub accent_soft: String,
    pub ink: String,
    pub good: String,
    pub warn: String,
    pub bad: String,
    pub blue: String,
    pub ansi: Vec<String>,
    /// Gradient for the focused window border and the color of other borders.
    pub border: Vec<String>,
    pub border_inactive: String,
    pub light: bool,
    pub radius: u32,
    /// "oxide" or "expressive"
    pub style: String,
    /// Material roles (also filled for the Oxide presets, derived from their colors)
    pub container: String,
    pub on_container: String,
    pub secondary_container: String,
    pub on_secondary_container: String,
    pub tertiary: String,
    pub surface_low: String,
    pub surface_high: String,
    pub surface_highest: String,
    pub outline: String,
}

fn s(v: &str) -> String {
    v.to_string()
}

fn list(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[allow(clippy::too_many_arguments)]
fn pal(name: &str, c: [&str; 13], ansi: [&str; 16], border: &[&str], border_inactive: &str) -> Palette {
    Palette {
        name: s(name),
        bg: s(c[0]),
        surface: s(c[1]),
        raised: s(c[2]),
        line: s(c[3]),
        text: s(c[4]),
        muted: s(c[5]),
        accent: s(c[6]),
        accent_soft: s(c[7]),
        ink: s(c[8]),
        good: s(c[9]),
        warn: s(c[10]),
        bad: s(c[11]),
        blue: s(c[12]),
        ansi: list(&ansi),
        border: list(border),
        border_inactive: s(border_inactive),
        light: false,
        radius: 8,
        style: s("oxide"),
        container: String::new(),
        on_container: String::new(),
        secondary_container: String::new(),
        on_secondary_container: String::new(),
        tertiary: String::new(),
        surface_low: String::new(),
        surface_high: String::new(),
        surface_highest: String::new(),
        outline: String::new(),
    }
}

pub fn presets() -> Vec<Palette> {
    vec![
        pal(
            "oxide",
            ["#1b1917", "#232120", "#2c2926", "#36322e", "#e9e4de", "#9a928a", "#d4572e", "#e08a63", "#1d0f09", "#5fa391", "#d9a441", "#c8504a", "#6fa8dc"],
            ["#2c2926", "#c8504a", "#5fa391", "#d9a441", "#6f93b8", "#b07a9c", "#5aa5a8", "#cfc8c0", "#4a443e", "#e0675f", "#7bbcab", "#e6b85c", "#8aaed0", "#c795b4", "#78bfc2", "#e9e4de"],
            &["#d4572e", "#e08a63"],
            "#4a443eaa",
        ),
        pal(
            "nord",
            ["#2e3440", "#343b49", "#3b4252", "#4c566a", "#d8dee9", "#9099ad", "#88c0d0", "#8fbcbb", "#2e3440", "#a3be8c", "#ebcb8b", "#bf616a", "#81a1c1"],
            ["#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0", "#e5e9f0", "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#8fbcbb", "#eceff4"],
            &["#88c0d0", "#81a1c1", "#b48ead"],
            "#4c566aaa",
        ),
        pal(
            "gruvbox",
            ["#282828", "#2f2d2c", "#3c3836", "#504945", "#ebdbb2", "#a89984", "#d65d0e", "#fe8019", "#1d2021", "#b8bb26", "#fabd2f", "#fb4934", "#83a598"],
            ["#282828", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#a89984", "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b", "#8ec07c", "#ebdbb2"],
            &["#d65d0e", "#fabd2f"],
            "#504945aa",
        ),
        pal(
            "catppuccin",
            ["#1e1e2e", "#252536", "#313244", "#45475a", "#cdd6f4", "#a6adc8", "#89b4fa", "#b4befe", "#1e1e2e", "#a6e3a1", "#f9e2af", "#f38ba8", "#89b4fa"],
            ["#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de", "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8"],
            &["#89b4fa", "#94e2d5"],
            "#45475aaa",
        ),
        pal(
            "tokyo",
            ["#1a1b26", "#20212e", "#292e42", "#3b4261", "#c0caf5", "#8089b3", "#7aa2f7", "#7dcfff", "#1a1b26", "#9ece6a", "#e0af68", "#f7768e", "#7aa2f7"],
            ["#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5"],
            &["#7aa2f7", "#7dcfff"],
            "#3b4261aa",
        ),
        pal(
            "everforest",
            ["#2d353b", "#343f44", "#3d484d", "#475258", "#d3c6aa", "#9da9a0", "#a7c080", "#83c092", "#2d353b", "#a7c080", "#dbbc7f", "#e67e80", "#7fbbb3"],
            ["#343f44", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa", "#475258", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa"],
            &["#a7c080", "#83c092", "#dbbc7f"],
            "#475258aa",
        ),
        pal(
            "rose-pine",
            ["#191724", "#1f1d2e", "#26233a", "#403d52", "#e0def4", "#908caa", "#ebbcba", "#f6c177", "#191724", "#9ccfd8", "#f6c177", "#eb6f92", "#9ccfd8"],
            ["#26233a", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba", "#e0def4", "#6e6a86", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba", "#e0def4"],
            &["#ebbcba", "#f6c177"],
            "#403d52aa",
        ),
    ]
}


/// Light counterparts of the presets (same names).
pub fn light_presets() -> Vec<Palette> {
    let mut v = vec![
        pal(
            "oxide",
            ["#f4efe9", "#ebe5de", "#e2dbd3", "#d3cbc1", "#2a2420", "#7d7369", "#c24a22", "#d9764f", "#fff7f1", "#3f8a76", "#b8801c", "#b83c36", "#3f78b0"],
            ["#2a2420", "#b83c36", "#3f8a76", "#b8801c", "#3f78b0", "#9a5c88", "#3a8c90", "#cfc6bc", "#5a5048", "#d0504a", "#55a28d", "#cf9a30", "#5a92c8", "#b476a2", "#52a4a8", "#f4efe9"],
            &["#c24a22", "#d9764f"],
            "#d3cbc1aa",
        ),
        pal(
            "nord",
            ["#eceff4", "#e5e9f0", "#d8dee9", "#c8cfdc", "#2e3440", "#5c677d", "#5e81ac", "#81a1c1", "#eceff4", "#6f8f57", "#b8913b", "#bf616a", "#5e81ac"],
            ["#3b4252", "#bf616a", "#6f8f57", "#b8913b", "#5e81ac", "#a07c9a", "#4c9aa5", "#d8dee9", "#4c566a", "#cf7a82", "#8fae75", "#d4ab55", "#81a1c1", "#b48ead", "#6ab3bd", "#eceff4"],
            &["#5e81ac", "#81a1c1"],
            "#c8cfdcaa",
        ),
        pal(
            "gruvbox",
            ["#fbf1c7", "#f2e5bc", "#ebdbb2", "#d5c4a1", "#3c3836", "#7c6f64", "#d65d0e", "#af3a03", "#fbf1c7", "#79740e", "#b57614", "#9d0006", "#076678"],
            ["#fbf1c7", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#7c6f64", "#928374", "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71", "#427b58", "#3c3836"],
            &["#d65d0e", "#b57614"],
            "#d5c4a1aa",
        ),
        pal(
            "catppuccin",
            ["#eff1f5", "#e6e9ef", "#dce0e8", "#ccd0da", "#4c4f69", "#6c6f85", "#1e66f5", "#7287fd", "#eff1f5", "#40a02b", "#df8e1d", "#d20f39", "#1e66f5"],
            ["#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#acb0be", "#6c6f85", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#bcc0cc"],
            &["#1e66f5", "#179299"],
            "#ccd0daaa",
        ),
        pal(
            "tokyo",
            ["#e1e2e7", "#d5d6db", "#c9cad0", "#b6b8c4", "#343b58", "#68709a", "#2e7de9", "#007197", "#e1e2e7", "#587539", "#8c6c3e", "#f52a65", "#2e7de9"],
            ["#b4b5b9", "#f52a65", "#587539", "#8c6c3e", "#2e7de9", "#9854f1", "#007197", "#6172b0", "#a1a6c5", "#ff4774", "#5c8524", "#a27629", "#358aff", "#a463ff", "#007ea8", "#3760bf"],
            &["#2e7de9", "#007197"],
            "#b6b8c4aa",
        ),
        pal(
            "everforest",
            ["#fdf6e3", "#f4f0d9", "#efebd4", "#e0dcc7", "#5c6a72", "#829181", "#8da101", "#35a77c", "#fdf6e3", "#8da101", "#dfa000", "#f85552", "#3a94c5"],
            ["#5c6a72", "#f85552", "#8da101", "#dfa000", "#3a94c5", "#df69ba", "#35a77c", "#dfddc8", "#829181", "#f85552", "#8da101", "#dfa000", "#3a94c5", "#df69ba", "#35a77c", "#fdf6e3"],
            &["#8da101", "#35a77c"],
            "#e0dcc7aa",
        ),
        pal(
            "rose-pine",
            ["#faf4ed", "#fffaf3", "#f2e9e1", "#dfdad9", "#575279", "#797593", "#d7827e", "#ea9d34", "#faf4ed", "#286983", "#ea9d34", "#b4637a", "#56949f"],
            ["#f2e9e1", "#b4637a", "#286983", "#ea9d34", "#56949f", "#907aa9", "#d7827e", "#575279", "#9893a5", "#b4637a", "#286983", "#ea9d34", "#56949f", "#907aa9", "#d7827e", "#575279"],
            &["#d7827e", "#ea9d34"],
            "#dfdad9aa",
        ),
    ];
    for p in &mut v {
        p.light = true;
    }
    v
}

fn parse_hex(h: &str) -> Option<[f32; 3]> {
    let h = h.strip_prefix('#')?;
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok().map(|b| b as f32);
    Some([v(0)?, v(2)?, v(4)?])
}

fn to_hex(c: [f32; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0].round().clamp(0.0, 255.0) as u8, c[1].round().clamp(0.0, 255.0) as u8, c[2].round().clamp(0.0, 255.0) as u8)
}

fn luminance(c: [f32; 3]) -> f32 {
    let f = |v: f32| {
        let v = v / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

/// The palette for a theme setting: the preset with the custom accent applied.
pub fn palette(cfg: &ThemeCfg) -> Palette {
    let all = if cfg.mode == "light" { light_presets() } else { presets() };
    let mut p = if cfg.preset == "auto" {
        // the colors of the wallpaper (until the first one is read, the standard palette)
        match crate::autotheme::load_seed() {
            Some(seed) => crate::autotheme::palette_from_seed(&seed, cfg.mode == "light", &all[0]),
            None => all[0].clone(),
        }
    } else {
        all.iter().find(|p| p.name == cfg.preset).cloned().unwrap_or_else(|| all[0].clone())
    };
    if let Some(acc) = cfg.accent.as_deref().and_then(|a| parse_hex(a).map(|c| (a, c))) {
        let (hex, c) = acc;
        p.accent = hex.to_string();
        // softer sibling: mix toward white
        p.accent_soft = to_hex([c[0] + (255.0 - c[0]) * 0.28, c[1] + (255.0 - c[1]) * 0.28, c[2] + (255.0 - c[2]) * 0.28]);
        // text on the accent: dark on light accents, light on dark ones
        p.ink = if luminance(c) > 0.3 { "#161310".into() } else { "#fbf7f2".into() };
        p.border = vec![p.accent.clone(), p.accent_soft.clone()];
    }
    p.radius = cfg.roundness.min(24);
    if cfg.style == "expressive" {
        expressive(&mut p, cfg);
    } else {
        derive_roles(&mut p);
    }
    p
}

fn mix(a: &str, b: &str, t: f32) -> String {
    match (parse_hex(a), parse_hex(b)) {
        (Some(x), Some(y)) => to_hex([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]),
        _ => a.to_string(),
    }
}

/// The Material roles for an Oxide preset, from its own colors.
fn derive_roles(p: &mut Palette) {
    p.style = "oxide".into();
    p.container = mix(&p.surface, &p.accent, 0.28);
    p.on_container = p.text.clone();
    p.secondary_container = mix(&p.surface, &p.accent_soft, 0.14);
    p.on_secondary_container = p.text.clone();
    p.tertiary = p.blue.clone();
    p.surface_low = mix(&p.bg, &p.surface, 0.5);
    p.surface_high = p.raised.clone();
    p.surface_highest = mix(&p.raised, &p.text, 0.06);
    p.outline = mix(&p.line, &p.muted, 0.5);
}

/// Material Expressive: a tonal scheme from the accent (or the wallpaper's color), big shapes.
fn expressive(p: &mut Palette, cfg: &ThemeCfg) {
    let seed = crate::material::parse(&p.accent).unwrap_or([212, 87, 46]);
    let m = crate::material::scheme(seed, p.light);
    p.style = "expressive".into();
    p.bg = m.surface.clone();
    p.surface = m.surface_container.clone();
    p.raised = m.surface_high.clone();
    p.line = m.outline_variant.clone();
    p.text = m.on_surface.clone();
    p.muted = m.on_surface_variant.clone();
    p.accent = m.primary.clone();
    p.accent_soft = m.tertiary.clone();
    p.ink = m.on_primary.clone();
    p.good = m.good.clone();
    p.warn = m.warn.clone();
    p.bad = m.error.clone();
    p.container = m.primary_container;
    p.on_container = m.on_primary_container;
    p.secondary_container = m.secondary_container;
    p.on_secondary_container = m.on_secondary_container;
    p.tertiary = m.tertiary;
    p.surface_low = m.surface_low;
    p.surface_high = m.surface_high;
    p.surface_highest = m.surface_highest;
    p.outline = m.outline;
    p.border = vec![p.accent.clone(), p.accent_soft.clone()];
    p.border_inactive = format!("{}aa", m.outline_variant);
    // Expressive shapes are big and soft
    p.radius = cfg.roundness.clamp(14, 24);
}

fn config_dir() -> PathBuf {
    crate::Config::path().parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

pub fn theme_json_path() -> PathBuf {
    config_dir().join("theme.json")
}

pub fn kitty_theme_path() -> PathBuf {
    config_dir().join("kitty-theme.conf")
}

pub fn kitty_conf() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("kitty").join("kitty.conf")
}

pub fn kitty_theme(p: &Palette) -> String {
    let mut o = String::from("# Written by Oxide (Settings > Theme). Changes here are overwritten.\n");
    o.push_str(&format!("foreground {}\nbackground {}\n", p.text, p.bg));
    o.push_str(&format!("selection_foreground {}\nselection_background {}\n", p.ink, p.accent));
    o.push_str(&format!("cursor {}\ncursor_text_color {}\nurl_color {}\n", p.accent, p.bg, p.accent_soft));
    o.push_str(&format!("active_border_color {}\ninactive_border_color {}\nbell_border_color {}\n", p.accent, p.line, p.warn));
    o.push_str(&format!(
        "active_tab_foreground {}\nactive_tab_background {}\ninactive_tab_foreground {}\ninactive_tab_background {}\ntab_bar_background {}\n",
        p.ink, p.accent, p.muted, p.raised, p.surface
    ));
    o.push_str(&format!("mark1_foreground {}\nmark1_background {}\n", p.bg, p.accent_soft));
    for (i, c) in p.ansi.iter().enumerate() {
        o.push_str(&format!("color{i} {c}\n"));
    }
    o
}

/// Make sure kitty.conf includes the Oxide theme file (appended last so it wins).
fn ensure_kitty_include() -> std::io::Result<()> {
    let conf = kitty_conf();
    let line = format!("include {}", kitty_theme_path().display());
    let existing = std::fs::read_to_string(&conf).unwrap_or_default();
    if existing.contains(&line) {
        return Ok(());
    }
    if let Some(d) = conf.parent() {
        std::fs::create_dir_all(d)?;
    }
    let sep = if existing.is_empty() || existing.ends_with('\n') { "" } else { "\n" };
    std::fs::write(&conf, format!("{existing}{sep}\n# Oxide theme (Settings > Theme)\n{line}\n"))
}

/// Remove the include again (when "terminal follows theme" is turned off).
fn remove_kitty_include() {
    let conf = kitty_conf();
    let Ok(existing) = std::fs::read_to_string(&conf) else { return };
    let line = format!("include {}", kitty_theme_path().display());
    if !existing.contains(&line) {
        return;
    }
    let cleaned: Vec<&str> = existing.lines().filter(|l| *l != line && *l != "# Oxide theme (Settings > Theme)").collect();
    let _ = std::fs::write(&conf, cleaned.join("\n") + "\n");
}

const GTK_BEGIN: &str = "/* oxide theme begin */";
const GTK_END: &str = "/* oxide theme end */";

fn home_config() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Replace (or remove) our marked block in a user stylesheet, leaving the rest alone.
fn set_block(path: &std::path::Path, block: Option<&str>) {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut kept = existing.clone();
    if let (Some(a), Some(b)) = (existing.find(GTK_BEGIN), existing.find(GTK_END)) {
        if b > a {
            kept = format!("{}{}", &existing[..a], existing[b + GTK_END.len()..].trim_start_matches('\n'));
        }
    }
    let kept = kept.trim_end().to_string();
    let out = match block {
        Some(b) => format!("{kept}{}{GTK_BEGIN}\n{b}{GTK_END}\n", if kept.is_empty() { "" } else { "\n\n" }),
        None if kept.is_empty() => String::new(),
        None => format!("{kept}\n"),
    };
    if out == existing {
        return;
    }
    if out.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(path, out);
}

fn gtk_css(p: &Palette) -> String {
    let c = |n: &str, v: &str| format!("@define-color {n} {v};\n");
    let mut o = String::new();
    for (n, v) in [
        ("accent_color", &p.accent), ("accent_bg_color", &p.accent), ("accent_fg_color", &p.ink),
        ("window_bg_color", &p.bg), ("window_fg_color", &p.text),
        ("view_bg_color", &p.surface), ("view_fg_color", &p.text),
        ("headerbar_bg_color", &p.surface), ("headerbar_fg_color", &p.text), ("headerbar_border_color", &p.line),
        ("card_bg_color", &p.surface), ("card_fg_color", &p.text),
        ("popover_bg_color", &p.raised), ("popover_fg_color", &p.text),
        ("sidebar_bg_color", &p.surface), ("sidebar_fg_color", &p.text),
        ("destructive_bg_color", &p.bad), ("success_bg_color", &p.good), ("warning_bg_color", &p.warn),
    ] {
        o.push_str(&c(n, v));
    }
    o
}

/// Run a helper without ever waiting on it for long (it may need services that are not up yet).
fn run_quiet(cmd: &str, args: &[&str]) -> bool {
    std::process::Command::new("timeout")
        .arg("4")
        .arg(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Set `key=value` inside the [Settings] section of a GTK settings.ini, keeping everything else.
fn set_ini(path: &std::path::Path, key: &str, value: &str, only_if_stock: bool) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if !lines.iter().any(|l| l.trim() == "[Settings]") {
        lines.insert(0, "[Settings]".into());
    }
    let sect = lines.iter().position(|l| l.trim() == "[Settings]").unwrap_or(0);
    let existing = lines.iter().position(|l| l.trim_start().starts_with(&format!("{key}=")) || l.trim_start().starts_with(&format!("{key} =")));
    match existing {
        Some(i) => {
            let cur = lines[i].split_once('=').map(|(_, v)| v.trim().to_string()).unwrap_or_default();
            if only_if_stock && !(cur.is_empty() || cur.starts_with("Adwaita") || cur.starts_with("adw-gtk3") || cur == "Default") {
                return;
            }
            lines[i] = format!("{key}={value}");
        }
        None => lines.insert(sect + 1, format!("{key}={value}")),
    }
    let out = lines.join("\n") + "\n";
    if out != text {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(path, out);
    }
}

/// Make other toolkits follow: GTK/libadwaita (color scheme + accent + window colors), KDE/Qt (Breeze scheme).
fn apply_apps(cfg: &ThemeCfg, p: &Palette) {
    let scheme = if p.light { "prefer-light" } else { "prefer-dark" };
    if cfg.apps {
        run_quiet("gsettings", &["set", "org.gnome.desktop.interface", "color-scheme", scheme]);
        // Oxide draws window frames itself: no minimize/maximize/close buttons in GTK header bars
        run_quiet("gsettings", &["set", "org.gnome.desktop.wm.preferences", "button-layout", ":"]);
        // GTK3 apps pick the variant from the theme name; only touch the stock ones
        let cur = std::process::Command::new("timeout")
            .args(["4", "gsettings"])
            .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
            .unwrap_or_default();
        if matches!(cur.as_str(), "" | "Adwaita" | "Adwaita-dark" | "Default" | "adw-gtk3" | "adw-gtk3-dark") {
            let name = if cur.starts_with("adw-gtk3") { if p.light { "adw-gtk3" } else { "adw-gtk3-dark" } } else if p.light { "Adwaita" } else { "Adwaita-dark" };
            run_quiet("gsettings", &["set", "org.gnome.desktop.interface", "gtk-theme", name]);
        }
        // apps that read GTK's own settings file (GTK 3, Flutter apps like LocalSend, many Electron apps)
        for v in ["gtk-3.0", "gtk-4.0"] {
            let ini = home_config().join(v).join("settings.ini");
            set_ini(&ini, "gtk-application-prefer-dark-theme", if p.light { "0" } else { "1" }, false);
            set_ini(&ini, "gtk-theme-name", if p.light { "Adwaita" } else { "Adwaita-dark" }, true);
        }
        let css = gtk_css(p);
        for v in ["gtk-3.0", "gtk-4.0"] {
            set_block(&home_config().join(v).join("gtk.css"), Some(&css));
        }
        // KDE apps read their scheme from kdeglobals when they start
        let scheme = if p.light { "BreezeLight" } else { "BreezeDark" };
        for tool in ["kwriteconfig6", "kwriteconfig5"] {
            if run_quiet(tool, &["--file", "kdeglobals", "--group", "General", "--key", "ColorScheme", scheme]) {
                break;
            }
        }
    } else {
        for v in ["gtk-3.0", "gtk-4.0"] {
            set_block(&home_config().join(v).join("gtk.css"), None);
        }
    }
}

/// Write theme.json (for the shell), the kitty theme and the app colors; tell running kitties to reload.
pub fn write_all(cfg: &ThemeCfg) -> std::io::Result<()> {
    let p = palette(cfg);
    std::fs::create_dir_all(config_dir())?;
    let json = serde_json::to_string_pretty(&p).map_err(std::io::Error::other)?;
    std::fs::write(theme_json_path(), json)?;
    // the standard terminal config is generated by Oxide and already includes the theme; the user's own
    // kitty.conf (Advanced > Own configs) gets one include line
    let full = crate::Config::load().unwrap_or_default();
    let full_own_gtk = full.own.gtk;
    if cfg.kitty {
        std::fs::write(kitty_theme_path(), kitty_theme(&p))?;
        if full.own.kitty {
            ensure_kitty_include()?;
        }
    } else if full.own.kitty {
        remove_kitty_include();
    }
    if !full.own.kitty {
        remove_kitty_include();
    }
    let mut full = full;
    full.theme = cfg.clone();
    crate::defaults::ensure(&full);
    // kitty reloads its config on SIGUSR1
    let testing = std::env::var_os("OXIDE_TEST").is_some();
    if !testing {
        let _ = std::process::Command::new("pkill").args(["-USR1", "-x", "kitty"]).status();
    }
    if !full_own_gtk && !testing {
        apply_apps(cfg, &p);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_presets_match_dark() {
        let (d, l) = (presets(), light_presets());
        assert_eq!(d.len(), l.len());
        for (a, b) in d.iter().zip(l.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(b.ansi.len(), 16);
            assert!(b.light);
        }
        let p = palette(&ThemeCfg { mode: "light".into(), roundness: 3, ..Default::default() });
        assert!(p.light && p.radius == 3);
    }

    #[test]
    fn presets_are_complete() {
        for p in presets() {
            assert_eq!(p.ansi.len(), 16, "{}", p.name);
            for c in [&p.bg, &p.text, &p.accent, &p.line] {
                assert!(parse_hex(c).is_some(), "{} {c}", p.name);
            }
            assert!(!p.border.is_empty());
        }
    }

    #[test]
    fn custom_accent_picks_readable_ink() {
        let light = palette(&ThemeCfg { preset: "oxide".into(), accent: Some("#f0d060".into()), kitty: true, ..Default::default() });
        assert_eq!(light.ink, "#161310");
        let dark = palette(&ThemeCfg { preset: "oxide".into(), accent: Some("#304090".into()), kitty: true, ..Default::default() });
        assert_eq!(dark.ink, "#fbf7f2");
        assert_eq!(dark.accent, "#304090");
    }

    #[test]
    fn kitty_theme_has_colors() {
        let t = kitty_theme(&palette(&ThemeCfg::default()));
        assert!(t.contains("background #1b1917") && t.contains("color15 #e9e4de"));
    }
}
