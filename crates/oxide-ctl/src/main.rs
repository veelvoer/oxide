mod clip;
mod tools;
use oxide_config::Action;
use oxide_ipc::{call, Request, Response};

fn usage() -> ! {
    eprintln!("usage: oxide-ctl <input ...|restart-shell|subscribe|get-config|reload|unlock|lock|state|set <config.toml>|import-hyprland [file] [--write]|dispatch <json-action>|exec <cmd>|workspace <n>|rule <app> k=v..|terminal [cmd]|files|ipc <target> <fn>|doctor|default-config|quit>");
    std::process::exit(2)
}

fn import_hyprland(args: &[String]) -> ! {
    let write = args.iter().any(|a| a == "--write");
    let path = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| {
        format!("{}/.config/hypr/hyprland.conf", std::env::var("HOME").unwrap_or_default())
    });
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1)
    });
    let imported = oxide_config::hyprland::import(&text);
    for s in &imported.skipped {
        eprintln!("skipped: {s}");
    }
    let toml = imported.config.to_toml().expect("serialize");
    if write {
        imported.config.save().expect("save config");
        eprintln!("written to {}", oxide_config::Config::path().display());
    } else {
        println!("{toml}");
    }
    std::process::exit(0)
}

/// `oxide-ctl input move X Y | click X Y [button] | dclick X Y | scroll DX DY | key NAME [mods..] | type TEXT`
fn input_cmd(args: &[String]) -> Option<oxide_ipc::InputCmd> {
    use oxide_ipc::InputCmd as I;
    let f = |i: usize| args.get(i)?.parse::<f64>().ok();
    Some(match args.first()?.as_str() {
        "move" => I::Move { x: f(1)?, y: f(2)? },
        "click" => I::Click { x: f(1)?, y: f(2)?, button: args.get(3).cloned().unwrap_or_else(|| "left".into()) },
        "button" => I::Button { button: args.get(1)?.clone(), pressed: args.get(2)? == "down" },
        "dclick" => I::DoubleClick { x: f(1)?, y: f(2)? },
        "scroll" => I::Scroll { dx: f(1)?, dy: f(2)? },
        "key" => I::Key { name: args.get(1)?.clone(), mods: args[2..].to_vec() },
        "type" => I::Type { text: args[1..].join(" ") },
        _ => return None,
    })
}

fn subscribe() -> ! {
    use std::io::{BufRead, BufReader, Write};
    let mut s = std::os::unix::net::UnixStream::connect(oxide_ipc::socket_path()).unwrap_or_else(|e| {
        eprintln!("cannot reach compositor: {e}");
        std::process::exit(1)
    });
    s.write_all(b"{\"cmd\":\"subscribe\"}\n").ok();
    for line in BufReader::new(s).lines().map_while(Result::ok) {
        println!("{line}");
    }
    std::process::exit(0)
}

/// Lock the screen with the shell's lock screen; fall back to another locker if the shell is not running.
fn lock() -> ! {
    let link = oxide_config::active_shell_link();
    let ok = std::process::Command::new("quickshell")
        .arg("-p").arg(&link).args(["ipc", "call", "shell", "lock"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        for (bin, args) in [("hyprlock", vec![]), ("swaylock", vec![]), ("loginctl", vec!["lock-session"])] {
            if std::process::Command::new(bin).args(&args).spawn().is_ok() {
                break;
            }
        }
    }
    std::process::exit(0)
}

/// Toggle the shell's launcher; fall back to rofi/wofi if the shell is not running.
fn launcher() -> ! {
    let link = oxide_config::active_shell_link();
    let ok = std::process::Command::new("quickshell")
        .arg("-p").arg(&link).args(["ipc", "call", "shell", "toggleLauncher"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        for (bin, args) in [("rofi", vec!["-show", "drun"]), ("wofi", vec!["--show", "drun"])] {
            if std::process::Command::new(bin).args(&args).spawn().is_ok() {
                break;
            }
        }
    }
    std::process::exit(0)
}

/// `oxide-ctl theme list | apply <preset> [--no-kitty]`
fn theme_cmd(args: &[String]) -> ! {
    use oxide_config::{theme, Config};
    match args.first().map(String::as_str) {
        Some("list") => {
            for p in theme::presets() {
                println!("{}", p.name);
            }
        }
        Some("apply") if args.len() >= 2 => {
            let mut cfg = Config::load().unwrap_or_default();
            if !theme::presets().iter().any(|p| p.name == args[1]) {
                eprintln!("unknown theme {}; try `oxide-ctl theme list`", args[1]);
                std::process::exit(1)
            }
            cfg.theme.preset = args[1].clone();
            cfg.theme.accent = None;
            cfg.save().expect("save config");
            theme::write_all(&cfg.theme).expect("write theme");
            let _ = call(&Request::Reload);
        }
        _ => usage(),
    }
    std::process::exit(0)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(String::as_str) == Some("subscribe") {
        subscribe();
    }
    if a.first().map(String::as_str) == Some("input") {
        let Some(input) = input_cmd(&a[1..]) else { usage() };
        match call(&Request::Input { input }) {
            Ok(Response::Error { message }) => {
                eprintln!("error: {message}");
                std::process::exit(1)
            }
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("cannot reach compositor: {e}");
                std::process::exit(1)
            }
        }
    }
    if a.first().map(String::as_str) == Some("import-hyprland") {
        import_hyprland(&a[1..]);
    }
    match a.first().map(String::as_str) {
        Some("launcher") => launcher(),
        Some("terminal") => tools::terminal(&a[1..]),
        Some("files") => tools::files(),
        Some("ipc") if a.len() >= 3 => tools::ipc(&a[1..]),
        Some("doctor") => tools::doctor(),
        // the complete default configuration (what a fresh install starts with), as TOML
        Some("default-config") => {
            print!("{}", oxide_config::Config::default().to_toml().unwrap_or_default());
            std::process::exit(0)
        }
        Some("lock") => lock(),
        Some("unlock") => {
            // forget the "locked" markers too, or the shell would lock again when it restarts
            if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("oxide-locked")) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        Some("clip") => clip::run(&a[1..]),
        Some("theme") => theme_cmd(&a[1..]),
        _ => {}
    }
    let req = match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["get-config"] => Request::GetConfig,
        ["reload"] => Request::Reload,
        ["state"] => Request::GetState,
        ["restart-shell"] => Request::RestartShell,
        ["quit"] => Request::Dispatch { action: Action::Quit },
        ["unlock"] => Request::ForceUnlock,
        ["record", dir, n] => Request::Record { dir: dir.to_string(), frames: n.parse().unwrap_or(60) },
        ["workspace", n] => Request::Dispatch { action: Action::Workspace(n.parse().unwrap_or_else(|_| usage())) },
        ["exec", rest @ ..] if !rest.is_empty() => Request::Dispatch { action: Action::Exec(rest.join(" ")) },
        ["set", file] => {
            let text = std::fs::read_to_string(file).unwrap_or_else(|e| {
                eprintln!("cannot read {file}: {e}");
                std::process::exit(1)
            });
            let config = oxide_config::Config::from_toml(&text).unwrap_or_else(|e| {
                eprintln!("invalid config: {e}");
                std::process::exit(1)
            });
            Request::SetConfig { config: Box::new(config) }
        }
        // oxide-ctl rule APP floating=true workspace=2 opacity=0.9 blur=false   ("-" removes a setting)
        ["rule", app, fields @ ..] if !fields.is_empty() => {
            let mut rule = oxide_config::WindowRule::default();
            let mut clear = vec![];
            for f in fields {
                let (k, v) = f.split_once('=').unwrap_or_else(|| usage());
                if v == "-" {
                    clear.push(k.to_string());
                    continue;
                }
                match k {
                    "floating" => rule.floating = Some(v == "true"),
                    "blur" => rule.blur = Some(v == "true"),
                    "workspace" => rule.workspace = Some(v.parse().unwrap_or_else(|_| usage())),
                    "opacity" => rule.opacity = Some(v.parse().unwrap_or_else(|_| usage())),
                    _ => usage(),
                }
            }
            Request::SetRule { app_id: app.to_string(), rule, clear }
        }
        ["dispatch", j] => Request::Dispatch { action: serde_json::from_str(j).unwrap_or_else(|_| usage()) },
        _ => usage(),
    };
    match call(&req) {
        Ok(Response::Error { message }) => {
            eprintln!("error: {message}");
            std::process::exit(1)
        }
        Ok(r) => println!("{}", serde_json::to_string_pretty(&r).unwrap()),
        Err(e) => {
            eprintln!("cannot reach compositor: {e}");
            std::process::exit(1)
        }
    }
}
