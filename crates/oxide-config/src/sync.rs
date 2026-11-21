//! Sign in with GitHub and keep your presets and configuration in a private gist of your own.
//!
//! Signing in works through GitHub's *device flow* (an OAuth app of yours with "Enable Device Flow" ticked; its client id
//! goes in Settings > Presets), or with the GitHub CLI when it is installed and signed in (`gh auth token`). The only
//! permission asked is `gist`. Nothing secret is sent: the assistant's API key is never in a preset (see presets.rs).
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

use crate::presets::{self, Preset};

const DESCRIPTION: &str = "Oxide configuration";
const FILE: &str = "oxide-sync.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Account {
    pub token: String,
    pub login: String,
    /// the gist that holds the data
    pub gist: String,
    /// the OAuth app's client id (device flow)
    pub client_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bundle {
    pub version: u32,
    /// the configuration at the moment of the upload
    pub config: Preset,
    pub presets: Vec<Preset>,
}

pub fn account_path() -> PathBuf {
    crate::Config::path().with_file_name("github.json")
}

pub fn load_account() -> Account {
    std::fs::read_to_string(account_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_account(a: &Account) -> std::io::Result<()> {
    let p = account_path();
    std::fs::write(&p, serde_json::to_string_pretty(a).map_err(std::io::Error::other)?)?;
    // the token is a password: only for you
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn sign_out() {
    let a = load_account();
    // the client id stays: it is not secret and signing in again needs it
    let _ = save_account(&Account { client_id: a.client_id, ..Default::default() });
}

/// One HTTPS request through curl (every Linux has it; no extra library). Returns the body of a successful answer.
fn http(method: &str, url: &str, token: Option<&str>, body: Option<&str>) -> Result<String, String> {
    let mut c = Command::new("curl");
    c.args(["-sS", "--max-time", "30", "-X", method, "-H", "Accept: application/vnd.github+json", "-H", "User-Agent: oxide-settings", "-w", "\n%{http_code}"]);
    if let Some(t) = token {
        c.args(["-H", &format!("Authorization: Bearer {t}")]);
    }
    if body.is_some() {
        c.args(["-H", "Content-Type: application/json", "--data-binary", "@-"]);
    }
    c.arg(url).stdin(if body.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("curl is needed to talk to GitHub: {e}"))?;
    if let (Some(b), Some(mut i)) = (body, child.stdin.take()) {
        let _ = i.write_all(b.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("No connection to GitHub: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or((&text, "0"));
    let code: u32 = code.trim().parse().unwrap_or(0);
    if (200..300).contains(&code) {
        Ok(body.to_string())
    } else {
        let msg = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["message"].as_str().map(str::to_string)).unwrap_or_else(|| body.chars().take(120).collect());
        Err(match code {
            401 => "GitHub does not accept the sign-in any more. Sign in again.".to_string(),
            403 | 429 => format!("GitHub says wait a moment ({msg})"),
            _ => format!("GitHub answered {code}: {msg}"),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub url: String,
    pub interval: u64,
    pub expires_in: u64,
}

/// Step 1 of the device flow: GitHub gives a short code to type at github.com/login/device.
pub fn device_start(client_id: &str) -> Result<DeviceCode, String> {
    if client_id.trim().is_empty() {
        return Err("Enter the client id of your GitHub OAuth app first (see below).".into());
    }
    let body = serde_json::json!({ "client_id": client_id.trim(), "scope": "gist" }).to_string();
    let r = http("POST", "https://github.com/login/device/code", None, Some(&body))?;
    let v: serde_json::Value = serde_json::from_str(&r).map_err(|_| "GitHub gave an answer that is not understood".to_string())?;
    if let Some(e) = v["error_description"].as_str() {
        return Err(e.to_string());
    }
    Ok(DeviceCode {
        device_code: v["device_code"].as_str().unwrap_or("").into(),
        user_code: v["user_code"].as_str().unwrap_or("").into(),
        url: v["verification_uri"].as_str().unwrap_or("https://github.com/login/device").into(),
        interval: v["interval"].as_u64().unwrap_or(5).max(5),
        expires_in: v["expires_in"].as_u64().unwrap_or(900),
    })
}

/// Step 2: has the code been entered? `Ok(None)` = not yet.
pub fn device_poll(client_id: &str, device_code: &str) -> Result<Option<String>, String> {
    let body = serde_json::json!({ "client_id": client_id.trim(), "device_code": device_code, "grant_type": "urn:ietf:params:oauth:grant-type:device_code" }).to_string();
    let r = http("POST", "https://github.com/login/oauth/access_token", None, Some(&body))?;
    let v: serde_json::Value = serde_json::from_str(&r).map_err(|_| "GitHub gave an answer that is not understood".to_string())?;
    if let Some(t) = v["access_token"].as_str() {
        return Ok(Some(t.to_string()));
    }
    match v["error"].as_str() {
        Some("authorization_pending") | Some("slow_down") => Ok(None),
        Some("expired_token") => Err("The code ran out. Start again.".into()),
        Some("access_denied") => Err("The sign-in was cancelled on GitHub.".into()),
        _ => Err(v["error_description"].as_str().unwrap_or("The sign-in failed").to_string()),
    }
}

/// The GitHub CLI's token, when `gh` is installed and signed in. It may ask the keyring, which can take a moment (or
/// never answer), so it is given 4 seconds.
pub fn token_from_gh() -> Option<String> {
    let mut child = Command::new("gh").args(["auth", "token"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let mut out = String::new();
                if let Some(mut o) = child.stdout.take() {
                    let _ = std::io::Read::read_to_string(&mut o, &mut out);
                }
                let t = out.trim().to_string();
                return (st.success() && !t.is_empty()).then_some(t);
            }
            Ok(None) if start.elapsed() < std::time::Duration::from_secs(4) => std::thread::sleep(std::time::Duration::from_millis(30)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

pub fn whoami(token: &str) -> Result<String, String> {
    let r = http("GET", "https://api.github.com/user", Some(token), None)?;
    let v: serde_json::Value = serde_json::from_str(&r).map_err(|_| "GitHub gave an answer that is not understood".to_string())?;
    v["login"].as_str().map(str::to_string).ok_or_else(|| "GitHub did not say who you are".into())
}

/// Sign in with a token and remember it.
pub fn sign_in(token: String) -> Result<Account, String> {
    let login = whoami(&token)?;
    let old = load_account();
    let a = Account { token, login, gist: String::new(), client_id: old.client_id };
    save_account(&a).map_err(|e| format!("Could not keep the sign-in: {e}"))?;
    Ok(a)
}

/// The gist of this account (found by its description; made when there is none).
fn find_gist(a: &Account) -> Result<Option<String>, String> {
    if !a.gist.is_empty() {
        return Ok(Some(a.gist.clone()));
    }
    let r = http("GET", "https://api.github.com/gists?per_page=100", Some(&a.token), None)?;
    let v: serde_json::Value = serde_json::from_str(&r).unwrap_or_default();
    Ok(v.as_array().and_then(|l| l.iter().find(|g| g["description"] == DESCRIPTION && g["files"].get(FILE).is_some())).and_then(|g| g["id"].as_str().map(str::to_string)))
}

/// Everything that is shared: the configuration now and all presets, without secrets.
pub fn bundle() -> Bundle {
    let mut presets = presets::list();
    for p in &mut presets {
        for (f, t) in p.files.iter_mut() {
            *t = presets::strip_secrets(f, t);
        }
    }
    Bundle { version: 1, config: presets::capture("Current configuration"), presets }
}

/// Put the bundle in the gist. Returns what happened, for a toast.
pub fn upload() -> Result<String, String> {
    let mut a = load_account();
    if a.token.is_empty() {
        return Err("Sign in with GitHub first.".into());
    }
    let b = bundle();
    let content = serde_json::to_string_pretty(&b).map_err(|e| e.to_string())?;
    let payload = serde_json::json!({ "description": DESCRIPTION, "public": false, "files": { FILE: { "content": content } } }).to_string();
    let n = b.presets.len();
    match find_gist(&a)? {
        Some(id) => {
            let r = http("PATCH", &format!("https://api.github.com/gists/{id}"), Some(&a.token), Some(&payload));
            match r {
                Ok(_) => {}
                // the gist was deleted on GitHub: make a new one
                Err(e) if e.contains("404") => {
                    a.gist.clear();
                    let _ = save_account(&a);
                    return upload();
                }
                Err(e) => return Err(e),
            }
            a.gist = id;
        }
        None => {
            let r = http("POST", "https://api.github.com/gists", Some(&a.token), Some(&payload))?;
            let v: serde_json::Value = serde_json::from_str(&r).unwrap_or_default();
            a.gist = v["id"].as_str().unwrap_or("").to_string();
        }
    }
    let _ = save_account(&a);
    Ok(format!("Saved your configuration and {n} preset{} to GitHub", if n == 1 { "" } else { "s" }))
}

/// Fetch the bundle from the gist.
pub fn download() -> Result<Bundle, String> {
    let a = load_account();
    if a.token.is_empty() {
        return Err("Sign in with GitHub first.".into());
    }
    let id = find_gist(&a)?.ok_or("There is nothing saved on GitHub yet. Save first (on this or another computer).")?;
    let r = http("GET", &format!("https://api.github.com/gists/{id}"), Some(&a.token), None)?;
    let v: serde_json::Value = serde_json::from_str(&r).map_err(|_| "GitHub gave an answer that is not understood".to_string())?;
    let file = &v["files"][FILE];
    // big files come truncated: fetch them from their address
    let text = match (file["truncated"].as_bool(), file["raw_url"].as_str()) {
        (Some(true), Some(url)) => http("GET", url, Some(&a.token), None)?,
        _ => file["content"].as_str().ok_or("The gist has no Oxide data")?.to_string(),
    };
    serde_json::from_str(&text).map_err(|_| "The data on GitHub is not an Oxide configuration".to_string())
}

/// Take the presets from a bundle (what is already here under the same name is replaced) and, when asked, the
/// configuration itself too.
pub fn import(b: &Bundle, with_config: bool) -> Result<String, String> {
    for p in &b.presets {
        if p.name == presets::UNDO {
            continue;
        }
        presets::save(p).map_err(|e| e.to_string())?;
    }
    if with_config {
        presets::apply(&b.config).map_err(|e| e.to_string())?;
    }
    Ok(format!("Got {} preset{} from GitHub{}", b.presets.len(), if b.presets.len() == 1 { "" } else { "s" }, if with_config { " and the configuration" } else { "" }))
}
