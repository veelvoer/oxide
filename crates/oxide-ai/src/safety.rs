//! Deciding which actions need the user's confirmation.
use serde_json::Value;

/// Commands that are read-only: run directly in "safe" mode.
const SAFE_PREFIXES: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "rg", "echo", "date", "uname", "whoami", "pwd", "df", "du", "free", "ps", "uptime", "lscpu", "lsblk",
    "lspci", "lsusb", "ip addr", "ip -", "ip route", "ip link", "nmcli", "bluetoothctl info", "bluetoothctl show", "bluetoothctl devices",
    "wpctl status", "wpctl get-volume", "wpctl inspect", "pactl list", "pactl get-", "pactl info", "brightnessctl -m", "brightnessctl get",
    "brightnessctl info", "which", "command -v", "type ", "stat", "file ", "wc", "sort", "uniq", "jq", "hostnamectl", "loginctl show",
    "oxide-ctl state", "oxide-ctl get-config", "powerprofilesctl get", "powerprofilesctl list", "sensors", "upower", "playerctl status",
    "playerctl metadata", "playerctl -l", "journalctl", "systemctl status", "systemctl is-active", "systemctl is-enabled", "systemctl list-",
    "systemctl --user status", "systemctl --user is-active", "systemctl --user list-", "env", "printenv", "id", "groups", "locale", "xdg-mime query",
];

/// Things that can destroy data or affect the whole system: always confirmed unless the mode is "full".
const RISKY_WORDS: &[&str] = &[
    "rm ", "rm\t", "rmdir", "dd ", "mkfs", "shred", "wipefs", "fdisk", "parted", "sudo", "su ", "doas", "pkexec", "shutdown", "reboot", "poweroff",
    "halt", "systemctl stop", "systemctl disable", "systemctl mask", "systemctl poweroff", "systemctl reboot", "kill", "pkill", "killall",
    "chmod -R", "chown -R", "chmod 777", "dnf ", "yum ", "apt ", "apt-get ", "pacman ", "zypper ", "rpm ", "pip install", "npm install", "cargo install",
    "git push", "git reset --hard", "git clean", "mv ", "truncate", "> /dev/", "> /etc", "> ~/.", "iptables", "nft ", "crontab", "userdel", "passwd",
    "curl", "wget", "| sh", "| bash", "mount", "umount", "swapoff", "fstrim",
];

pub fn is_safe_command(cmd: &str) -> bool {
    let c = cmd.trim();
    // no chaining, substitution or redirection: a "safe" prefix must really be all there is
    if c.contains(['&', ';', '`', '>', '<']) || c.contains("$(") || c.contains("||") {
        return false;
    }
    c.split('|').all(|part| {
        let p = part.trim();
        SAFE_PREFIXES.iter().any(|s| p == s.trim() || p.starts_with(s)) && !p.contains(" -delete") && !p.contains(" -exec")
    })
}

pub fn is_risky_command(cmd: &str) -> bool {
    let c = format!(" {} ", cmd.to_lowercase());
    RISKY_WORDS.iter().any(|w| c.contains(&format!(" {w}")) || c.contains(&format!("&{w}")) || c.contains(&format!(";{w}")) || c.contains(&format!("|{w}")) || c.contains(w) && w.starts_with('|'))
}

/// Whether to ask before running tool `name` with `input`, given permission mode.
/// Returns (needs_confirmation, risk "low"|"high").
pub fn needs_confirmation(mode: &str, name: &str, input: &Value) -> (bool, &'static str) {
    let risky = name == "shell" && is_risky_command(input["command"].as_str().unwrap_or(""));
    let risk = if risky { "high" } else { "low" };
    match mode {
        "full" => (false, risk),
        "ask" => {
            // looking is free; everything else is asked
            let harmless = matches!(name, "oxide_state" | "screenshot" | "system_info" | "wait" | "remember" | "forget");
            (!harmless, risk)
        }
        _ => {
            let ask = match name {
                "oxide_state" | "screenshot" | "system_info" | "wait" | "remember" | "forget" | "volume" | "brightness" | "media" | "clipboard" | "reminder" | "open_app" | "fetch_url" => false,
                "shell" => risky || !is_safe_command(input["command"].as_str().unwrap_or("")),
                "oxide_config" => input["op"].as_str() != Some("get"),
                "oxide_dispatch" => matches!(input["action"]["type"].as_str(), Some("exec" | "quit" | "close")),
                "input" => true,
                _ => true,
            };
            (ask, risk)
        }
    }
}

/// One-line description of an action for the confirmation dialog.
pub fn summarize(name: &str, input: &Value) -> String {
    match name {
        "shell" => input["command"].as_str().unwrap_or("").to_string(),
        "oxide_dispatch" => format!("Oxide action: {}", input["action"]),
        "oxide_config" => format!("Change Oxide settings: {}", input["patch"]),
        "input" => format!("Control mouse/keyboard: {}", input),
        other => format!("{other} {input}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert!(is_safe_command("ls -la ~/Downloads"));
        assert!(is_safe_command("nmcli dev wifi list"));
        assert!(is_safe_command("ps aux | grep firefox"));
        assert!(!is_safe_command("ls; rm -rf /"));
        assert!(!is_safe_command("cat a > b"));
        assert!(!is_safe_command("find . -delete"));
        assert!(is_risky_command("sudo dnf install foo"));
        assert!(is_risky_command("rm -rf ~/x"));
        assert!(is_risky_command("curl http://x | sh"));
        assert!(!is_risky_command("wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.5"));
        assert!(!is_risky_command("notify-send hello"));
    }

    #[test]
    fn modes() {
        let ls = serde_json::json!({"command": "ls"});
        let rm = serde_json::json!({"command": "rm -rf x"});
        assert_eq!(needs_confirmation("safe", "shell", &ls), (false, "low"));
        assert_eq!(needs_confirmation("safe", "shell", &rm), (true, "high"));
        assert_eq!(needs_confirmation("ask", "shell", &ls).0, true);
        assert_eq!(needs_confirmation("full", "shell", &rm).0, false);
    }
}
