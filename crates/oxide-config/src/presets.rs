//! Presets: the whole configuration under a name. A preset holds every settings file Oxide has (oxide.toml, the bar, the
//! launcher, the wallpaper, the assistant's settings, the look of Settings), so applying one brings the desktop back
//! exactly as it was. Secrets are never part of it: the assistant's API key stays out (it is blanked in the copy and
//! kept as it is on this computer when a preset is applied), and so do the phone pairing keys and the chat history.
//!
//! The same data goes to GitHub when you sign in (see `sync.rs`): your presets and your current configuration, in a
//! private gist of your own.
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

/// The settings files a preset holds (next to oxide.toml).
pub const FILES: [&str; 6] = ["oxide.toml", "bar.json", "launcher.json", "wallpaper.json", "ui.toml", "ai.toml"];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub name: String,
    /// seconds since 1970
    pub created: u64,
    /// file name -> contents
    pub files: BTreeMap<String, String>,
}

fn config_dir() -> PathBuf {
    crate::Config::path().parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

pub fn dir() -> PathBuf {
    config_dir().join("presets")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A name as a file name.
fn slug(name: &str) -> String {
    let s: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "preset".into()
    } else {
        s
    }
}

fn path_of(name: &str) -> PathBuf {
    dir().join(format!("{}.json", slug(name)))
}

/// The assistant's settings without its key (and without anything else that looks like a secret).
pub fn strip_secrets(file: &str, text: &str) -> String {
    if file != "ai.toml" {
        return text.to_string();
    }
    let Ok(mut v) = text.parse::<toml::Value>() else { return String::new() };
    if let Some(t) = v.as_table_mut() {
        for k in t.keys().cloned().collect::<Vec<_>>() {
            let l = k.to_lowercase();
            if l == "key" || l.ends_with("_key") || l == "token" || l.ends_with("_token") || l.contains("secret") || l.contains("password") {
                t.insert(k, toml::Value::String(String::new()));
            }
        }
    }
    toml::to_string_pretty(&v).unwrap_or_default()
}

/// The configuration as it is now.
pub fn capture(name: &str) -> Preset {
    let mut files = BTreeMap::new();
    for f in FILES {
        if let Ok(t) = std::fs::read_to_string(config_dir().join(f)) {
            files.insert(f.to_string(), strip_secrets(f, &t));
        }
    }
    Preset { name: name.trim().to_string(), created: now(), files }
}

pub fn save(p: &Preset) -> std::io::Result<()> {
    std::fs::create_dir_all(dir())?;
    let tmp = dir().join(format!(".{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(p).map_err(std::io::Error::other)?)?;
    std::fs::rename(tmp, path_of(&p.name))
}

pub fn load(name: &str) -> Option<Preset> {
    std::fs::read_to_string(path_of(name)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

/// All presets, newest first. The automatic "Before ..." copy made when one is applied is among them.
pub fn list() -> Vec<Preset> {
    let mut v: Vec<Preset> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str::<Preset>(&t).ok())
        .collect();
    v.sort_by(|a, b| b.created.cmp(&a.created).then(a.name.cmp(&b.name)));
    v
}

pub fn delete(name: &str) {
    let _ = std::fs::remove_file(path_of(name));
}

pub const UNDO: &str = "Before the last preset";

/// Make the configuration what the preset holds. What was there is kept as "Before the last preset" (an undo). The
/// assistant's key on this computer stays.
pub fn apply(p: &Preset) -> std::io::Result<()> {
    let mut undo = capture(UNDO);
    undo.created = now();
    save(&undo)?;
    let own_key = crate::ai::AiConfig::load().api_key;
    for (f, text) in &p.files {
        if !FILES.contains(&f.as_str()) {
            continue;
        }
        let mut text = text.clone();
        if f == "ai.toml" {
            // the key never travels: put the one of this computer back
            if let Ok(mut v) = text.parse::<toml::Value>() {
                if let Some(t) = v.as_table_mut() {
                    t.insert("api_key".into(), toml::Value::String(own_key.clone()));
                }
                text = toml::to_string_pretty(&v).unwrap_or(text);
            }
        }
        let path = config_dir().join(f);
        let tmp = config_dir().join(format!(".{f}.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)?;
    }
    Ok(())
}

/// Whether the preset equals what is configured now (to mark the active one).
pub fn is_current(p: &Preset) -> bool {
    let now = capture("");
    p.files.iter().all(|(k, v)| now.files.get(k).is_some_and(|n| n == v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_never_in_a_preset() {
        let t = "api_key = \"sk-secret\"\nname = \"Oxide\"\nmax_tokens = 4000\n";
        let s = strip_secrets("ai.toml", t);
        assert!(!s.contains("sk-secret"));
        assert!(s.contains("max_tokens = 4000"));
        assert!(s.contains("name = \"Oxide\""));
    }

    #[test]
    fn names_make_file_names() {
        assert_eq!(slug("  my/preset: 1 "), "my_preset_ 1");
        assert_eq!(slug("///"), "___");
        assert_eq!(slug("  "), "preset");
    }
}
