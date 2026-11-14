//! Settings of the AI assistant (`~/.config/oxide/ai.toml`), shared by `oxide-ai` and the settings app.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiConfig {
    /// Anthropic API key. If empty, `ANTHROPIC_API_KEY` from the environment is used.
    pub api_key: String,
    /// What you call it. Shown in the chat and in its instructions.
    pub name: String,
    pub model: String,
    /// "ask": confirm every action, "safe": run harmless commands directly, "full": never ask.
    pub permission: String,
    pub base_url: String,
    pub max_tokens: u32,
    /// "low", "medium" or "high": how hard the model thinks (not used by Haiku).
    pub effort: String,
    /// Let the API retry on another model if a safety classifier declines a request.
    pub fallbacks: bool,
    /// Extra instructions appended to the built-in system prompt.
    pub instructions: String,
    pub allow_shell: bool,
    pub allow_input: bool,
    pub allow_screenshot: bool,
    pub allow_config: bool,
    /// Web search (a server-side tool of the API).
    pub allow_web: bool,
    /// How many of your last requests it remembers within a conversation (fewer = cheaper).
    pub history_turns: u32,
    /// Stop a single request after it has cost this many US dollars.
    pub cost_cap: f64,
    /// Quietly remember lasting facts about you (kept in ai-memory.json).
    pub learn: bool,
    /// Bumped when the built-in defaults change, so old saved defaults are replaced once.
    pub defaults_version: u32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            name: "Oxide".into(),
            model: "claude-haiku-5-5".into(),
            permission: "safe".into(),
            base_url: "https://api.anthropic.com".into(),
            max_tokens: 4000,
            effort: "low".into(),
            fallbacks: false,
            instructions: String::new(),
            allow_shell: true,
            allow_input: true,
            allow_screenshot: true,
            allow_config: true,
            allow_web: true,
            history_turns: 8,
            cost_cap: 0.5,
            learn: true,
            defaults_version: 2,
        }
    }
}

pub fn path() -> PathBuf {
    crate::Config::path().with_file_name("ai.toml")
}

impl AiConfig {
    pub fn load() -> Self {
        let mut c: Self = std::fs::read_to_string(path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        // version 2: the assistant became fast and cheap by default (Haiku 5.5, low effort, short replies)
        if c.defaults_version < 2 {
            c.model = "claude-haiku-5-5".into();
            c.effort = "low".into();
            c.fallbacks = false;
            c.max_tokens = 4000;
            c.defaults_version = 2;
            let _ = c.save();
        }
        c
    }

    /// The running conversation, so the chat is still there tomorrow.
    pub fn chat_path() -> PathBuf {
        path().with_file_name("ai-chat.json")
    }

    /// Lessons it taught itself from errors and wasted steps.
    pub fn lessons_path() -> PathBuf {
        path().with_file_name("ai-lessons.json")
    }

    /// Where the assistant keeps what it has learned about you.
    pub fn memory_path() -> PathBuf {
        path().with_file_name("ai-memory.json")
    }

    /// The key to use: the file wins, then the environment.
    pub fn key(&self) -> Option<String> {
        let k = if self.api_key.trim().is_empty() { std::env::var("ANTHROPIC_API_KEY").unwrap_or_default() } else { self.api_key.clone() };
        let k = k.trim().to_string();
        (!k.is_empty()).then_some(k)
    }

    /// Write the file readable by the owner only (it may contain an API key).
    pub fn save(&self) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let p = path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = p.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        std::fs::rename(&tmp, &p)
    }
}
