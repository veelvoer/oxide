//! Claude Messages API over raw HTTP (there is no official Rust SDK): streaming, tool use loop.
use std::{io::BufRead, sync::atomic::Ordering, time::Duration};

use oxide_config::ai::AiConfig;
use serde_json::{json, Value};

use crate::{emit, safety, tools, Agent};

const MAX_STEPS: usize = 25;

/// US dollars per million tokens (input, output). Cache reads cost 10 %, cache writes 125 % of input.
fn prices(model: &str) -> (f64, f64) {
    if model.contains("haiku") {
        (0.10, 0.50)
    } else if model.contains("sonnet") {
        (2.0, 10.0)
    } else {
        (4.0, 20.0)
    }
}

fn cost(model: &str, input: u64, output: u64, cache_read: u64, cache_write: u64) -> f64 {
    let (pi, po) = prices(model);
    (input as f64 * pi + output as f64 * po + cache_read as f64 * pi * 0.1 + cache_write as f64 * pi * 1.25) / 1e6
}

fn system_prompt(cfg: &AiConfig) -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "Linux".into());
    let user = std::env::var("USER").unwrap_or_default();
    let mut s = format!(
        "You are {name}, the assistant of {user}'s computer ({os}, Oxide tiling desktop). The user may address you by your name. You act through tools with the user's rights (no sudo).\n\
Style: calm, sharp, dry. No emoji, no exclamation marks. Reply in the user's language.\n\
Answers: ULTRA SHORT. After an action confirm in a few words (\"Firefox is open.\", \"Volume is 30.\"). No explanations, no step lists, no follow-up questions unless asked.\n\
Working: act first, then confirm. Use the most direct tool: open_app, volume, brightness, media, clipboard, oxide_dispatch (windows, workspaces), oxide_config (desktop settings), shell for the rest. Screenshot and input only for graphical tasks nothing else can do. Verify only complicated actions. Search the web for current or uncertain facts. Never invent; if something fails try an alternative or say so.\n\
Safety: before anything destructive or irreversible say what you will do; if the user refuses, accept it. Never reveal secrets (keys, passwords) unless asked for exactly that.\n\
Memory: you learn from the user. Use what you know without mentioning it. Store lasting facts with `remember`.",
        name = if cfg.name.trim().is_empty() { "Oxide" } else { cfg.name.trim() }
    );
    let facts = crate::memory::all();
    if !facts.is_empty() {
        s.push_str("\n\nKnown facts:\n");
        for f in facts {
            s.push_str(&format!("- {f}\n"));
        }
    }
    let lessons = crate::memory::lessons();
    if !lessons.is_empty() {
        s.push_str("\nLessons from your own earlier mistakes (follow them, they save steps):\n");
        for l in lessons {
            s.push_str(&format!("- {l}\n"));
        }
    }
    if !cfg.instructions.trim().is_empty() {
        s.push_str("\nExtra instructions from the user:\n");
        s.push_str(cfg.instructions.trim());
    }
    s
}

fn supports_thinking_param(model: &str) -> bool {
    !model.contains("haiku")
}

/// The 5.x models take an effort level.
fn supports_effort(model: &str) -> bool {
    model.contains("-5-")
}

fn supports_fallbacks(model: &str) -> bool {
    model.contains("opus-5") || model.contains("fable") || model.contains("sonnet-5-5")
}

fn endpoint(cfg: &AiConfig) -> String {
    format!("{}/v1/messages", cfg.base_url.trim_end_matches('/'))
}

fn post(cfg: &AiConfig, body: &Value, fallbacks: bool) -> Result<ureq::Response, String> {
    let Some(key) = cfg.key() else {
        return Err("No API key yet. Open Oxide Settings > Assistant and paste your Anthropic API key.".into());
    };
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(180)).build();
    let mut req = agent
        .post(&endpoint(cfg))
        .set("x-api-key", &key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json");
    if fallbacks {
        req = req.set("anthropic-beta", "server-side-fallback-2026-07-01");
    }
    match req.send_json(body) {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"]["message"].as_str().map(str::to_string)).unwrap_or(text);
            Err(match code {
                401 | 403 => format!("The API key was rejected ({code}). Check it in Settings > Assistant."),
                429 => "Rate limited by the API. Wait a moment and try again.".into(),
                529 | 503 => "The API is overloaded right now. Try again in a moment.".into(),
                _ => format!("API error {code}: {msg}"),
            })
        }
        Err(e) => Err(format!("Could not reach the API: {e}")),
    }
}

/// One cheap request to verify key, model and network. Returns the model name.
pub fn check(cfg: &AiConfig) -> Result<String, String> {
    let body = json!({"model": cfg.model, "max_tokens": 16, "messages": [{"role": "user", "content": "Reply with the word ok."}]});
    let resp = post(cfg, &body, false)?;
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    Ok(v["model"].as_str().unwrap_or(&cfg.model).to_string())
}

fn build_body(agent: &Agent, fallbacks: bool) -> Value {
    let cfg = &agent.cfg;
    let mut tools = tools::definitions(cfg);
    // cache breakpoints: the tools (before the web search tool, which takes none), the system prompt and
    // the end of the conversation so far
    if !agent.no_cache {
        if let Some(last) = tools.last_mut() {
            last["cache_control"] = json!({"type": "ephemeral"});
        }
    }
    if cfg.allow_web && !agent.no_web {
        tools.push(tools::web_search_tool());
    }
    let mut messages = prepare_messages(&agent.messages);
    if !agent.no_cache {
        if let Some(blocks) = messages.last_mut().and_then(|m| m["content"].as_array_mut()) {
            if let Some(l) = blocks.last_mut().filter(|l| l["type"] == "text" || l["type"] == "tool_result") {
                l["cache_control"] = json!({"type": "ephemeral"});
            }
        }
    }
    let system_block = if agent.no_cache {
        json!({"type": "text", "text": system_prompt(cfg)})
    } else {
        json!({"type": "text", "text": system_prompt(cfg), "cache_control": {"type": "ephemeral"}})
    };
    let mut body = json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "stream": true,
        "system": [system_block],
        "tools": tools,
        "messages": messages,
    });
    if supports_thinking_param(&cfg.model) && cfg.effort != "low" && !agent.no_thinking {
        body["thinking"] = json!({"type": "adaptive"});
    }
    if supports_effort(&cfg.model) && !agent.no_effort {
        body["output_config"] = json!({"effort": cfg.effort});
    }
    if fallbacks && supports_fallbacks(&cfg.model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// What is sent: only the last `history_turns` requests, old screenshots dropped and old tool output shortened.
fn prepare_messages(all: &[Value]) -> Vec<Value> {
    let mut m = strip_old_images(all);
    strip_old_thinking(&mut m);
    repair(&mut m);
    m
}

/// A thinking block carries a signature tied to the exact conversation before it. Trimming old requests or
/// shortening old tool output changes that conversation, so thinking from earlier requests is dropped (the API
/// does not need it back); only the request in progress keeps its own.
pub fn strip_old_thinking(messages: &mut [Value]) {
    let last = messages
        .iter()
        .rposition(|m| m["role"] == "user" && m["content"].as_array().is_some_and(|c| c.iter().any(|b| b["type"] == "text")))
        .unwrap_or(0);
    for m in messages[..last].iter_mut().filter(|m| m["role"] == "assistant") {
        if let Some(blocks) = m["content"].as_array_mut() {
            blocks.retain(|b| b["type"] != "thinking" && b["type"] != "redacted_thinking");
        }
    }
}

/// Every tool call needs its result right after it, or the API answers 400. A cancelled or interrupted
/// request can leave calls dangling: close them with "(aborted)", drop half-finished web searches and
/// empty messages, and make sure the conversation starts with a user message and alternates.
pub fn repair(messages: &mut Vec<Value>) {
    let mut i = 0;
    while i < messages.len() {
        if messages[i]["role"] == "assistant" {
            if let Some(blocks) = messages[i]["content"].as_array_mut() {
                // a server tool call (web search) without its result cannot be sent back
                let results: Vec<String> = blocks.iter().filter(|b| b["type"] == "web_search_tool_result").filter_map(|b| b["tool_use_id"].as_str().map(str::to_string)).collect();
                blocks.retain(|b| b["type"] != "server_tool_use" || b["id"].as_str().is_some_and(|id| results.iter().any(|r| r == id)));
                blocks.retain(|b| !(b["type"] == "text" && b["text"].as_str().unwrap_or("").is_empty()));
            }
            let ids: Vec<String> = messages[i]["content"].as_array().map(|c| c.iter().filter(|b| b["type"] == "tool_use").filter_map(|b| b["id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
            if !ids.is_empty() {
                let have: Vec<String> = messages
                    .get(i + 1)
                    .filter(|n| n["role"] == "user")
                    .and_then(|n| n["content"].as_array())
                    .map(|c| c.iter().filter(|b| b["type"] == "tool_result").filter_map(|b| b["tool_use_id"].as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let missing: Vec<Value> = ids
                    .iter()
                    .filter(|id| !have.contains(id))
                    .map(|id| json!({"type": "tool_result", "tool_use_id": id, "content": [{"type": "text", "text": "(aborted)"}], "is_error": true}))
                    .collect();
                if !missing.is_empty() {
                    match messages.get_mut(i + 1).filter(|n| n["role"] == "user" && n["content"].is_array()) {
                        Some(n) => {
                            let mut v = missing;
                            v.extend(n["content"].as_array().cloned().unwrap_or_default());
                            n["content"] = Value::Array(v);
                        }
                        None => messages.insert(i + 1, json!({"role": "user", "content": missing})),
                    }
                }
            }
            if messages[i]["content"].as_array().is_some_and(|c| c.is_empty()) {
                messages.remove(i);
                continue;
            }
        }
        i += 1;
    }
    // must start with a user message
    while messages.first().is_some_and(|m| m["role"] != "user") {
        messages.remove(0);
    }
}

/// Keep only the two most recent screenshots in the history to save tokens.
fn strip_old_images(messages: &[Value]) -> Vec<Value> {
    let mut seen = 0;
    let mut out: Vec<Value> = messages.to_vec();
    for m in out.iter_mut().rev() {
        let Some(blocks) = m["content"].as_array_mut() else { continue };
        for b in blocks.iter_mut() {
            if b["type"] == "tool_result" {
                if let Some(inner) = b["content"].as_array_mut() {
                    if inner.iter().any(|x| x["type"] == "image") {
                        seen += 1;
                        if seen > 2 {
                            inner.retain(|x| x["type"] != "image");
                            inner.push(json!({"type": "text", "text": "[older screenshot removed]"}));
                        }
                    }
                }
            }
        }
    }
    out
}

struct Streamed {
    content: Vec<Value>,
    stop_reason: String,
    stop_category: Option<String>,
}

fn stream_once(agent: &mut Agent) -> Result<Streamed, String> {
    let mut use_fallbacks = agent.cfg.fallbacks;
    let mut attempts = 0;
    let resp = loop {
        let body = build_body(agent, use_fallbacks);
        match post(&agent.cfg, &body, use_fallbacks && supports_fallbacks(&agent.cfg.model)) {
            Ok(r) => break r,
            Err(e) if use_fallbacks && (e.contains("fallback") || e.contains("beta")) => {
                // the API does not know the fallback feature: continue without it
                use_fallbacks = false;
                agent.cfg.fallbacks = false;
            }
            Err(e) if e.starts_with("API error 400") && attempts < 4 => {
                // a 400 is the request itself: repair the history, or switch off the feature the message names, and try again
                attempts += 1;
                log(&format!("400: {e}"));
                let l = e.to_lowercase();
                if l.contains("cache_control") || l.contains("cache") {
                    agent.no_cache = true;
                } else if l.contains("web_search") || l.contains("server tool") {
                    agent.no_web = true;
                } else if l.contains("effort") || l.contains("output_config") {
                    agent.no_effort = true;
                } else if l.contains("signature") || l.contains("thinking") {
                    // thinking blocks no longer match the conversation: drop every one, and stop asking for more
                    for m in agent.messages.iter_mut().filter(|m| m["role"] == "assistant") {
                        if let Some(blocks) = m["content"].as_array_mut() {
                            blocks.retain(|b| b["type"] != "thinking" && b["type"] != "redacted_thinking");
                        }
                    }
                    repair(&mut agent.messages);
                    agent.no_thinking = true;
                } else {
                    repair(&mut agent.messages);
                    if attempts >= 2 {
                        // still failing: drop everything but the newest request and tool state
                        let keep = agent.messages.iter().rposition(|m| m["role"] == "user" && m["content"].as_array().is_some_and(|c| c.iter().any(|b| b["type"] == "text"))).unwrap_or(0);
                        agent.messages.drain(..keep);
                    }
                }
            }
            Err(e) if (e.contains("overloaded") || e.contains("Rate limited")) && attempts < 2 => {
                attempts += 1;
                std::thread::sleep(Duration::from_secs(2 * attempts));
            }
            Err(e) => return Err(e),
        }
    };

    let reader = std::io::BufReader::new(resp.into_reader());
    let mut blocks: Vec<Value> = vec![];
    let mut partial_json: Vec<String> = vec![];
    let mut stop_reason = String::new();
    let mut category = None;
    let (mut in_tokens, mut out_tokens, mut cache_read, mut cache_write) = (0u64, 0u64, 0u64, 0u64);

    for line in reader.lines() {
        if agent.cancel.load(Ordering::SeqCst) {
            return Ok(Streamed { content: finish_blocks(blocks, partial_json), stop_reason: "cancelled".into(), stop_category: None });
        }
        let line = line.map_err(|e| format!("connection lost: {e}"))?;
        let Some(data) = line.strip_prefix("data:") else { continue };
        let Ok(ev) = serde_json::from_str::<Value>(data.trim()) else { continue };
        match ev["type"].as_str().unwrap_or("") {
            "message_start" => {
                let u = &ev["message"]["usage"];
                in_tokens = u["input_tokens"].as_u64().unwrap_or(0);
                cache_read = u["cache_read_input_tokens"].as_u64().unwrap_or(0);
                cache_write = u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
            }
            "content_block_start" => {
                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                let b = ev["content_block"].clone();
                while blocks.len() <= idx {
                    blocks.push(Value::Null);
                    partial_json.push(String::new());
                }
                match b["type"].as_str() {
                    Some("text") => emit(json!({"type": "assistant_start"})),
                    Some("thinking") | Some("redacted_thinking") => emit(json!({"type": "thinking"})),
                    Some("server_tool_use") => emit(json!({"type": "tool_start", "id": 0, "name": "web_search", "input": {}})),
                    _ => {}
                }
                blocks[idx] = b;
            }
            "content_block_delta" => {
                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                if idx >= blocks.len() {
                    continue;
                }
                let d = &ev["delta"];
                match d["type"].as_str().unwrap_or("") {
                    "text_delta" => {
                        let t = d["text"].as_str().unwrap_or("");
                        let cur = blocks[idx]["text"].as_str().unwrap_or("").to_string();
                        blocks[idx]["text"] = json!(format!("{cur}{t}"));
                        emit(json!({"type": "text_delta", "text": t}));
                    }
                    "thinking_delta" => {
                        let t = d["thinking"].as_str().unwrap_or("");
                        let cur = blocks[idx]["thinking"].as_str().unwrap_or("").to_string();
                        blocks[idx]["thinking"] = json!(format!("{cur}{t}"));
                    }
                    "signature_delta" => {
                        blocks[idx]["signature"] = json!(d["signature"].as_str().unwrap_or(""));
                    }
                    "input_json_delta" => partial_json[idx].push_str(d["partial_json"].as_str().unwrap_or("")),
                    _ => {}
                }
            }
            "content_block_stop" => {
                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                if idx < blocks.len() && blocks[idx]["type"] == "text" {
                    emit(json!({"type": "assistant_end"}));
                }
            }
            "message_delta" => {
                if let Some(r) = ev["delta"]["stop_reason"].as_str() {
                    stop_reason = r.to_string();
                }
                category = ev["delta"]["stop_details"]["category"].as_str().map(str::to_string);
                out_tokens = ev["usage"]["output_tokens"].as_u64().unwrap_or(out_tokens);
            }
            "error" => {
                let m = ev["error"]["message"].as_str().unwrap_or("stream error");
                return Err(m.to_string());
            }
            _ => {}
        }
    }
    let usd = cost(&agent.cfg.model, in_tokens, out_tokens, cache_read, cache_write);
    agent.turn_usd += usd;
    agent.total_usd += usd;
    emit(json!({"type": "usage", "input": in_tokens, "output": out_tokens, "cached": cache_read, "usd": usd, "total_usd": agent.total_usd}));
    Ok(Streamed { content: finish_blocks(blocks, partial_json), stop_reason, stop_category: category })
}

/// Turn accumulated tool input JSON into real objects and drop empty slots.
fn finish_blocks(blocks: Vec<Value>, partial: Vec<String>) -> Vec<Value> {
    blocks
        .into_iter()
        .zip(partial)
        .filter(|(b, _)| !b.is_null())
        .map(|(mut b, p)| {
            if b["type"] == "tool_use" || b["type"] == "server_tool_use" {
                b["input"] = if p.trim().is_empty() { json!({}) } else { serde_json::from_str(&p).unwrap_or_else(|_| json!({"_invalid_json": p})) };
            }
            b
        })
        .collect()
}

/// Run one user turn to completion: model calls and tool executions until the model stops.
pub fn run(agent: &mut Agent) -> Result<(), String> {
    for _ in 0..MAX_STEPS {
        if agent.turn_usd > agent.cfg.cost_cap {
            return Err(format!("Cost limit of ${:.2} for this request reached. Change it in Settings > Assistant.", agent.cfg.cost_cap));
        }
        let r = stream_once(agent)?;
        let mut content = r.content;
        // an empty text block is not valid to send back
        content.retain(|b| !(b["type"] == "text" && b["text"].as_str().unwrap_or("").is_empty()));
        if !content.is_empty() {
            agent.messages.push(json!({"role": "assistant", "content": content.clone()}));
        }
        match r.stop_reason.as_str() {
            "tool_use" => {
                let mut results = vec![];
                for b in content.iter().filter(|b| b["type"] == "tool_use") {
                    results.push(run_tool(agent, b));
                }
                agent.messages.push(json!({"role": "user", "content": results}));
                if agent.cancel.load(Ordering::SeqCst) {
                    return Ok(());
                }
            }
            "pause_turn" => {}
            "refusal" => {
                let why = r.stop_category.unwrap_or_default();
                emit(json!({"type": "assistant_start"}));
                emit(json!({"type": "text_delta", "text": format!("I can't help with that request{}.", if why.is_empty() { String::new() } else { format!(" ({why})") })}));
                emit(json!({"type": "assistant_end"}));
                return Ok(());
            }
            "max_tokens" => return Err("The reply was cut off because it got too long.".into()),
            _ => return Ok(()),
        }
    }
    Err("Stopped after too many steps. Ask me to continue if you want more.".into())
}

fn run_tool(agent: &mut Agent, b: &Value) -> Value {
    let id = b["id"].as_str().unwrap_or("").to_string();
    let name = b["name"].as_str().unwrap_or("").to_string();
    let input = b["input"].clone();
    let cid = agent.id();
    emit(json!({"type": "tool_start", "id": cid, "name": name, "input": input}));

    let result = |ok: bool, text: String, image: Option<String>| -> Value {
        let mut content = vec![json!({"type": "text", "text": text})];
        if let Some(img) = image {
            content.push(json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": img}}));
        }
        json!({"type": "tool_result", "tool_use_id": id, "content": content, "is_error": !ok})
    };

    if agent.cancel.load(Ordering::SeqCst) {
        return result(false, "Cancelled by the user.".into(), None);
    }
    if input.get("_invalid_json").is_some() {
        return result(false, "The tool input was not valid JSON. Try again.".into(), None);
    }
    let (ask, risk) = safety::needs_confirmation(&agent.cfg.permission, &name, &input);
    if ask {
        emit(json!({"type": "confirm", "id": cid, "name": name, "summary": safety::summarize(&name, &input), "risk": risk}));
        if !agent.wait_confirm(cid) {
            emit(json!({"type": "tool_result", "id": cid, "name": name, "ok": false, "output": "Denied", "has_image": false}));
            return result(false, "The user did not allow this action. Do not retry it; ask what they want instead.".into(), None);
        }
    }
    let out = tools::execute(&name, &input);
    {
        let short = |s: &str, n: usize| s.chars().take(n).collect::<String>().replace('\n', " ");
        agent.trace.push(format!("{}({}) -> {}: {}", name, short(&input.to_string(), 160), if out.ok { "ok" } else { "ERROR" }, short(&out.text, 160)));
    }
    emit(json!({"type": "tool_result", "id": cid, "name": name, "ok": out.ok, "output": out.text.chars().take(2000).collect::<String>(), "has_image": out.image_b64.is_some()}));
    result(out.ok, out.text, out.image_b64)
}

/// Long output of earlier tool calls is cut down; the last request keeps everything.
pub fn slim_old_results(messages: &mut [Value]) {
    let last_user_text = messages.iter().rposition(|m| m["role"] == "user" && m["content"].as_array().is_some_and(|c| c.iter().any(|b| b["type"] == "text")));
    let end = last_user_text.unwrap_or(messages.len());
    for m in messages[..end].iter_mut() {
        let Some(blocks) = m["content"].as_array_mut() else { continue };
        for b in blocks.iter_mut().filter(|b| b["type"] == "tool_result") {
            if let Some(inner) = b["content"].as_array_mut() {
                for x in inner.iter_mut() {
                    if x["type"] == "text" {
                        let t = x["text"].as_str().unwrap_or("");
                        if t.chars().count() > 400 {
                            x["text"] = json!(format!("{} ...[cut]", t.chars().take(300).collect::<String>()));
                        }
                    }
                }
            }
        }
    }
}

pub fn last_reply_text(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m["role"] == "assistant")
        .and_then(|m| m["content"].as_array())
        .map(|c| c.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

const LEARN_PROMPT: &str = "You maintain the long-term memory of a personal assistant that controls a Linux computer. You get the known facts, the known lessons, the latest exchange and a trace of the tool calls it made. Answer ONLY with JSON: {\"add\": [], \"remove\": [], \"lessons\": [], \"drop_lessons\": []}.\nadd: new lasting facts about the user (name, preferences, routines, apps and devices used, how they want the assistant to behave, corrections, recurring people or projects). One short sentence each, third person. remove: facts (copied exactly) made obsolete or contradicted.\nlessons: concrete, reusable tips for the assistant itself so it needs fewer steps next time, drawn from the trace: a command or tool that failed and what worked instead, something that is installed or missing on this machine, a wrong assumption, a detour that a direct call would have avoided, or something the user had to correct. Short imperative sentences (\"Use wpctl, pactl is not installed.\"). Only generalisable tips, nothing about one-off content. drop_lessons: known lessons (copied exactly) that turned out wrong.\nNothing about one-off commands, temporary states or screen contents. When in doubt: empty lists. No duplicates.";

/// After a request: one cheap call lets the model keep what is worth remembering and teach itself from
/// errors and detours (the tool trace), so the next request needs fewer steps.
pub fn learn(agent: &mut Agent, user_text: &str, reply: &str) {
    let had_error = agent.trace.iter().any(|t| t.contains("-> ERROR"));
    let long = agent.trace.len() >= 4;
    let correction = {
        let l = user_text.to_lowercase();
        ["no,", "not that", "wrong", "niet zo", "fout", "stop", "again", "i said", "nee", "instead", "ipv", "beter", "je moet"].iter().any(|w| l.contains(w))
    };
    let facts_worth = user_text.trim().chars().count() >= 8;
    if !agent.cfg.learn || agent.cfg.key().is_none() || !(had_error || long || correction || facts_worth) {
        return;
    }
    let known = crate::memory::all();
    let lessons = crate::memory::lessons();
    let list = |v: &[String]| if v.is_empty() { "(none)".to_string() } else { v.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n") };
    let trace = if agent.trace.is_empty() { "(no tools used)".to_string() } else { agent.trace.iter().take(14).map(|t| format!("- {t}")).collect::<Vec<_>>().join("\n") };
    let input = format!(
        "Known facts:\n{}\n\nKnown lessons:\n{}\n\nUser: {}\nAssistant: {}\nTool trace ({} calls, {} errors, ${:.4} spent):\n{}",
        list(&known),
        list(&lessons),
        user_text.chars().take(600).collect::<String>(),
        reply.chars().take(400).collect::<String>(),
        agent.trace.len(),
        agent.trace.iter().filter(|t| t.contains("-> ERROR")).count(),
        agent.turn_usd,
        trace
    );
    let mut cfg = agent.cfg.clone();
    cfg.model = "claude-haiku-5-5".into();
    let mut body = json!({"model": cfg.model, "max_tokens": 600, "system": LEARN_PROMPT, "messages": [{"role": "user", "content": input}]});
    if !agent.no_effort {
        body["output_config"] = json!({"effort": "low"});
    }
    let Ok(resp) = post(&cfg, &body, false) else { return };
    let Ok(v) = resp.into_json::<Value>() else { return };
    let u = &v["usage"];
    let usd = cost(&cfg.model, u["input_tokens"].as_u64().unwrap_or(0), u["output_tokens"].as_u64().unwrap_or(0), 0, 0);
    agent.total_usd += usd;
    let raw: String = v["content"].as_array().map(|c| c.iter().filter_map(|b| b["text"].as_str()).collect()).unwrap_or_default();
    let (Some(a0), Some(b0)) = (raw.find('{'), raw.rfind('}')) else { return };
    let Ok(data) = serde_json::from_str::<Value>(&raw[a0..=b0]) else { return };
    let strs = |k: &str| data[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>();
    for old in strs("remove") {
        crate::memory::remove_exact(&old);
    }
    for f in strs("add") {
        crate::memory::add(&f);
    }
    for old in strs("drop_lessons") {
        crate::memory::remove_lesson(&old);
    }
    let mut learned = vec![];
    for l in strs("lessons") {
        if crate::memory::add_lesson(&l) {
            learned.push(l);
        }
    }
    if !learned.is_empty() {
        log(&format!("lessons: {}", learned.join(" | ")));
    }
}

/// Errors worth keeping for later: ~/.cache/oxide/ai.log
pub fn log(line: &str) {
    use std::io::Write;
    let dir = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache"))).map(|d| d.join("oxide"));
    if let Some(d) = dir {
        let _ = std::fs::create_dir_all(&d);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("ai.log")) {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_closes_dangling_tool_calls() {
        let mut m = vec![
            json!({"role": "user", "content": [{"type": "text", "text": "hi"}]}),
            json!({"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "volume", "input": {}}, {"type": "server_tool_use", "id": "s1", "name": "web_search", "input": {}}]}),
            json!({"role": "user", "content": [{"type": "text", "text": "next"}]}),
        ];
        repair(&mut m);
        // the tool call got an "(aborted)" result placed first in the following user message
        let next = m[2]["content"].as_array().unwrap();
        assert_eq!(next[0]["type"], "tool_result");
        assert_eq!(next[0]["tool_use_id"], "t1");
        // the unfinished web search is gone
        assert!(m[1]["content"].as_array().unwrap().iter().all(|b| b["type"] != "server_tool_use"));
    }

    #[test]
    fn repair_drops_leading_assistant_messages() {
        let mut m = vec![json!({"role": "assistant", "content": [{"type": "text", "text": "x"}]}), json!({"role": "user", "content": [{"type": "text", "text": "y"}]})];
        repair(&mut m);
        assert_eq!(m.len(), 1);
    }
}
