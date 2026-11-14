//! Clipboard history: `oxide-ctl clip watch|add|add-image|list|copy|delete|pin|clear`.
//! Entries are plain files in ~/.local/share/oxide/clipboard (`<ms>.txt` or `<ms>.png`, plus `<ms>.pin` when pinned).
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const MAX_TEXT: usize = 1 << 20;
const MAX_IMAGE: usize = 12 << 20;
const KEEP_TEXT: usize = 200;
const KEEP_IMAGES: usize = 30;

fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("oxide/clipboard")
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

struct Entry {
    id: String,
    image: bool,
    pinned: bool,
}

fn entries() -> Vec<Entry> {
    let mut v: Vec<Entry> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?;
            if ext != "txt" && ext != "png" {
                return None;
            }
            let id = p.file_stem()?.to_str()?.to_string();
            Some(Entry { pinned: p.with_extension("pin").exists(), image: ext == "png", id })
        })
        .collect();
    // newest first (ids are millisecond timestamps of equal width for a few hundred years)
    v.sort_by(|a, b| b.id.len().cmp(&a.id.len()).then(b.id.cmp(&a.id)));
    v
}

fn file(e: &Entry) -> PathBuf {
    dir().join(format!("{}.{}", e.id, if e.image { "png" } else { "txt" }))
}

fn secret_offered() -> bool {
    Command::new("wl-paste")
        .arg("--list-types")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("x-kde-passwordManagerHint"))
        .unwrap_or(false)
}

fn remove(e: &Entry) {
    let _ = std::fs::remove_file(file(e));
    let _ = std::fs::remove_file(dir().join(format!("{}.pin", e.id)));
}

fn add(image: bool) {
    let mut data = Vec::new();
    let limit = if image { MAX_IMAGE } else { MAX_TEXT };
    let _ = std::io::stdin().take(limit as u64 + 1).read_to_end(&mut data);
    if data.is_empty() || data.len() > limit {
        return;
    }
    if !image && String::from_utf8_lossy(&data).trim().is_empty() {
        return;
    }
    if secret_offered() {
        return;
    }
    let _ = std::fs::create_dir_all(dir());
    let all = entries();
    // an identical entry moves to the top instead of being stored twice (a pin is kept)
    for e in all.iter().filter(|e| e.image == image) {
        if std::fs::read(file(e)).ok().as_deref() == Some(&data) {
            let pinned = e.pinned;
            remove(e);
            write_new(&data, image, pinned);
            return;
        }
    }
    write_new(&data, image, false);
    // trim old, unpinned entries
    for (kind, keep) in [(false, KEEP_TEXT), (true, KEEP_IMAGES)] {
        for e in entries().iter().filter(|e| e.image == kind && !e.pinned).skip(keep) {
            remove(e);
        }
    }
}

fn write_new(data: &[u8], image: bool, pinned: bool) {
    let id = now_ms().to_string();
    let ext = if image { "png" } else { "txt" };
    let _ = std::fs::write(dir().join(format!("{id}.{ext}")), data);
    if pinned {
        let _ = std::fs::write(dir().join(format!("{id}.pin")), b"");
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn list() {
    let mut all = entries();
    all.sort_by_key(|e| !e.pinned); // stable: pinned first, newest first within
    let items: Vec<String> = all
        .iter()
        .map(|e| {
            let p = file(e);
            let (preview, size) = if e.image {
                (String::new(), std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0))
            } else {
                let t = std::fs::read_to_string(&p).unwrap_or_default();
                (t.chars().take(400).collect::<String>(), t.len() as u64)
            };
            format!(
                "{{\"id\":{},\"kind\":\"{}\",\"pinned\":{},\"preview\":{},\"size\":{},\"path\":{},\"time\":{}}}",
                json_str(&e.id),
                if e.image { "image" } else { "text" },
                e.pinned,
                json_str(&preview),
                size,
                json_str(&p.to_string_lossy()),
                e.id
            )
        })
        .collect();
    println!("[{}]", items.join(","));
}

fn find(id: &str) -> Option<Entry> {
    entries().into_iter().find(|e| e.id == id)
}

fn copy(id: &str) {
    let Some(e) = find(id) else { return };
    let Ok(f) = std::fs::File::open(file(&e)) else { return };
    let mut c = Command::new("wl-copy");
    if e.image {
        c.args(["--type", "image/png"]);
    }
    // wl-copy daemonizes itself and keeps serving the selection
    let _ = c.stdin(f).stdout(Stdio::null()).stderr(Stdio::null()).status();
    // moving it to the top is the watcher's job: it sees the new selection and re-adds it
}

/// Run the two `wl-paste --watch` helpers and restart them if they stop.
fn watch() -> ! {
    let me = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "oxide-ctl".into());
    let spawn = |kind: &str, sub: &str| {
        Command::new("wl-paste")
            .args(["--type", kind, "--watch", &me, "clip", sub])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    };
    let parent = std::os::unix::process::parent_id();
    let mut text = spawn("text", "add");
    let mut image = spawn("image/png", "add-image");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3));
        // stop with whoever started us (the shell), so restarts do not pile up watchers
        if std::os::unix::process::parent_id() != parent {
            for c in [&mut text, &mut image].into_iter().flatten() {
                let _ = c.kill();
            }
            std::process::exit(0);
        }
        for (slot, kind, sub) in [(&mut text, "text", "add"), (&mut image, "image/png", "add-image")] {
            let dead = match slot {
                Some(c) => c.try_wait().map(|s| s.is_some()).unwrap_or(true),
                None => true,
            };
            if dead {
                *slot = spawn(kind, sub);
            }
        }
    }
}

pub fn run(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        Some("watch") => watch(),
        Some("add") => add(false),
        Some("add-image") => add(true),
        Some("list") => list(),
        // newest text entry, as it was copied (no added newline)
        Some("latest") => {
            if let Some(e) = entries().iter().find(|e| !e.image) {
                if let Ok(t) = std::fs::read_to_string(file(e)) {
                    print!("{t}");
                }
            }
        }
        Some("copy") if args.len() > 1 => copy(&args[1]),
        Some("delete") if args.len() > 1 => {
            if let Some(e) = find(&args[1]) {
                remove(&e);
            }
        }
        Some("pin") if args.len() > 1 => {
            if let Some(e) = find(&args[1]) {
                let p = dir().join(format!("{}.pin", e.id));
                if e.pinned {
                    let _ = std::fs::remove_file(p);
                } else {
                    let _ = std::fs::write(p, b"");
                }
            }
        }
        Some("clear") => {
            for e in entries().iter().filter(|e| !e.pinned) {
                remove(e);
            }
        }
        _ => {
            eprintln!("usage: oxide-ctl clip <watch|add|add-image|list|latest|copy ID|delete ID|pin ID|clear>");
            std::process::exit(2)
        }
    }
    std::process::exit(0)
}
