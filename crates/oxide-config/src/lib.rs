//! Shared configuration schema for the compositor and the settings GUI.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod ai;
pub mod anim;
pub mod autotheme;
pub mod defaults;
pub mod hyprland;
pub mod material;
pub mod theme;

/// Workspace id used for the special (scratchpad) workspace.
pub const SPECIAL_WS: u32 = 100;
/// Workspaces of macro scratchpads: GROUP_WS + the scratchpad's index (never listed as normal workspaces).
pub const GROUP_WS: u32 = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub decoration: Decoration,
    pub animations: Animations,
    pub input: Input,
    pub layout: LayoutCfg,
    #[serde(rename = "monitor")]
    pub monitors: Vec<Monitor>,
    #[serde(rename = "bind")]
    pub binds: Vec<Bind>,
    pub shell: ShellCfg,
    pub theme: theme::ThemeCfg,
    /// Parts that use the user's own configuration (Advanced mode); everything else is Oxide's standard.
    pub own: defaults::OwnCfg,
    pub terminal: defaults::TerminalCfg,
    #[serde(rename = "mouse_bind")]
    pub mouse_binds: Vec<MouseBind>,
    #[serde(rename = "rule")]
    pub rules: Vec<WindowRule>,
    /// Macro scratchpads: groups of apps summoned and dismissed together with one key.
    #[serde(rename = "scratchpad")]
    pub scratchpads: Vec<Scratchpad>,
    /// The phone app (Oxide Remote): run oxide-remoted so paired phones can connect.
    pub remote: RemoteCfg,
    /// Updates from the git repository (oxide-ctl update).
    pub update: UpdateCfg,
    pub exec_once: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// The Quickshell bar/shell Oxide starts and supervises.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ShellCfg {
    pub enabled: bool,
    /// Name of a folder in `shells_dir()` that contains `shell.qml`.
    pub active: String,
}

impl Default for ShellCfg {
    fn default() -> Self {
        Self { enabled: true, active: "oxide".into() }
    }
}

/// Folder holding all installed shells: `~/.config/oxide/shells`.
pub fn shells_dir() -> PathBuf {
    Config::path().with_file_name("shells")
}

/// Symlink that always points at the running shell: `~/.config/oxide/active-shell`.
/// Folders that may hold wallpapers, most personal first (the bundled photos live in the last ones).
pub fn wallpaper_dirs() -> Vec<std::path::PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))).unwrap_or_default();
    vec![data.join("oxide/wallpapers"), "/usr/local/share/oxide/wallpapers".into(), "/usr/share/oxide/wallpapers".into()]
}

/// Names (file stems) of the photos you can pick, without duplicates.
pub fn wallpaper_names() -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for d in wallpaper_dirs() {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png")) {
                if let Some(n) = p.file_stem().and_then(|x| x.to_str()) {
                    if !v.iter().any(|x| x == n) {
                        v.push(n.to_string());
                    }
                }
            }
        }
    }
    v.sort();
    v
}

pub fn active_shell_link() -> PathBuf {
    Config::path().with_file_name("active-shell")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct General {
    pub gaps_in: i32,
    pub gaps_out: i32,
    pub border_size: i32,
    /// One color, or several for a gradient ("#rrggbb" or "#rrggbbaa").
    #[serde(deserialize_with = "one_or_many")]
    pub active_border: Vec<String>,
    /// Gradient direction in degrees.
    pub border_angle: f32,
    pub inactive_border: String,
    pub layout: LayoutKind,
    pub focus_follows_mouse: bool,
    pub cursor_theme: String,
    pub cursor_size: u32,
    /// Fraction used for the default floating window size.
    pub float_size: f32,
    /// Start XWayland for X11 applications.
    pub xwayland: bool,
    /// Super + mouse wheel switches workspaces.
    pub scroll_workspaces: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutKind {
    #[default]
    Dwindle,
    Master,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LayoutCfg {
    /// Master layout: fraction of the screen the master area takes.
    pub master_factor: f32,
    /// Dwindle: split ratio for new windows (0.1..0.9).
    pub split_ratio: f32,
    /// A window that needs you (a build failed in it, it is in a call) grows without you resizing it.
    pub context_aware: bool,
    /// How much of its share such a window takes (0.5..0.85).
    pub focus_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Decoration {
    pub rounding: i32,
    pub active_opacity: f32,
    pub inactive_opacity: f32,
    pub blur: Blur,
    pub shadow: Shadow,
    pub dim_inactive: f32,
    /// Borders that tell you something: a red pulse when a build fails, a sweep while it runs, a green flash when it
    /// succeeds, a slow breath during a call, and a warmer focus border when the CPU is busy.
    pub telemetry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Blur {
    pub enabled: bool,
    pub size: u32,
    pub passes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Shadow {
    pub enabled: bool,
    pub range: u32,
    pub color: String,
}

/// A named easing: either a cubic bezier or a damped spring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Curve {
    pub name: String,
    /// "bezier" or "spring"
    pub kind: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub mass: f32,
    pub stiffness: f32,
    pub dampening: f32,
}

impl Default for Curve {
    fn default() -> Self {
        Self { name: String::new(), kind: "bezier".into(), x1: 0.25, y1: 1.0, x2: 0.5, y2: 1.0, mass: 0.7, stiffness: 130.0, dampening: 16.0 }
    }
}

impl Curve {
    pub fn is_spring(&self) -> bool {
        self.kind == "spring"
    }
    pub fn bezier(name: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { name: name.into(), kind: "bezier".into(), x1, y1, x2, y2, ..Self::default() }
    }
    pub fn spring(name: &str, mass: f32, stiffness: f32, dampening: f32) -> Self {
        Self { name: name.into(), kind: "spring".into(), mass, stiffness, dampening, ..Self::default() }
    }
}

/// One animated thing (like Hyprland's animation "leaf").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Anim {
    pub enabled: bool,
    /// Bezier curves: duration is speed x 100 ms. Springs ignore it (physics decides).
    pub speed: f32,
    /// Name of one of `Animations::curves`.
    pub curve: String,
    /// "none", "slide", "popin 90%" or "gnomed" (windows and workspaces).
    pub style: String,
}

impl Default for Anim {
    fn default() -> Self {
        Self { enabled: true, speed: 4.0, curve: "ease".into(), style: String::new() }
    }
}

impl Anim {
    pub fn new(speed: f32, curve: &str, style: &str) -> Self {
        Self { enabled: true, speed, curve: curve.into(), style: style.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Animations {
    pub enabled: bool,
    pub curves: Vec<Curve>,
    /// Windows moving and resizing (layout changes).
    pub windows: Anim,
    pub windows_in: Anim,
    pub windows_out: Anim,
    pub fade_in: Anim,
    pub fade_out: Anim,
    pub workspaces: Anim,
    /// Border color when focus changes.
    pub border: Anim,
}

impl Animations {
    /// Look a curve up by name; unknown names fall back to a gentle ease.
    pub fn curve(&self, name: &str) -> Curve {
        self.curves
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .unwrap_or_else(|| Curve::bezier("ease", 0.25, 1.0, 0.5, 1.0))
    }
}

pub fn default_curves() -> Vec<Curve> {
    vec![
        Curve::bezier("ease", 0.25, 1.0, 0.5, 1.0),
        Curve::bezier("snap", 0.15, 0.0, 0.1, 1.0),
        Curve::bezier("soft", 0.4, 0.0, 0.2, 1.0),
        Curve::bezier("linear", 0.0, 0.0, 1.0, 1.0),
        Curve::spring("spring", 0.7, 130.0, 16.0),
        Curve::spring("bounce", 0.6, 170.0, 9.0),
        // Material 3 Expressive motion (mass 1; damping = 2 * ratio * sqrt(stiffness))
        Curve::spring("m3-spatial", 1.0, 380.0, 31.2),
        Curve::spring("m3-fast", 1.0, 800.0, 33.9),
        Curve::spring("m3-effects", 1.0, 1600.0, 80.0),
    ]
}

impl Animations {
    /// Material 3 Expressive: springy moves and openings, calm fades.
    pub fn expressive() -> Self {
        Self {
            windows: Anim::new(4.0, "m3-spatial", ""),
            windows_in: Anim::new(4.0, "m3-spatial", "popin 86%"),
            windows_out: Anim::new(2.6, "m3-effects", "popin 92%"),
            workspaces: Anim::new(4.0, "m3-spatial", "slide"),
            fade_in: Anim::new(3.0, "m3-effects", ""),
            fade_out: Anim::new(2.1, "m3-effects", ""),
            border: Anim::new(4.0, "m3-effects", ""),
            ..Self::default()
        }
    }
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            enabled: true,
            curves: default_curves(),
            windows: Anim::new(4.0, "spring", ""),
            windows_in: Anim::new(3.6, "spring", "gnomed"),
            windows_out: Anim::new(2.2, "spring", "gnomed"),
            fade_in: Anim::new(3.0, "ease", ""),
            fade_out: Anim::new(2.1, "ease", ""),
            workspaces: Anim::new(4.5, "spring", "slide"),
            border: Anim::new(4.0, "ease", ""),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Input {
    pub kb_layout: String,
    pub kb_variant: String,
    pub kb_options: String,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
    pub natural_scroll: bool,
    pub tap_to_click: bool,
    pub disable_while_typing: bool,
    pub sensitivity: f64,
    /// "adaptive" or "flat"
    pub accel_profile: String,
    pub scroll_factor: f64,
    pub numlock_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Monitor {
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub refresh: Option<f64>,
    /// None places the screen automatically, to the right of the others.
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bind {
    /// e.g. "SUPER+SHIFT", "ALT"
    pub mods: String,
    /// xkb keysym name, e.g. "Return", "q", "1"
    pub key: String,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "arg", rename_all = "snake_case")]
pub enum Action {
    Exec(String),
    Close,
    ToggleFloating,
    ToggleFullscreen,
    FocusDir(Dir),
    MoveDir(Dir),
    Workspace(u32),
    MoveToWorkspace(u32),
    /// Go to the next (+1) or previous (-1) workspace, wrapping around.
    WorkspaceRel(i32),
    /// Next (+1) or previous (-1) workspace that has windows (or is shown), like Hyprland's `e+1`.
    WorkspaceExisting(i32),
    MoveToWorkspaceRel(i32),
    /// Show or hide the special (scratchpad) workspace.
    ToggleSpecial,
    MoveToSpecial,
    FocusMonitor(Dir),
    MoveToMonitor(Dir),
    CycleLayout,
    GrowMaster,
    ShrinkMaster,
    ReloadConfig,
    /// Press another key for the focused app, e.g. "alt+Left" or "ctrl+shift+t".
    SendKey(String),
    /// Show or hide a macro scratchpad (by name).
    ToggleScratchpad(String),
    /// Zoom out to the overview of all workspaces (and back).
    ToggleOverview,
    Quit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MouseBind {
    pub mods: String,
    pub button: MouseButton,
    pub action: MouseAction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
    MoveWindow,
    ResizeWindow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RemoteCfg {
    pub enabled: bool,
    /// Also reachable away from home: the router is asked to forward the port (UPnP) and paired phones learn
    /// the PC's address through an encrypted note on `rendezvous`.
    pub anywhere: bool,
    /// A ntfy server (https://ntfy.sh or your own) used to tell phones where the PC is; only encrypted data goes there.
    pub rendezvous: String,
    /// Leave an encrypted note with this PC's current addresses, so paired phones find it on any network (also
    /// when the network blocks the search). Only the phones can read it.
    pub notes: bool,
    /// The phone's screen on the PC (oxide-ctl phone, Super+P): longest side in pixels, video bit rate in Mbit/s,
    /// and whether its sound plays on the PC too.
    pub phone_max_size: u32,
    pub phone_bitrate: u32,
    pub phone_audio: bool,
}

impl Default for RemoteCfg {
    fn default() -> Self {
        Self { enabled: false, anywhere: false, rendezvous: "https://ntfy.sh".into(), notes: true, phone_max_size: 1600, phone_bitrate: 12, phone_audio: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UpdateCfg {
    /// Look for a new version now and then, build and install it; what changed is shown at the next login.
    pub auto: bool,
    pub repo: String,
    pub branch: String,
}

impl Default for UpdateCfg {
    fn default() -> Self {
        Self { auto: true, repo: "https://github.com/veelvoer/oxide".into(), branch: "main".into() }
    }
}

/// A group of windows that appears and disappears as one, arranged side by side over the current workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Scratchpad {
    pub name: String,
    pub apps: Vec<ScratchApp>,
    /// Size of the group as a fraction of the screen.
    pub width: f32,
    pub height: f32,
}

impl Default for Scratchpad {
    fn default() -> Self {
        Self { name: String::new(), apps: vec![], width: 0.84, height: 0.72 }
    }
}

/// One member of a scratchpad: the window to take (by app id) and how to start it when it is not open.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ScratchApp {
    pub app_id: String,
    pub exec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct WindowRule {
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub floating: Option<bool>,
    pub workspace: Option<u32>,
    pub opacity: Option<f32>,
    /// Force background blur on (or off) for matching windows.
    pub blur: Option<bool>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            gaps_in: 7,
            gaps_out: 3,
            border_size: 2,
            active_border: vec!["#88c0d0".into(), "#81a1c1".into(), "#b48ead".into()],
            border_angle: 45.0,
            inactive_border: "#4c566aaa".into(),
            layout: LayoutKind::Dwindle,
            focus_follows_mouse: true,
            cursor_theme: "default".into(),
            cursor_size: 24,
            float_size: 0.6,
            xwayland: true,
            scroll_workspaces: true,
        }
    }
}
fn one_or_many<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        One(String),
        Many(Vec<String>),
    }
    Ok(match V::deserialize(d)? {
        V::One(s) => vec![s],
        V::Many(v) => v,
    })
}

impl Default for LayoutCfg {
    fn default() -> Self {
        Self { master_factor: 0.55, split_ratio: 0.5, context_aware: true, focus_weight: 0.62 }
    }
}
impl Default for Decoration {
    fn default() -> Self {
        Self {
            rounding: 12,
            active_opacity: 1.0,
            inactive_opacity: 0.94,
            blur: Blur::default(),
            shadow: Shadow::default(),
            dim_inactive: 0.0,
            telemetry: true,
        }
    }
}
impl Default for Blur {
    fn default() -> Self {
        Self { enabled: true, size: 14, passes: 3 }
    }
}
impl Default for Shadow {
    fn default() -> Self {
        Self { enabled: true, range: 12, color: "#171a20aa".into() }
    }
}
/// The keyboard layout (and variant) the system is set up with, so a new install types what the keys say:
/// `$XKB_DEFAULT_LAYOUT`, /etc/default/keyboard, an X11 keyboard snippet, the console keymap, else "us".
pub fn detect_layout() -> (String, String) {
    let clean = |v: &str| v.trim().trim_matches('"').split(',').next().unwrap_or("").trim().to_string();
    if let Ok(l) = std::env::var("XKB_DEFAULT_LAYOUT") {
        if !clean(&l).is_empty() {
            return (clean(&l), std::env::var("XKB_DEFAULT_VARIANT").map(|v| clean(&v)).unwrap_or_default());
        }
    }
    let from_kv = |text: &str, lk: &str, vk: &str| -> Option<(String, String)> {
        let find = |k: &str| text.lines().filter_map(|l| l.trim().strip_prefix(k)).map(|r| r.trim_start_matches(['=', ' ']).to_string()).next();
        let l = clean(&find(lk)?);
        (!l.is_empty()).then(|| (l, find(vk).map(|v| clean(&v)).unwrap_or_default()))
    };
    if let Some(r) = std::fs::read_to_string("/etc/default/keyboard").ok().and_then(|t| from_kv(&t, "XKBLAYOUT", "XKBVARIANT")) {
        return r;
    }
    // Option "XkbLayout" "be"
    if let Ok(t) = std::fs::read_to_string("/etc/X11/xorg.conf.d/00-keyboard.conf") {
        let opt = |k: &str| t.lines().find(|l| l.contains(k)).and_then(|l| l.split('"').nth(3)).map(|v| clean(v));
        if let Some(l) = opt("XkbLayout").filter(|l| !l.is_empty()) {
            return (l, opt("XkbVariant").unwrap_or_default());
        }
    }
    if let Ok(t) = std::fs::read_to_string("/etc/vconsole.conf") {
        if let Some(k) = t.lines().find_map(|l| l.strip_prefix("KEYMAP=")) {
            // "be-latin1" -> "be", "us" -> "us", "de-nodeadkeys" -> "de"
            let l = clean(k).split(['-', '_']).next().unwrap_or("").to_string();
            if l.len() == 2 {
                return (l, String::new());
            }
        }
    }
    ("us".into(), String::new())
}

impl Default for Input {
    fn default() -> Self {
        Self {
            kb_layout: detect_layout().0,
            kb_variant: detect_layout().1,
            kb_options: String::new(),
            repeat_rate: 25,
            repeat_delay: 600,
            natural_scroll: true,
            tap_to_click: true,
            disable_while_typing: true,
            sensitivity: 0.0,
            accel_profile: "flat".into(),
            scroll_factor: 1.0,
            numlock_on_start: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        use Action::*;
        let b = |mods: &str, key: &str, action| Bind { mods: mods.into(), key: key.into(), action };
        // Defaults mirror the author's Hyprland setup: IJKL navigation (I up, J left, K down, L right).
        let mut binds = vec![
            b("SUPER", "q", Exec("oxide-ctl terminal".into())),
            b("SUPER", "c", Close),
            b("SUPER", "m", Quit),
            b("SUPER", "e", Exec("oxide-ctl files".into())),
            b("SUPER", "v", ToggleFloating),
            b("SUPER", "f", ToggleFullscreen),
            b("SUPER", "r", Exec("oxide-ctl launcher".into())),
            b("ALT", "space", Exec("oxide-ctl launcher".into())),
            b("SUPER", "comma", Exec("oxide-settings".into())),
            b("SUPER", "a", Exec("oxide-ctl ipc ai toggle".into())),
            b("SUPER", "z", Exec("oxide-ctl ipc shell toggleControl".into())),
            b("SUPER", "o", Exec("oxide-ctl ipc shell toggleWindow".into())),
            b("SUPER", "d", Exec("oxide-ctl ipc shell toggleLauncher".into())),
            // the phone's screen in a window (scrcpy)
            b("SUPER", "p", Exec("oxide-ctl phone".into())),
            // Alt + J K I L act as the arrow keys everywhere (add Shift or Ctrl to select or jump words)
            b("ALT", "j", SendKey("Left".into())),
            b("ALT", "k", SendKey("Down".into())),
            b("ALT", "i", SendKey("Up".into())),
            b("ALT", "l", SendKey("Right".into())),
            b("ALT+SHIFT", "j", SendKey("shift+Left".into())),
            b("ALT+SHIFT", "k", SendKey("shift+Down".into())),
            b("ALT+SHIFT", "i", SendKey("shift+Up".into())),
            b("ALT+SHIFT", "l", SendKey("shift+Right".into())),
            b("ALT+CTRL", "j", SendKey("ctrl+Left".into())),
            b("ALT+CTRL", "l", SendKey("ctrl+Right".into())),
            b("SUPER+SHIFT", "v", Exec("oxide-ctl ipc shell toggleClipboard".into())),
            b("SUPER+SHIFT", "x", Exec("oxide-ctl lock".into())),
            b("SUPER", "n", Exec("oxide-ctl lock".into())),
            b("", "Print", Exec("grim -g \"$(slurp)\" - | wl-copy".into())),
            // focus: arrows and IJKL
            b("SUPER", "Left", FocusDir(Dir::Left)),
            b("SUPER", "Right", FocusDir(Dir::Right)),
            b("SUPER", "Up", FocusDir(Dir::Up)),
            b("SUPER", "Down", FocusDir(Dir::Down)),
            b("SUPER", "j", FocusDir(Dir::Left)),
            b("SUPER", "l", FocusDir(Dir::Right)),
            b("SUPER", "i", FocusDir(Dir::Up)),
            b("SUPER", "k", FocusDir(Dir::Down)),
            // move windows
            b("SUPER+CTRL", "j", MoveDir(Dir::Left)),
            b("SUPER+CTRL", "l", MoveDir(Dir::Right)),
            b("SUPER+CTRL", "i", MoveDir(Dir::Up)),
            b("SUPER+CTRL", "k", MoveDir(Dir::Down)),
            // workspaces
            b("SUPER+SHIFT", "l", WorkspaceRel(1)),
            b("SUPER+SHIFT", "j", WorkspaceRel(-1)),
            b("SUPER+SHIFT+ALT", "l", MoveToWorkspaceRel(1)),
            b("SUPER+SHIFT+ALT", "j", MoveToWorkspaceRel(-1)),
            b("ALT", "Tab", ToggleSpecial),
            b("SUPER+SHIFT", "s", MoveToSpecial),
            // layout and monitors (Oxide specific)
            b("SUPER", "space", CycleLayout),
            b("SUPER", "Tab", ToggleOverview),
            b("SUPER", "minus", ShrinkMaster),
            b("SUPER", "equal", GrowMaster),
            b("SUPER+SHIFT", "r", ReloadConfig),
            b("SUPER+SHIFT", "Left", FocusMonitor(Dir::Left)),
            b("SUPER+SHIFT", "Right", FocusMonitor(Dir::Right)),
            b("SUPER+CTRL", "Left", MoveToMonitor(Dir::Left)),
            b("SUPER+CTRL", "Right", MoveToMonitor(Dir::Right)),
            // media and hardware keys
            b("", "XF86AudioRaiseVolume", Exec("wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+ || pactl set-sink-volume @DEFAULT_SINK@ +5%".into())),
            b("", "XF86AudioLowerVolume", Exec("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%- || pactl set-sink-volume @DEFAULT_SINK@ -5%".into())),
            b("", "XF86AudioMute", Exec("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle || pactl set-sink-mute @DEFAULT_SINK@ toggle".into())),
            b("", "XF86AudioMicMute", Exec("wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle || pactl set-source-mute @DEFAULT_SOURCE@ toggle".into())),
            b("", "XF86MonBrightnessUp", Exec("brightnessctl -e4 -n2 set 5%+".into())),
            b("", "XF86MonBrightnessDown", Exec("brightnessctl -e4 -n2 set 5%-".into())),
            b("", "XF86AudioNext", Exec("playerctl next".into())),
            b("", "XF86AudioPause", Exec("playerctl play-pause".into())),
            b("", "XF86AudioPlay", Exec("playerctl play-pause".into())),
            b("", "XF86AudioPrev", Exec("playerctl previous".into())),
        ];
        for n in 1..=9u32 {
            binds.push(b("SUPER", &n.to_string(), Workspace(n)));
            binds.push(b("SUPER+SHIFT", &n.to_string(), MoveToWorkspace(n)));
        }
        Self {
            general: General::default(),
            decoration: Decoration::default(),
            animations: Animations::default(),
            input: Input::default(),
            layout: LayoutCfg::default(),
            // every screen is used as it comes (add [[monitor]] entries to set a mode, scale or position)
            monitors: vec![],
            binds,
            mouse_binds: vec![
                MouseBind { mods: "SUPER".into(), button: MouseButton::Left, action: MouseAction::MoveWindow },
                MouseBind { mods: "SUPER".into(), button: MouseButton::Right, action: MouseAction::ResizeWindow },
            ],
            rules: ["codium", "codium-url-handler", "VSCodium", "code"]
                .iter()
                .map(|c| WindowRule { app_id: Some((*c).into()), opacity: Some(0.92), ..WindowRule::default() })
                // the phone's screen floats, at the phone's own size
                .chain(std::iter::once(WindowRule { app_id: Some("scrcpy".into()), floating: Some(true), ..WindowRule::default() }))
                .collect(),
            scratchpads: vec![],
            remote: RemoteCfg::default(),
            update: UpdateCfg::default(),
            exec_once: vec![],
            shell: ShellCfg::default(),
            theme: theme::ThemeCfg::default(),
            own: defaults::OwnCfg::default(),
            terminal: defaults::TerminalCfg::default(),
            env: vec![],
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Ser(toml::ser::Error),
}
impl Error {
    /// The line of the config file a parse error points at (1-based).
    pub fn line(&self) -> Option<usize> {
        match self {
            Error::Parse(e) => {
                let span = e.span()?;
                let text = std::fs::read_to_string(Config::path()).ok()?;
                Some(text.get(..span.start.min(text.len()))?.matches('\n').count() + 1)
            }
            _ => None,
        }
    }

    /// The error without the source excerpt toml adds (for a one-line message).
    pub fn short(&self) -> String {
        match self {
            Error::Parse(e) => e.message().trim().to_string(),
            other => other.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io: {e}"),
            Error::Parse(e) => write!(f, "parse: {e}"),
            Error::Ser(e) => write!(f, "serialize: {e}"),
        }
    }
}
impl std::error::Error for Error {}

impl Config {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("oxide").join("oxide.toml")
    }

    pub fn from_toml(s: &str) -> Result<Self, Error> {
        toml::from_str(s).map_err(Error::Parse)
    }

    pub fn to_toml(&self) -> Result<String, Error> {
        toml::to_string_pretty(self).map_err(Error::Ser)
    }

    /// Load from disk; a missing file yields defaults.
    pub fn load() -> Result<Self, Error> {
        match std::fs::read_to_string(Self::path()) {
            Ok(s) => Self::from_toml(&s).map(|mut c| {
                // the first bar lived in ~/.config/oxide/bar and was started from exec_once: the shell replaced it
                c.exec_once.retain(|cmd| !cmd.contains(".config/oxide/bar"));
                c.add_missing_shell_binds();
                // hyprlock was the first screen locker; the shell has its own lock screen now
                for b in c.binds.iter_mut() {
                    if matches!(&b.action, Action::Exec(cmd) if cmd.trim() == "hyprlock") {
                        b.action = Action::Exec("oxide-ctl lock".into());
                    }
                }
                // older configs called the shell by its path; `oxide-ctl ipc` finds the active shell by itself
                for b in c.binds.iter_mut() {
                    if let Action::Exec(cmd) = &b.action {
                        if let Some(rest) = cmd.trim().strip_prefix("quickshell -p $HOME/.config/oxide/active-shell ipc call ") {
                            b.action = Action::Exec(format!("oxide-ctl ipc {rest}"));
                        } else if cmd.trim() == "kitty -e yazi" {
                            b.action = Action::Exec("oxide-ctl files".into());
                        }
                    }
                }
                // rofi was the first launcher; the shell has its own now
                for b in c.binds.iter_mut() {
                    if matches!(&b.action, Action::Exec(cmd) if cmd.trim() == "rofi -show drun") {
                        b.action = Action::Exec("oxide-ctl launcher".into());
                    }
                }
                c
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::Io(e)),
        }
    }

    /// True when the bundled Oxide shell is what runs (bar, launcher, wallpaper): its own settings pages apply.
    pub fn default_shell_active(&self) -> bool {
        self.shell.enabled && (!self.own.shell || self.shell.active == "oxide")
    }

    /// Older config files do not know the shell shortcuts: add them unless the key is taken.
    pub fn add_missing_shell_binds(&mut self) {
        for d in Config::default().binds.into_iter().filter(|b| matches!(&b.action, Action::SendKey(_)) || (matches!(&b.action, Action::Exec(c) if c.contains("active-shell") || c.starts_with("oxide-ctl ipc ") || c == "oxide-ctl phone") || matches!(&b.action, Action::ToggleOverview))) {
            let taken = self.binds.iter().any(|b| b.mods.eq_ignore_ascii_case(&d.mods) && b.key.eq_ignore_ascii_case(&d.key));
            let has_action = self.binds.iter().any(|b| b.action == d.action);
            if !taken && !has_action {
                self.binds.push(d);
            }
        }
    }

    pub fn save(&self) -> Result<(), Error> {
        let p = Self::path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(Error::Io)?;
        }
        let tmp = p.with_extension("toml.tmp");
        std::fs::write(&tmp, self.to_toml()?).map_err(Error::Io)?;
        std::fs::rename(&tmp, &p).map_err(Error::Io)
    }
}

/// Parse "#rrggbb" or "#rrggbbaa" into RGBA floats.
pub fn parse_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim();
    let h = s.strip_prefix('#').or_else(|| s.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')))?;
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok().map(|b| b as f32 / 255.0);
    match h.len() {
        6 => Some([v(0)?, v(2)?, v(4)?, 1.0]),
        8 => Some([v(0)?, v(2)?, v(4)?, v(6)?]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let c = Config::default();
        let s = c.to_toml().unwrap();
        assert_eq!(Config::from_toml(&s).unwrap(), c);
    }
    #[test]
    fn partial_file_uses_defaults() {
        let c = Config::from_toml("[general]\ngaps_in = 1\n").unwrap();
        assert_eq!(c.general.gaps_in, 1);
        assert_eq!(c.general.gaps_out, General::default().gaps_out);
        assert!(!c.binds.is_empty());
    }
    #[test]
    fn colors() {
        assert_eq!(parse_color("#ff0000"), Some([1.0, 0.0, 0.0, 1.0]));
        assert!(parse_color("red").is_none());
    }
}

/// Where the installer puts the shells that ship with Oxide: `~/.local/share/oxide/shells`.
pub fn bundled_shells_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("oxide/shells")
}

/// Where the shipped copy of shell `name` lives: the user's data dir (a user install), else the system's
/// (/usr/local/share, /usr/share: what a distro package or `make install` provides).
pub fn bundled_shell_path(name: &str) -> Option<PathBuf> {
    [bundled_shells_dir(), "/usr/local/share/oxide/shells".into(), "/usr/share/oxide/shells".into()]
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.join("shell.qml").is_file())
}

/// Fingerprint of a shell folder (file names and contents).
fn dir_stamp(dir: &std::path::Path) -> u64 {
    fn walk(dir: &std::path::Path, base: &std::path::Path, files: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, files);
            } else if p.file_name().is_some_and(|n| n != ".managed") {
                files.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.sort();
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |b: &[u8]| {
        for x in b {
            h ^= *x as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for f in files {
        eat(f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().as_bytes());
        eat(&std::fs::read(&f).unwrap_or_default());
    }
    h
}

/// Copy a shell that ships with Oxide into the user's shells folder. A copy made by Oxide (it carries a
/// `.managed` stamp) is refreshed when the shipped version changes; the old one is kept in `~/.config/oxide/shell-backups`.
/// A folder without the stamp is the user's own and is never touched, except the default shell
/// named "oxide", which predates the stamp (it is refreshed once, with a backup, and stamped).
pub fn ensure_bundled_shell(name: &str) {
    let dest = shells_dir().join(name);
    let Some(src) = bundled_shell_path(name) else { return };
    let stamp = dir_stamp(&src).to_string();
    let installed = dest.join("shell.qml").is_file();
    if installed {
        let marker = std::fs::read_to_string(dest.join(".managed")).ok();
        let legacy = marker.is_none() && name == "oxide";
        let stale = marker.as_deref().map(str::trim) != Some(stamp.as_str());
        if !(legacy || (marker.is_some() && stale)) {
            return;
        }
        let bak = Config::path().with_file_name("shell-backups").join(name);
        let _ = std::fs::create_dir_all(bak.parent().unwrap_or(&dest));
        let _ = std::fs::remove_dir_all(&bak);
        let _ = std::fs::rename(&dest, &bak);
    }
    let _ = std::fs::create_dir_all(shells_dir());
    let _ = std::process::Command::new("cp").arg("-a").arg(&src).arg(&dest).status();
    let _ = std::fs::write(dest.join(".managed"), stamp);
}
