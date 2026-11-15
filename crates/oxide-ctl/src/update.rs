//! `oxide-ctl update`: keep Oxide up to date from its git repository.
//!
//! A private clone lives in `~/.local/share/oxide/src`; a new version is built there at low priority and
//! installed with `contrib/install.sh`. What changed is written to `~/.local/state/oxide/whats-new.json`, which
//! the compositor shows once at the next login. A build that is not part of the repository's history (local work)
//! is never replaced.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};

fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("oxide")
}

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("oxide")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The commit the installed binaries were built from (written by install.sh), if known.
pub fn installed() -> Option<String> {
    let v = std::fs::read_to_string(data_dir().join("version")).ok()?;
    let v = v.trim();
    let hash = v.strip_suffix("-dirty").unwrap_or(v);
    (hash.len() >= 7 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| v.to_string())
}

fn cargo() -> Option<PathBuf> {
    let home_dir = std::env::var_os("CARGO_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cargo"));
    let local = home_dir.join("bin/cargo");
    if local.exists() {
        return Some(local);
    }
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| PathBuf::from(d).join("cargo")).find(|p| p.exists())
}

fn git(dir: Option<&PathBuf>, args: &[&str]) -> Result<String, String> {
    let mut c = Command::new("git");
    if let Some(d) = dir {
        c.arg("-C").arg(d);
    }
    let out = c.args(args).stdin(Stdio::null()).output().map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or("git failed").to_string())
    }
}

fn status_path() -> PathBuf {
    state_dir().join("update.json")
}

fn read_status() -> Value {
    std::fs::read_to_string(status_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}))
}

fn write_json(path: PathBuf, v: &Value) {
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&state_dir()));
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

fn set_status(f: impl FnOnce(&mut serde_json::Map<String, Value>)) {
    let mut s = read_status();
    if !s.is_object() {
        s = json!({});
    }
    f(s.as_object_mut().unwrap());
    write_json(status_path(), &s);
}

/// The newest commit on the configured branch, asked from the remote without downloading anything.
fn latest(repo: &str, branch: &str) -> Result<String, String> {
    let out = git(None, &["ls-remote", repo, &format!("refs/heads/{branch}")])?;
    out.split_whitespace().next().map(str::to_string).ok_or_else(|| format!("branch {branch} not found"))
}

/// Ok(Some(commit)) when a newer version exists.
pub fn check() -> Result<Option<String>, String> {
    let cfg = oxide_config::Config::load().unwrap_or_default().update;
    let head = latest(&cfg.repo, &cfg.branch)?;
    let have = installed();
    set_status(|s| {
        s.insert("checked".into(), json!(now()));
        s.insert("latest".into(), json!(head));
        s.insert("installed".into(), json!(have));
        s.remove("error");
    });
    // a build with local changes is never replaced, so there is nothing to offer for it
    if have.as_deref().is_some_and(|h| h.ends_with("-dirty")) {
        return Ok(None);
    }
    Ok((have.as_deref() != Some(head.as_str())).then_some(head))
}

/// Single run at a time.
struct Lock(PathBuf);

impl Lock {
    fn take() -> Option<Lock> {
        let p = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("oxide-update.lock");
        if let Ok(pid) = std::fs::read_to_string(&p) {
            if PathBuf::from(format!("/proc/{}", pid.trim())).exists() {
                return None;
            }
        }
        std::fs::write(&p, std::process::id().to_string()).ok()?;
        Some(Lock(p))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Fetch, build and install the newest version. Returns the installed commit.
pub fn run() -> Result<Option<String>, String> {
    let Some(_lock) = Lock::take() else { return Err("an update is already running".into()) };
    let cfg = oxide_config::Config::load().unwrap_or_default().update;
    let src = data_dir().join("src");
    if !src.join(".git").exists() {
        let _ = std::fs::create_dir_all(data_dir());
        git(None, &["clone", "--quiet", "--branch", &cfg.branch, &cfg.repo, src.to_str().unwrap_or_default()])?;
    } else {
        git(Some(&src), &["remote", "set-url", "origin", &cfg.repo])?;
        git(Some(&src), &["fetch", "--quiet", "origin", &cfg.branch])?;
    }
    let target = format!("origin/{}", cfg.branch);
    let head = git(Some(&src), &["rev-parse", &target])?;
    let have = installed();
    if have.as_deref() == Some(head.as_str()) {
        set_status(|s| {
            s.insert("checked".into(), json!(now()));
            s.insert("latest".into(), json!(head));
        });
        return Ok(None);
    }
    // never replace a build that is not part of this history (local work, another fork)
    if have.as_deref().is_some_and(|h| h.ends_with("-dirty")) {
        return Err("the installed build has local changes; not replacing it".into());
    }
    if let Some(h) = &have {
        if git(Some(&src), &["merge-base", "--is-ancestor", h, &head]).is_err() {
            return Err("the installed build is not from this repository (local changes?); not replacing it".into());
        }
    }
    git(Some(&src), &["reset", "--quiet", "--hard", &head])?;
    set_status(|s| {
        s.insert("running".into(), json!(true));
        s.insert("started".into(), json!(now()));
    });
    let cargo = cargo().ok_or("cargo is not installed")?;
    let log_path = state_dir().join("update.log");
    let _ = std::fs::create_dir_all(state_dir());
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let target_dir = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cache")).join("oxide/build");
    let mut path = cargo.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if let Ok(p) = std::env::var("PATH") {
        path = format!("{path}:{p}");
    }
    // the build takes a while: lowest CPU and disk priority so the desktop stays smooth
    let ok = Command::new("nice")
        .args(["-n", "19", "ionice", "-c", "3", "bash", "contrib/install.sh"])
        .current_dir(&src)
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("PATH", &path)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    set_status(|s| {
        s.insert("running".into(), json!(false));
    });
    if !ok {
        return Err(format!("the build failed, see {}", log_path.display()));
    }
    std::fs::write(data_dir().join("version"), format!("{head}\n")).map_err(|e| e.to_string())?;
    // what changed, for the note at the next login
    let range = match &have {
        Some(h) => format!("{h}..{head}"),
        None => format!("{head}~10..{head}"),
    };
    let log = git(Some(&src), &["log", "--no-merges", "--format=%s%n%b%x1e", &range]).unwrap_or_default();
    let mut changes: Vec<String> = vec![];
    for entry in log.split('\u{1e}') {
        let mut lines = entry.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with("Co-Authored-By"));
        let Some(subject) = lines.next().map(str::trim) else { continue };
        // "* item" lines, with their wrapped continuation lines joined back on
        let mut bullets: Vec<String> = vec![];
        for l in lines {
            let t = l.trim();
            if let Some(b) = t.strip_prefix("* ").or_else(|| t.strip_prefix("- ")) {
                bullets.push(b.to_string());
            } else if let Some(last) = bullets.last_mut().filter(|_| l.starts_with(' ')) {
                last.push(' ');
                last.push_str(t);
            }
        }
        if bullets.is_empty() {
            changes.push(subject.to_string());
        } else {
            changes.extend(bullets);
        }
    }
    let version = git(Some(&src), &["describe", "--tags", "--always", &head]).unwrap_or_else(|_| head[..7].to_string());
    // notes that were not shown yet are kept: two updates before a login show both
    let old = std::fs::read_to_string(state_dir().join("whats-new.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    if let Some(o) = old.filter(|o| o["shown"] == json!(false)) {
        if let Some(prev) = o["changes"].as_array() {
            changes.extend(prev.iter().filter_map(|c| c.as_str().map(str::to_string)));
        }
    }
    write_json(
        state_dir().join("whats-new.json"),
        &json!({ "version": version, "from": have, "to": head, "at": now(), "changes": changes, "shown": false }),
    );
    set_status(|s| {
        s.insert("installed".into(), json!(head));
        s.insert("latest".into(), json!(head));
        s.insert("updated".into(), json!(now()));
        s.remove("error");
    });
    Ok(Some(head))
}

pub fn cmd(args: &[String]) -> ! {
    let r = match args.first().map(String::as_str) {
        Some("check") => check().map(|n| match n {
            Some(c) => println!("a new version is available: {}", &c[..7.min(c.len())]),
            None if installed().is_some_and(|h| h.ends_with("-dirty")) => println!("this build has local changes; it is not updated automatically"),
            None => println!("Oxide is up to date"),
        }),
        // what the compositor runs now and then: only when automatic updates are on
        Some("auto") => {
            if !oxide_config::Config::load().unwrap_or_default().update.auto {
                std::process::exit(0)
            }
            match check() {
                Ok(Some(_)) => run().map(|_| ()),
                Ok(None) => Ok(()),
                Err(e) => Err(e),
            }
        }
        Some("seen") => {
            // the compositor forgets the note and marks it read; without one, mark the file directly
            if matches!(oxide_ipc::call(&oxide_ipc::Request::WhatsNewSeen), Ok(oxide_ipc::Response::Ok)) {
                std::process::exit(0)
            }
            let p = state_dir().join("whats-new.json");
            if let Some(mut v) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                v["shown"] = json!(true);
                write_json(p, &v);
            }
            Ok(())
        }
        Some("status") => {
            let mut s = read_status();
            s["installed"] = json!(installed());
            println!("{}", serde_json::to_string_pretty(&s).unwrap_or_default());
            Ok(())
        }
        None | Some("run") => run().map(|n| match n {
            Some(c) => println!("updated to {}; log out and back in to use it", &c[..7.min(c.len())]),
            None => println!("Oxide is up to date"),
        }),
        _ => Err("usage: oxide-ctl update [run|check|auto|status|seen]".into()),
    };
    if let Err(e) = r {
        let busy = e.contains("already running");
        set_status(|s| {
            if busy {
                return;
            }
            s.insert("error".into(), json!(e));
            s.insert("running".into(), json!(false));
        });
        eprintln!("update: {e}");
        std::process::exit(1)
    }
    std::process::exit(0)
}
