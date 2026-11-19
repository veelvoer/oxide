//! oxide-remoted: lets the Oxide Remote phone app see this PC's screen, read its numbers and control it.
//!
//!   oxide-remoted              run (Oxide starts it when Settings > Phone is on)
//!   oxide-remoted pair         show a code for pairing a phone (valid two minutes)
//!   oxide-remoted status       paired phones, as JSON
//!   oxide-remoted forget ID    unpair a phone
//!   oxide-remoted rename NAME  the name phones see
mod actions;
mod assistant;
mod capture;
mod reach;
mod server;
mod stats;
mod store;

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn control_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("oxide-remote.sock")
}

/// Commands from the settings app and the CLI, over a local socket only you can reach.
fn control(shared: server::Shared) {
    let p = control_path();
    let _ = std::fs::remove_file(&p);
    let Ok(l) = UnixListener::bind(&p) else { return };
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let mut r = BufReader::new(match c.try_clone() {
                Ok(c) => c,
                Err(_) => continue,
            });
            let mut line = String::new();
            if r.read_line(&mut line).is_err() {
                continue;
            }
            let mut w = c;
            let mut parts = line.trim().splitn(2, ' ');
            let answer = match (parts.next().unwrap_or(""), parts.next()) {
                ("pair", _) => {
                    let code = format!("{:06}", u32::from_be_bytes(oxide_remote_proto::random::<4>()) % 1_000_000);
                    let mut p = shared.pairing.lock().unwrap();
                    p.code = Some((code.clone(), Instant::now() + Duration::from_secs(120)));
                    p.failures = 0;
                    serde_json::json!({ "code": code, "valid": 120 }).to_string()
                }
                ("status", _) => {
                    let s = shared.store.lock().unwrap();
                    let waiting = shared.pairing.lock().unwrap().code.as_ref().filter(|(_, u)| Instant::now() < *u).map(|(c, _)| c.clone());
                    let reach = shared.reach.lock().map(|r| r.clone()).unwrap_or_default();
                    serde_json::json!({
                        "name": s.pc_name, "id": s.pc_id, "macs": store::macs(), "pairing": waiting, "reach": reach,
                        "phones": s.phones.iter().map(|p| serde_json::json!({"id": p.id, "name": p.name, "paired": p.paired, "last_seen": p.last_seen})).collect::<Vec<_>>(),
                    })
                    .to_string()
                }
                ("forget", Some(id)) => {
                    let mut s = shared.store.lock().unwrap();
                    s.phones.retain(|p| p.id != id.trim());
                    s.save();
                    "{\"ok\":true}".into()
                }
                ("rename", Some(name)) => {
                    let mut s = shared.store.lock().unwrap();
                    s.pc_name = name.trim().to_string();
                    s.save();
                    "{\"ok\":true}".into()
                }
                // stop cleanly: the router forgets the forwarded port
                ("quit", _) => {
                    reach::stop(&shared.reach);
                    let _ = writeln!(w, "{{\"ok\":true}}");
                    std::process::exit(0)
                }
                ("cancel", _) => {
                    shared.pairing.lock().unwrap().code = None;
                    "{\"ok\":true}".into()
                }
                _ => "{\"error\":\"unknown command\"}".into(),
            };
            let _ = writeln!(w, "{answer}");
        }
    });
}

fn ask(cmd: &str) -> ! {
    match UnixStream::connect(control_path()) {
        Ok(mut s) => {
            let _ = writeln!(s, "{cmd}");
            let mut line = String::new();
            let _ = BufReader::new(s).read_line(&mut line);
            print!("{line}");
            std::process::exit(0)
        }
        Err(_) => {
            eprintln!("oxide-remoted is not running (turn it on in Settings > Phone, or run oxide-remoted)");
            std::process::exit(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("pair") => ask("pair"),
        Some("status") => ask("status"),
        Some("cancel") => ask("cancel"),
        Some("forget") if args.len() > 1 => ask(&format!("forget {}", args[1])),
        Some("rename") if args.len() > 1 => ask(&format!("rename {}", args[1..].join(" "))),
        Some("-h" | "--help" | "help") | Some(_) => {
            eprintln!("usage: oxide-remoted [pair|status|cancel|forget ID|rename NAME]");
            std::process::exit(if args.is_empty() { 0 } else { 2 })
        }
        None => {}
    }
    // one daemon per user
    if UnixStream::connect(control_path()).is_ok() {
        eprintln!("oxide-remoted is already running");
        return;
    }
    let shared = server::Shared {
        store: Arc::new(Mutex::new(store::Store::load())),
        pairing: Arc::new(Mutex::new(server::Pairing::default())),
        sampler: stats::Sampler::default(),
        reach: Default::default(),
        remote_handshakes: Default::default(),
    };
    reach::start(shared.reach.clone(), shared.store.clone());
    shared.sampler.start();
    control(shared.clone());
    server::discovery(shared.clone());
    if let Err(e) = server::listen(shared) {
        eprintln!("oxide-remoted: cannot listen on port {}: {e}", oxide_remote_proto::TCP_PORT);
        std::process::exit(1);
    }
}
