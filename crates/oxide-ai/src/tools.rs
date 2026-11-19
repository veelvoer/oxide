//! The tools the assistant can use: shell, Oxide actions and settings, screenshots and synthetic input.
use std::process::Command;

use base64::Engine;
use oxide_config::{ai::AiConfig, Action, Config};
use oxide_ipc::{call, InputCmd, Request, Response};
use serde_json::{json, Value};

pub struct ToolOutput {
    pub ok: bool,
    pub text: String,
    pub image_b64: Option<String>,
}

impl ToolOutput {
    fn ok(text: impl Into<String>) -> Self {
        Self { ok: true, text: text.into(), image_b64: None }
    }
    fn err(text: impl Into<String>) -> Self {
        Self { ok: false, text: text.into(), image_b64: None }
    }
}

pub fn definitions(cfg: &AiConfig) -> Vec<Value> {
    let mut t = vec![];
    if cfg.allow_shell {
        t.push(json!({
            "name": "shell",
            "description": "Run a bash command (Linux, Wayland; no sudo). For anything the other tools do not cover: Wi-Fi (nmcli), Bluetooth (bluetoothctl), files, packages, power. Start GUI apps with open_app instead. Working directory is home.",
            "input_schema": {"type": "object", "properties": {
                "command": {"type": "string", "description": "The bash command line."},
                "timeout_secs": {"type": "integer", "description": "Give up after this many seconds (default 30, max 300)."}
            }, "required": ["command"], "additionalProperties": false}
        }));
    }
    let small = |name: &str, desc: &str, props: Value, req: &[&str]| {
        json!({"name": name, "description": desc, "input_schema": {"type": "object", "properties": props, "required": req, "additionalProperties": false}})
    };
    t.push(small("open_app", "Start an app (name, .desktop id or command), or open a file/URL with its default app.", json!({"target": {"type": "string"}}), &["target"]));
    t.push(small("volume", "Set output volume 0-100 and/or mute; with no arguments returns the current volume.", json!({"percent": {"type": "integer"}, "mute": {"type": "boolean"}}), &[]));
    t.push(small("brightness", "Set screen brightness 1-100 (percent); with no arguments returns it.", json!({"percent": {"type": "integer"}}), &[]));
    t.push(small("media", "Control the media player.", json!({"action": {"type": "string", "enum": ["play-pause", "next", "previous", "stop", "status"]}}), &["action"]));
    t.push(small("clipboard", "Read the clipboard, or set it when `text` is given.", json!({"text": {"type": "string"}}), &[]));
    t.push(small("system_info", "RAM, CPU load, battery, uptime, disk and the busiest processes.", json!({}), &[]));
    t.push(small("fetch_url", "Fetch a web page as plain text.", json!({"url": {"type": "string"}}), &["url"]));
    t.push(small("reminder", "Show a notification after N seconds.", json!({"seconds": {"type": "integer"}, "message": {"type": "string"}}), &["seconds", "message"]));
    t.push(small("wait", "Wait up to 10 seconds, e.g. for an app to open.", json!({"seconds": {"type": "number"}}), &["seconds"]));
    t.push(small("remember", "Store a lasting fact about the user (preferences, habits, names). One short sentence.", json!({"fact": {"type": "string"}}), &["fact"]));
    t.push(small("forget", "Delete stored facts that contain this text.", json!({"text": {"type": "string"}}), &["text"]));
    t.push(json!({
        "name": "oxide_state",
        "description": "Oxide desktop state: workspaces, open windows (app id, title, workspace, focus), screens, layout.",
        "input_schema": {"type": "object", "properties": {}, "additionalProperties": false}
    }));
    t.push(json!({
        "name": "oxide_dispatch",
        "description": "Window-manager action, e.g. {\"type\":\"workspace\",\"arg\":3}. Types: close, toggle_floating, toggle_fullscreen, focus_dir/move_dir (left|right|up|down), workspace/move_to_workspace (number), workspace_rel/move_to_workspace_rel/workspace_existing (+1|-1), toggle_special, move_to_special, focus_monitor/move_to_monitor (direction), cycle_layout, grow_master, shrink_master, send_key (arg like \"alt+Left\"), toggle_overview, lock, set_profile (arg eco|normal|performance), reload_config.",
        "input_schema": {"type": "object", "properties": {"action": {"type": "object", "properties": {"type": {"type": "string"}, "arg": {}}, "required": ["type"]}}, "required": ["action"], "additionalProperties": false}
    }));
    if cfg.allow_config {
        t.push(json!({
            "name": "oxide_config",
            "description": "Oxide settings (gaps, borders, rounding, blur, animations, input, layout, monitors, rules, theme...). op=get [section] reads them; op=patch merges a JSON object, applies and saves. Call get for the section first to use exact key names. Arrays are replaced whole.",
            "input_schema": {"type": "object", "properties": {
                "op": {"type": "string", "enum": ["get", "patch"]},
                "section": {"type": "string", "description": "For get: general, decoration, animations, input, layout, monitor, bind, rule, shell, power, remote, exec_once, env."},
                "patch": {"type": "object"}
            }, "required": ["op"], "additionalProperties": false}
        }));
    }
    if cfg.allow_screenshot {
        t.push(json!({
            "name": "screenshot",
            "description": "Look at the screen. The result says how to convert image pixels to `input` coordinates. Only for graphical tasks no other tool can do.",
            "input_schema": {"type": "object", "properties": {}, "additionalProperties": false}
        }));
    }
    if cfg.allow_input {
        t.push(json!({
            "name": "input",
            "description": "Mouse and keyboard like a user (global logical pixels, see screenshot). Last resort after shell and oxide_* tools.",
            "input_schema": {"type": "object", "properties": {
                "action": {"type": "string", "enum": ["move", "click", "double_click", "scroll", "key", "type"]},
                "x": {"type": "number"}, "y": {"type": "number"},
                "button": {"type": "string", "enum": ["left", "right", "middle"]},
                "dx": {"type": "number"}, "dy": {"type": "number", "description": "Scroll amount; positive dy scrolls down."},
                "key": {"type": "string", "description": "xkb key name such as Return, Escape, Tab, a, F5, Left."},
                "mods": {"type": "array", "items": {"type": "string", "enum": ["ctrl", "alt", "shift", "super"]}},
                "text": {"type": "string", "description": "Text to type."}
            }, "required": ["action"], "additionalProperties": false}
        }));
    }
    t
}

/// The API's own web search tool (runs on Anthropic's side).
pub fn web_search_tool() -> Value {
    json!({"type": "web_search_20250305", "name": "web_search", "max_uses": 3})
}

fn cap(mut s: String) -> String {
    const MAX: usize = 6_000;
    if s.chars().count() > MAX {
        let head: String = s.chars().take(3_000).collect();
        let tail: String = s.chars().rev().take(2_500).collect::<Vec<_>>().into_iter().rev().collect();
        s = format!("{head}\n...[output truncated]...\n{tail}");
    }
    s
}

fn shell(input: &Value) -> ToolOutput {
    let Some(cmd) = input["command"].as_str() else { return ToolOutput::err("missing command") };
    let secs = input["timeout_secs"].as_u64().unwrap_or(30).clamp(1, 300);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let out = Command::new("timeout")
        .arg(format!("{secs}s"))
        .args(["bash", "-lc", cmd])
        .current_dir(home)
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                text.push_str("\n[stderr]\n");
                text.push_str(&err);
            }
            let code = o.status.code().unwrap_or(-1);
            if code == 124 {
                text.push_str(&format!("\n[timed out after {secs}s]"));
            }
            if text.trim().is_empty() {
                text = "(no output)".into();
            }
            ToolOutput { ok: code == 0, text: cap(format!("{text}\n[exit code {code}]")), image_b64: None }
        }
        Err(e) => ToolOutput::err(format!("could not run: {e}")),
    }
}

fn dispatch(input: &Value) -> ToolOutput {
    match serde_json::from_value::<Action>(input["action"].clone()) {
        Ok(action) => match call(&Request::Dispatch { action }) {
            Ok(Response::Ok) => ToolOutput::ok("done"),
            Ok(Response::Error { message }) => ToolOutput::err(message),
            Ok(_) => ToolOutput::ok("done"),
            Err(e) => ToolOutput::err(format!("Oxide is not reachable: {e}")),
        },
        Err(e) => ToolOutput::err(format!("invalid action: {e}")),
    }
}

fn state() -> ToolOutput {
    match call(&Request::GetState) {
        Ok(Response::State { state }) => ToolOutput::ok(serde_json::to_string_pretty(&state).unwrap_or_default()),
        Ok(Response::Error { message }) => ToolOutput::err(message),
        Ok(_) => ToolOutput::err("unexpected reply"),
        Err(e) => ToolOutput::err(format!("Oxide is not reachable: {e}")),
    }
}

fn merge(into: &mut Value, patch: &Value) {
    match (into, patch) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

fn live_config() -> Result<Config, String> {
    match call(&Request::GetConfig) {
        Ok(Response::Config { config }) => Ok(*config),
        Ok(Response::Error { message }) => Err(message),
        Ok(_) => Err("unexpected reply".into()),
        // not running: work on the file
        Err(_) => Config::load().map_err(|e| e.to_string()),
    }
}

fn config(input: &Value) -> ToolOutput {
    let cfg = match live_config() {
        Ok(c) => c,
        Err(e) => return ToolOutput::err(e),
    };
    let mut value = serde_json::to_value(&cfg).unwrap_or(Value::Null);
    match input["op"].as_str() {
        Some("get") => {
            if let Some(sec) = input["section"].as_str() {
                return match value.get(sec) {
                    Some(v) => ToolOutput::ok(serde_json::to_string_pretty(v).unwrap_or_default()),
                    None => ToolOutput::err(format!("no section {sec:?}; sections: {}", value.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default())),
                };
            }
            // compact overview: shortcuts as one line each
            let binds: Vec<String> = cfg.binds.iter().map(|b| format!("{}+{} -> {}", b.mods, b.key, serde_json::to_string(&b.action).unwrap_or_default())).collect();
            if let Some(o) = value.as_object_mut() {
                o.insert("bind".into(), json!(binds));
                if let Some(a) = o.get_mut("animations").and_then(|a| a.as_object_mut()) {
                    a.remove("curves");
                }
            }
            ToolOutput::ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }
        Some("patch") => {
            let patch = &input["patch"];
            if !patch.is_object() {
                return ToolOutput::err("patch must be an object");
            }
            merge(&mut value, patch);
            let new: Config = match serde_json::from_value(value) {
                Ok(c) => c,
                Err(e) => return ToolOutput::err(format!("the patch does not fit the settings: {e}")),
            };
            match call(&Request::SetConfig { config: Box::new(new.clone()) }) {
                Ok(Response::Error { message }) => return ToolOutput::err(message),
                Err(_) => {} // compositor not running: only the file changes
                _ => {}
            }
            match new.save() {
                Ok(()) => ToolOutput::ok("settings applied and saved"),
                Err(e) => ToolOutput::err(format!("applied but could not save: {e}")),
            }
        }
        _ => ToolOutput::err("op must be get or patch"),
    }
}

fn screenshot() -> ToolOutput {
    const SCALE: f64 = 0.8;
    let out = Command::new("grim").args(["-t", "png", "-s", &SCALE.to_string(), "-"]).output();
    let png = match out {
        Ok(o) if o.status.success() && o.stdout.len() > 24 => o.stdout,
        Ok(o) => return ToolOutput::err(format!("grim failed: {}", String::from_utf8_lossy(&o.stderr))),
        Err(_) => return ToolOutput::err("grim is not installed"),
    };
    let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    let out_scale = match call(&Request::GetState) {
        Ok(Response::State { state }) => state.outputs.first().map_or(1.0, |o| o.scale),
        _ => 1.0,
    };
    let k = 1.0 / (SCALE * out_scale);
    ToolOutput {
        ok: true,
        text: format!("Screenshot {w}x{h} px. To click something at pixel (px, py) in this image use input coordinates x = px * {k:.3}, y = py * {k:.3}."),
        image_b64: Some(base64::engine::general_purpose::STANDARD.encode(&png)),
    }
}

fn input_tool(input: &Value) -> ToolOutput {
    let f = |k: &str| input[k].as_f64();
    let button = input["button"].as_str().unwrap_or("left").to_string();
    let cmd = match input["action"].as_str() {
        Some("move") => f("x").zip(f("y")).map(|(x, y)| InputCmd::Move { x, y }),
        Some("click") => f("x").zip(f("y")).map(|(x, y)| InputCmd::Click { x, y, button }),
        Some("double_click") => f("x").zip(f("y")).map(|(x, y)| InputCmd::DoubleClick { x, y }),
        Some("scroll") => Some(InputCmd::Scroll { dx: f("dx").unwrap_or(0.0), dy: f("dy").unwrap_or(0.0) }),
        Some("key") => input["key"].as_str().map(|k| InputCmd::Key {
            name: k.to_string(),
            mods: input["mods"].as_array().map(|a| a.iter().filter_map(|m| m.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        }),
        Some("type") => input["text"].as_str().map(|t| InputCmd::Type { text: t.to_string() }),
        _ => None,
    };
    let Some(cmd) = cmd else { return ToolOutput::err("missing or invalid arguments for this input action") };
    match call(&Request::Input { input: cmd }) {
        Ok(Response::Error { message }) => ToolOutput::err(message),
        Ok(_) => ToolOutput::ok("done"),
        Err(e) => ToolOutput::err(format!("Oxide is not reachable: {e}")),
    }
}

fn sh(cmd: &str, args: &[&str]) -> Result<String, String> {
    let o = Command::new(cmd).args(args).stdin(std::process::Stdio::null()).output().map_err(|_| format!("{cmd} is not installed"))?;
    let mut t = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
    if !o.status.success() {
        return Err(if e.is_empty() { t } else { e });
    }
    if t.is_empty() {
        t = "ok".into();
    }
    Ok(t)
}

fn res(r: Result<String, String>) -> ToolOutput {
    match r {
        Ok(t) => ToolOutput::ok(t),
        Err(e) => ToolOutput::err(e),
    }
}

fn open_app(input: &Value) -> ToolOutput {
    let Some(t) = input["target"].as_str().map(str::trim).filter(|t| !t.is_empty()) else { return ToolOutput::err("missing target") };
    let is_target = t.contains("://") || t.starts_with('/') || t.starts_with('~');
    let script = if is_target {
        format!("setsid xdg-open {} >/dev/null 2>&1 &", shell_quote(&expand_home(t)))
    } else {
        // a command on PATH, else a desktop file name
        format!("if command -v {q} >/dev/null 2>&1; then setsid {t} >/dev/null 2>&1 & else setsid gtk-launch {q} >/dev/null 2>&1 & fi", q = shell_quote(t))
    };
    let _ = Command::new("bash").args(["-c", &script]).spawn();
    ToolOutput::ok("started")
}

fn expand_home(p: &str) -> String {
    match p.strip_prefix('~') {
        Some(rest) => format!("{}{rest}", std::env::var("HOME").unwrap_or_default()),
        None => p.to_string(),
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn volume(input: &Value) -> ToolOutput {
    const SINK: &str = "@DEFAULT_AUDIO_SINK@";
    if let Some(p) = input["percent"].as_i64() {
        if let Err(e) = sh("wpctl", &["set-volume", SINK, &format!("{}%", p.clamp(0, 100))]) {
            return ToolOutput::err(e);
        }
    }
    if let Some(m) = input["mute"].as_bool() {
        if let Err(e) = sh("wpctl", &["set-mute", SINK, if m { "1" } else { "0" }]) {
            return ToolOutput::err(e);
        }
    }
    res(sh("wpctl", &["get-volume", SINK]))
}

fn brightness(input: &Value) -> ToolOutput {
    if let Some(p) = input["percent"].as_i64() {
        if let Err(e) = sh("brightnessctl", &["set", &format!("{}%", p.clamp(1, 100))]) {
            return ToolOutput::err(e);
        }
    }
    res(sh("brightnessctl", &["-m"]).map(|t| t.split(',').nth(3).unwrap_or(&t).to_string()))
}

fn clipboard(input: &Value) -> ToolOutput {
    if let Some(t) = input["text"].as_str() {
        use std::io::Write;
        return match Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn() {
            Ok(mut c) => {
                if let Some(mut i) = c.stdin.take() {
                    let _ = i.write_all(t.as_bytes());
                }
                ToolOutput::ok("copied")
            }
            Err(_) => ToolOutput::err("wl-copy is not installed"),
        };
    }
    res(sh("wl-paste", &["--no-newline"]).map(|t| cap_chars(&t, 4000)))
}

fn cap_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}\n...[{} more characters]", s.chars().take(n).collect::<String>(), s.chars().count() - n)
    }
}

fn system_info() -> ToolOutput {
    let mut out = vec![];
    if let Ok(t) = std::fs::read_to_string("/proc/meminfo") {
        let get = |k: &str| t.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        out.push(format!("RAM: {} / {} MB used", (get("MemTotal:") - get("MemAvailable:")) / 1024, get("MemTotal:") / 1024));
    }
    if let Ok(t) = std::fs::read_to_string("/proc/loadavg") {
        out.push(format!("Load: {}", t.split_whitespace().next().unwrap_or("?")));
    }
    if let Ok(t) = std::fs::read_to_string("/proc/uptime") {
        out.push(format!("Uptime: {:.1} h", t.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 3600.0));
    }
    if let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") {
        for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("BAT")) {
            let cap = std::fs::read_to_string(e.path().join("capacity")).unwrap_or_default();
            let st = std::fs::read_to_string(e.path().join("status")).unwrap_or_default();
            out.push(format!("Battery: {}% ({})", cap.trim(), st.trim()));
        }
    }
    if let Ok(t) = sh("bash", &["-c", "ps -eo comm,%cpu,%mem --sort=-%cpu | head -6; df -h / /home | tail -n +1"]) {
        out.push(t);
    }
    ToolOutput::ok(out.join("\n"))
}

fn fetch_url(input: &Value) -> ToolOutput {
    let Some(url) = input["url"].as_str() else { return ToolOutput::err("missing url") };
    let resp = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(15)).build().get(url).set("User-Agent", "oxide-ai/1.0").call();
    let body = match resp {
        Ok(r) => {
            let mut b = String::new();
            use std::io::Read;
            let _ = r.into_reader().take(2_000_000).read_to_string(&mut b);
            b
        }
        Err(e) => return ToolOutput::err(format!("could not fetch: {e}")),
    };
    ToolOutput::ok(cap_chars(&strip_html(&body), 6000))
}

/// Very small HTML to text: drop script/style, tags and extra whitespace.
fn strip_html(h: &str) -> String {
    let mut out = String::with_capacity(h.len() / 2);
    let lower = h.to_lowercase();
    let (mut i, bytes) = (0, h.as_bytes());
    while i < h.len() {
        if bytes[i] == b'<' {
            let rest = &lower[i..];
            let skip_to = ["script", "style", "noscript"].iter().find(|t| rest.starts_with(&format!("<{t}"))).and_then(|t| rest.find(&format!("</{t}>")).map(|e| e + t.len() + 3));
            if let Some(n) = skip_to {
                i += n;
                continue;
            }
            match h[i..].find('>') {
                Some(n) => {
                    out.push(' ');
                    i += n + 1;
                    continue;
                }
                None => break,
            }
        }
        let ch = h[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn reminder(input: &Value) -> ToolOutput {
    let (Some(secs), Some(msg)) = (input["seconds"].as_u64(), input["message"].as_str()) else { return ToolOutput::err("seconds and message are required") };
    let script = format!("sleep {} && notify-send Reminder {}", secs.min(86_400), shell_quote(msg));
    let _ = Command::new("setsid").args(["bash", "-c", &script]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
    ToolOutput::ok(format!("reminder set for {secs}s from now"))
}

pub fn execute(name: &str, input: &Value) -> ToolOutput {
    match name {
        "open_app" => open_app(input),
        "volume" => volume(input),
        "brightness" => brightness(input),
        "media" => res(sh("playerctl", &[input["action"].as_str().unwrap_or("status")])),
        "clipboard" => clipboard(input),
        "system_info" => system_info(),
        "fetch_url" => fetch_url(input),
        "reminder" => reminder(input),
        "wait" => {
            std::thread::sleep(std::time::Duration::from_secs_f64(input["seconds"].as_f64().unwrap_or(1.0).clamp(0.0, 10.0)));
            ToolOutput::ok("ok")
        }
        "remember" => ToolOutput::ok(if crate::memory::add(input["fact"].as_str().unwrap_or("")) { "remembered" } else { "not stored (duplicate or too short)" }),
        "forget" => ToolOutput::ok(format!("{} fact(s) forgotten", crate::memory::forget(input["text"].as_str().unwrap_or("")))),
        "shell" => shell(input),
        "oxide_dispatch" => dispatch(input),
        "oxide_state" => state(),
        "oxide_config" => config(input),
        "screenshot" => screenshot(),
        "input" => input_tool(input),
        other => ToolOutput::err(format!("unknown tool {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_replaces_leaves_and_keeps_siblings() {
        let mut v = json!({"general": {"gaps_in": 7, "gaps_out": 3}, "x": [1, 2]});
        merge(&mut v, &json!({"general": {"gaps_in": 12}, "x": [9]}));
        assert_eq!(v, json!({"general": {"gaps_in": 12, "gaps_out": 3}, "x": [9]}));
    }

    #[test]
    fn shell_runs_and_reports_exit_code() {
        let o = shell(&json!({"command": "echo hi; exit 3"}));
        assert!(!o.ok && o.text.contains("hi") && o.text.contains("exit code 3"));
        let o = shell(&json!({"command": "echo fine"}));
        assert!(o.ok);
    }

    #[test]
    fn tool_definitions_respect_permissions() {
        let mut c = AiConfig::default();
        c.allow_shell = false;
        c.allow_input = false;
        let names: Vec<String> = definitions(&c).iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert!(!names.contains(&"shell".to_string()) && !names.contains(&"input".to_string()));
        assert!(names.contains(&"oxide_state".to_string()));
    }
}
