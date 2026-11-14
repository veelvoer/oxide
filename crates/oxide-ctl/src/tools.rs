//! Helpers that make the default shortcuts work on any distro: find a terminal or file manager that is
//! installed, call the running shell, and check what the system is missing (`oxide-ctl doctor`).
use std::process::{Command, Stdio};

fn which(bin: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(bin)).find(|p| p.is_file())
}

fn spawn(bin: &str, args: &[&str]) -> bool {
    Command::new(bin).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

/// Terminals in the order Oxide prefers them; `$TERMINAL` wins when it is set and installed.
const TERMINALS: &[&str] = &["kitty", "foot", "alacritty", "wezterm", "ghostty", "gnome-terminal", "konsole", "xfce4-terminal", "tilix", "xterm"];

pub fn terminal_bin() -> Option<String> {
    if let Some(t) = std::env::var("TERMINAL").ok().filter(|t| !t.is_empty() && which(t.split_whitespace().next().unwrap_or("")).is_some()) {
        return Some(t);
    }
    TERMINALS.iter().find(|t| which(t).is_some()).map(|t| t.to_string())
}

/// `oxide-ctl terminal [command...]`: open a terminal (running a command when given).
pub fn terminal(args: &[String]) -> ! {
    let Some(t) = terminal_bin() else {
        eprintln!("no terminal found: install kitty, foot, alacritty or another one (see `oxide-ctl doctor`)");
        std::process::exit(1)
    };
    let mut parts = t.split_whitespace();
    let bin = parts.next().unwrap_or("xterm").to_string();
    let mut cmd = Command::new(&bin);
    cmd.args(parts);
    if !args.is_empty() {
        // -e works for nearly all of them; gnome-terminal wants --
        if bin == "gnome-terminal" {
            cmd.arg("--");
        } else {
            cmd.arg("-e");
        }
        cmd.args(args);
    }
    let ok = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok();
    std::process::exit(if ok { 0 } else { 1 })
}

/// `oxide-ctl files`: a terminal file manager when there is one, else the desktop's file manager on $HOME.
pub fn files() -> ! {
    for tui in ["yazi", "ranger", "lf", "nnn", "mc"] {
        if which(tui).is_some() && terminal_bin().is_some() {
            terminal(&[tui.to_string()]);
        }
    }
    for gui in ["nautilus", "dolphin", "thunar", "pcmanfm", "nemo", "caja"] {
        if which(gui).is_some() && spawn(gui, &[]) {
            std::process::exit(0)
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    std::process::exit(if spawn("xdg-open", &[&home]) { 0 } else { 1 })
}

/// `oxide-ctl ipc <target> <function> [args]`: call a function of the running shell (e.g. `ipc shell toggleControl`).
/// Works with whatever shell is active; quietly does nothing when none is running.
pub fn ipc(args: &[String]) -> ! {
    let link = oxide_config::active_shell_link();
    let ok = Command::new("quickshell")
        .arg("-p")
        .arg(&link)
        .args(["ipc", "call"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    std::process::exit(if ok { 0 } else { 1 })
}

struct Check {
    what: &'static str,
    needed_for: &'static str,
    bins: &'static [&'static str],
    /// package names: (apt, dnf, pacman)
    pkg: (&'static str, &'static str, &'static str),
    required: bool,
}

const CHECKS: &[Check] = &[
    Check { what: "Quickshell", needed_for: "the bar, launcher and panels", bins: &["quickshell"], pkg: ("quickshell (see docs)", "quickshell", "quickshell"), required: true },
    Check { what: "A terminal", needed_for: "Super+Q", bins: &["kitty", "foot", "alacritty", "wezterm", "ghostty", "gnome-terminal", "konsole", "xterm"], pkg: ("kitty", "kitty", "kitty"), required: true },
    Check { what: "Xwayland", needed_for: "older X11 apps", bins: &["Xwayland"], pkg: ("xwayland", "xorg-x11-server-Xwayland", "xorg-xwayland"), required: false },
    Check { what: "grim + slurp", needed_for: "screenshots (Print)", bins: &["grim", "slurp"], pkg: ("grim slurp", "grim slurp", "grim slurp"), required: false },
    Check { what: "wl-clipboard", needed_for: "clipboard history", bins: &["wl-copy", "wl-paste"], pkg: ("wl-clipboard", "wl-clipboard", "wl-clipboard"), required: false },
    Check { what: "PipeWire (wpctl) or PulseAudio (pactl)", needed_for: "volume keys and sound settings", bins: &["wpctl", "pactl"], pkg: ("wireplumber", "wireplumber", "wireplumber"), required: false },
    Check { what: "brightnessctl", needed_for: "brightness keys", bins: &["brightnessctl"], pkg: ("brightnessctl", "brightnessctl", "brightnessctl"), required: false },
    Check { what: "playerctl", needed_for: "media keys", bins: &["playerctl"], pkg: ("playerctl", "playerctl", "playerctl"), required: false },
    Check { what: "NetworkManager (nmcli)", needed_for: "the Wi-Fi page and tile", bins: &["nmcli"], pkg: ("network-manager", "NetworkManager", "networkmanager"), required: false },
    Check { what: "BlueZ (bluetoothctl)", needed_for: "the Bluetooth page and tile", bins: &["bluetoothctl"], pkg: ("bluez", "bluez", "bluez"), required: false },
    Check { what: "A polkit agent", needed_for: "apps that ask for admin rights", bins: &["polkit-kde-authentication-agent-1", "lxqt-policykit-agent", "polkit-gnome-authentication-agent-1", "hyprpolkitagent"], pkg: ("policykit-1-gnome", "polkit-gnome", "polkit-gnome"), required: false },
    Check { what: "xdg-desktop-portal", needed_for: "screen sharing and file pickers", bins: &["xdg-desktop-portal"], pkg: ("xdg-desktop-portal xdg-desktop-portal-gtk", "xdg-desktop-portal xdg-desktop-portal-gtk", "xdg-desktop-portal xdg-desktop-portal-gtk"), required: false },
];

/// `oxide-ctl doctor`: what works, what is missing and how to install it.
pub fn doctor() -> ! {
    let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let id_like = os.lines().filter(|l| l.starts_with("ID=") || l.starts_with("ID_LIKE=")).collect::<Vec<_>>().join(" ").to_lowercase();
    let family = if id_like.contains("debian") || id_like.contains("ubuntu") {
        "apt"
    } else if id_like.contains("fedora") || id_like.contains("rhel") || id_like.contains("suse") {
        "dnf"
    } else if id_like.contains("arch") {
        "pacman"
    } else {
        "other"
    };
    println!("Oxide doctor ({} family)\n", family);
    let mut missing_required = false;
    for c in CHECKS {
        let found: Vec<&str> = c.bins.iter().copied().filter(|b| which(b).is_some()).collect();
        // the "grim + slurp" style entries need all binaries, the "or" style any one
        let ok = if c.what.contains('+') { found.len() == c.bins.len() } else { !found.is_empty() };
        if ok {
            println!("  ok       {:<42} {}", c.what, found.join(", "));
        } else {
            let hint = match family {
                "apt" => format!("sudo apt install {}", c.pkg.0),
                "dnf" => format!("sudo dnf install {}", c.pkg.1),
                "pacman" => format!("sudo pacman -S {}", c.pkg.2),
                _ => format!("install: {}", c.pkg.1),
            };
            println!("  {}  {:<42} needed for {}\n           {hint}", if c.required { "MISSING" } else { "missing" }, c.what, c.needed_for);
            missing_required |= c.required;
        }
    }
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    println!("\n  session   {}", if session.is_empty() { "(not in a graphical session)" } else { &session });
    println!("  config    {}", oxide_config::Config::path().display());
    println!("  shell     {}", oxide_config::active_shell_link().display());
    let ok = oxide_ipc::call(&oxide_ipc::Request::GetState).is_ok();
    println!("  running   {}", if ok { "yes, Oxide answers" } else { "no (start Oxide to check the rest)" });
    println!("\nMissing optional tools only switch off the feature named next to them.");
    std::process::exit(if missing_required { 1 } else { 0 })
}
