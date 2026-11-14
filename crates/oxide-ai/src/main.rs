//! `oxide-ai`: an AI assistant that can control the computer.
//!
//! It is a small long-running process driven over stdin/stdout with JSON lines, so any Quickshell
//! (or other) UI can embed it. See `contrib/shells/oxide/AIClient.qml` for a client.
//!
//! stdin  (UI -> agent):  {"type":"user","text":"..."} | {"type":"confirm_reply","id":N,"allow":bool}
//!                        | {"type":"cancel"} | {"type":"reset"} | {"type":"reload"}
//! stdout (agent -> UI):  ready, turn_start, assistant_start, text_delta, assistant_end, thinking,
//!                        tool_start, confirm, tool_result, usage, done, error  (one JSON object per line)
mod api;
mod memory;
mod safety;
mod tools;

use std::{
    io::{BufRead, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver},
        Arc,
    },
};

use serde_json::{json, Value};

use oxide_config::ai::AiConfig;

pub fn emit(v: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

pub enum Incoming {
    User(String),
    Confirm { id: u64, allow: bool },
    Reset,
    Reload,
}

pub struct Agent {
    pub cfg: AiConfig,
    pub rx: Receiver<Incoming>,
    pub cancel: Arc<AtomicBool>,
    pub messages: Vec<Value>,
    next_id: u64,
    /// dollars spent on the request that is running / since start
    pub turn_usd: f64,
    pub total_usd: f64,
    /// one line per tool call of the running request, for learning from mistakes
    pub trace: Vec<String>,
    /// features switched off for the rest of the session after the API rejected them
    pub no_cache: bool,
    pub no_web: bool,
    pub no_effort: bool,
    pub no_thinking: bool,
}

impl Agent {
    fn new(rx: Receiver<Incoming>, cancel: Arc<AtomicBool>) -> Self {
        Self { cfg: AiConfig::load(), rx, cancel, messages: vec![], next_id: 1, turn_usd: 0.0, total_usd: 0.0, trace: vec![], no_cache: false, no_web: false, no_effort: false, no_thinking: false }
    }

    pub fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Keep the last `history_turns` requests; shorten what is left of older tool output.
    fn trim_history(&mut self) {
        let starts: Vec<usize> = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m["role"] == "user" && m["content"].as_array().is_some_and(|c| c.iter().any(|b| b["type"] == "text")))
            .map(|(i, _)| i)
            .collect();
        let keep = self.cfg.history_turns.max(1) as usize;
        if starts.len() > keep {
            self.messages.drain(..starts[starts.len() - keep]);
        }
        api::slim_old_results(&mut self.messages);
    }

    fn ready(&self) {
        emit(json!({
            "type": "ready",
            "model": self.cfg.model,
            "name": self.cfg.name,
            "permission": self.cfg.permission,
            "has_key": self.cfg.key().is_some(),
        }));
    }

    /// Block until the UI answers confirmation `id` (a cancel counts as a refusal).
    pub fn wait_confirm(&mut self, id: u64) -> bool {
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return false;
            }
            match self.rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(Incoming::Confirm { id: got, allow }) if got == id => return allow,
                Ok(Incoming::Reset) => {
                    self.messages.clear();
                    return false;
                }
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return false,
            }
        }
    }

    fn run_turn(&mut self, text: String) {
        self.cancel.store(false, Ordering::SeqCst);
        // settings may have changed since the last turn
        let fresh = AiConfig::load();
        if fresh != self.cfg {
            self.cfg = fresh;
            self.ready();
        }
        emit(json!({"type": "turn_start"}));
        self.turn_usd = 0.0;
        self.trace.clear();
        api::repair(&mut self.messages);
        self.trim_history();
        let reply_start = self.messages.len() + 1;
        self.messages.push(json!({"role": "user", "content": [{"type": "text", "text": text.clone()}]}));
        if let Err(e) = api::run(self) {
            emit(json!({"type": "error", "message": e}));
            // a half-finished request must not leave dangling tool calls in the history
            self.messages.truncate(reply_start - 1);
        }
        emit(json!({"type": "done"}));
        let reply = api::last_reply_text(&self.messages);
        memory::chat_push("user", &text);
        memory::chat_push("assistant", &reply);
        api::learn(self, &text, &reply);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--check") {
        let cfg = AiConfig::load();
        match api::check(&cfg) {
            Ok(model) => println!("{}", json!({"ok": true, "message": format!("Connected ({model})")})),
            Err(e) => println!("{}", json!({"ok": false, "message": e})),
        }
        return;
    }

    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel2 = cancel.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            let msg = match v["type"].as_str() {
                Some("user") => Incoming::User(v["text"].as_str().unwrap_or("").to_string()),
                Some("confirm_reply") => Incoming::Confirm { id: v["id"].as_u64().unwrap_or(0), allow: v["allow"].as_bool().unwrap_or(false) },
                Some("cancel") => {
                    cancel2.store(true, Ordering::SeqCst);
                    continue;
                }
                Some("reset") => {
                    cancel2.store(true, Ordering::SeqCst);
                    Incoming::Reset
                }
                Some("reload") => Incoming::Reload,
                _ => continue,
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    let mut agent = Agent::new(rx, cancel);
    agent.ready();
    // the chat from last time: shown again, and the last few turns count as context
    let chat = memory::chat();
    if !chat.is_empty() {
        emit(json!({"type": "restore", "items": chat.iter().map(|(r, t)| json!({"role": r, "text": t})).collect::<Vec<_>>()}));
        for (role, text) in chat.iter().rev().take(16).rev() {
            if text.trim().is_empty() {
                continue;
            }
            agent.messages.push(json!({"role": role, "content": [{"type": "text", "text": text}]}));
        }
        api::repair(&mut agent.messages);
    }
    while let Ok(msg) = agent.rx.recv() {
        match msg {
            Incoming::User(text) if !text.trim().is_empty() => agent.run_turn(text),
            Incoming::Reset => {
                memory::chat_clear();
                agent.messages.clear();
                agent.cancel.store(false, Ordering::SeqCst);
            }
            Incoming::Reload => {
                agent.cfg = AiConfig::load();
                agent.ready();
            }
            _ => {}
        }
    }
}
