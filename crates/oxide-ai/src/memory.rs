//! What the assistant has learned about the user: short facts in ~/.config/oxide/ai-memory.json.
use oxide_config::ai::AiConfig;

pub fn all() -> Vec<String> {
    std::fs::read_to_string(AiConfig::memory_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(facts: &[String]) {
    let p = AiConfig::memory_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(facts).unwrap_or_default());
}

/// Returns false for duplicates and nonsense.
pub fn add(fact: &str) -> bool {
    let f = fact.trim();
    if f.len() < 4 || f.len() > 200 {
        return false;
    }
    let mut facts = all();
    if facts.iter().any(|x| x.eq_ignore_ascii_case(f)) {
        return false;
    }
    facts.push(f.to_string());
    // the newest 60 are kept
    let n = facts.len();
    if n > 60 {
        facts.drain(..n - 60);
    }
    save(&facts);
    true
}

/// Remove every fact containing `text` (case-insensitive); returns how many.
pub fn forget(text: &str) -> usize {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return 0;
    }
    let facts = all();
    let keep: Vec<String> = facts.iter().filter(|f| !f.to_lowercase().contains(&t)).cloned().collect();
    let removed = facts.len() - keep.len();
    if removed > 0 {
        save(&keep);
    }
    removed
}

pub fn remove_exact(fact: &str) {
    let facts = all();
    let keep: Vec<String> = facts.into_iter().filter(|f| f != fact).collect();
    save(&keep);
}

// ---- lessons: how to do things on this computer with fewer steps, learned from earlier mistakes

pub fn lessons() -> Vec<String> {
    std::fs::read_to_string(AiConfig::lessons_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_lessons(l: &[String]) {
    let p = AiConfig::lessons_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(l).unwrap_or_default());
}

pub fn add_lesson(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 8 || t.len() > 220 {
        return false;
    }
    let mut l = lessons();
    if l.iter().any(|x| x.eq_ignore_ascii_case(t)) {
        return false;
    }
    l.push(t.to_string());
    let n = l.len();
    if n > 40 {
        l.drain(..n - 40);
    }
    save_lessons(&l);
    true
}

pub fn remove_lesson(text: &str) {
    let l: Vec<String> = lessons().into_iter().filter(|x| x != text).collect();
    save_lessons(&l);
}

// ---- the chat itself: plain text turns, kept between sessions

pub fn chat() -> Vec<(String, String)> {
    let v: Vec<serde_json::Value> = std::fs::read_to_string(AiConfig::chat_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    v.iter().filter_map(|x| Some((x["role"].as_str()?.to_string(), x["text"].as_str()?.to_string()))).collect()
}

pub fn chat_push(role: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    let mut v = chat();
    v.push((role.to_string(), text.to_string()));
    let n = v.len();
    if n > 200 {
        v.drain(..n - 200);
    }
    let out: Vec<serde_json::Value> = v.into_iter().map(|(r, t)| serde_json::json!({"role": r, "text": t})).collect();
    let p = AiConfig::chat_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string(&out).unwrap_or_default());
}

pub fn chat_clear() {
    let _ = std::fs::remove_file(AiConfig::chat_path());
}
