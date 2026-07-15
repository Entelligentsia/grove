//! Structured, per-session tracing of explore-mode LLM traffic — plus the
//! pretty-printers that render a single request/response for humans.
//!
//! Two layers live here:
//! - [`TraceWriter`] — the **in-process tap** (`ExploreConfig::tap`). One writer
//!   per `grove serve` session records a JSONL stream to
//!   `.grove/traces/<session_id>.jsonl`: a `session` header, then `call_start` /
//!   `turn` / `call_end` events per `explore` invocation. Retention prunes the
//!   oldest sessions. `grove tap` browses these files.
//! - [`format_request`] / [`format_response`] — pretty-printers that turn one
//!   OpenAI chat-completions request/response [`serde_json::Value`] into a
//!   compact, human-readable block. The `grove tap` TUI renders each stored turn
//!   through these.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::client::Usage;

/// The directory holding per-session trace files for a project rooted at `root`.
pub fn traces_dir(root: &Path) -> PathBuf {
    root.join(".grove").join("traces")
}

/// Immutable header describing one `grove serve` trace session.
#[derive(Debug, Clone)]
pub struct SessionMeta {
    /// Filename-safe id: `<epoch_secs>-<client-slug>-<pid>`. The pid keeps two
    /// `grove serve` processes that start in the same second under the same
    /// client from colliding on one file (which would interleave their writes).
    pub session_id: String,
    /// Session start, epoch seconds (UTC).
    pub started_at: u64,
    /// The MCP client name (from `initialize` `clientInfo`), or `"unknown"`.
    pub client_name: String,
    /// The MCP client version, if reported.
    pub client_version: String,
    /// The model the session drives.
    pub model: String,
    /// The steering mode (lowercase spelling).
    pub mode: String,
    /// The provider (lowercase spelling).
    pub provider: String,
    /// The OpenAI-compatible base URL.
    pub base_url: String,
}

impl SessionMeta {
    /// Build a session header from the explore config and the MCP client
    /// identity (`name`, `version`), stamping the current time.
    pub fn new(
        model: &str,
        mode: &str,
        provider: &str,
        base_url: &str,
        client_name: &str,
        client_version: &str,
    ) -> Self {
        let started_at = now_epoch_secs();
        let name = if client_name.trim().is_empty() {
            "unknown"
        } else {
            client_name
        };
        let session_id = format!("{started_at}-{}-{}", slug(name), std::process::id());
        SessionMeta {
            session_id,
            started_at,
            client_name: name.to_string(),
            client_version: client_version.to_string(),
            model: model.to_string(),
            mode: mode.to_string(),
            provider: provider.to_string(),
            base_url: base_url.to_string(),
        }
    }
}

/// A structured per-session trace sink. One per `grove serve` session; each
/// `explore` invocation is a *call*, each model round-trip a *turn*.
///
/// All writes are best-effort (tracing must never break a run) and append a
/// single JSON line, so an interrupted process leaves a valid partial stream.
pub struct TraceWriter {
    path: PathBuf,
    next_call: AtomicU32,
}

impl TraceWriter {
    /// Open a new session trace under `.grove/traces/`, writing the `session`
    /// header line and pruning old sessions down to `retain` (0 = keep all).
    /// Returns `None` if the directory or header can't be written — tracing then
    /// silently no-ops rather than failing the session.
    pub fn open(root: &Path, meta: &SessionMeta, retain: u32) -> Option<Self> {
        let dir = traces_dir(root);
        std::fs::create_dir_all(&dir).ok()?;
        prune(&dir, retain);
        let path = dir.join(format!("{}.jsonl", meta.session_id));
        let header = json!({
            "t": "session",
            "session_id": meta.session_id,
            "started_at": meta.started_at,
            "client": { "name": meta.client_name, "version": meta.client_version },
            "model": meta.model,
            "mode": meta.mode,
            "provider": meta.provider,
            "base_url": meta.base_url,
        });
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        writeln!(f, "{header}").ok()?;
        Some(TraceWriter {
            path,
            next_call: AtomicU32::new(1),
        })
    }

    /// Begin a call (one `explore` invocation). Returns the call id used to
    /// correlate its `turn` and `call_end` events.
    pub fn call_start(&self, query: &str) -> u32 {
        let call_id = self.next_call.fetch_add(1, Ordering::SeqCst);
        self.write(&json!({
            "t": "call_start",
            "call_id": call_id,
            "query": query,
            "started_at": now_epoch_secs(),
        }));
        call_id
    }

    /// Record one model round-trip under `call_id`. `request`/`response` are the
    /// raw chat-completions bodies (the TUI re-renders them via the
    /// pretty-printers); `usage` and `wall_ms` carry the metrics.
    pub fn turn(
        &self,
        call_id: u32,
        turn_index: usize,
        request: &Value,
        response: &Value,
        usage: Option<Usage>,
        wall_ms: u128,
    ) {
        self.write(&json!({
            "t": "turn",
            "call_id": call_id,
            "turn": turn_index,
            "request": request,
            "response": response,
            "usage": usage.map(usage_json),
            "wall_ms": wall_ms,
        }));
    }

    /// Close a call, recording its aggregate metrics and final answer.
    pub fn call_end(
        &self,
        call_id: u32,
        answer: &str,
        turns: usize,
        tokens: Usage,
        wall_ms: u128,
        truncated: bool,
    ) {
        self.write(&json!({
            "t": "call_end",
            "call_id": call_id,
            "turns": turns,
            "tokens": usage_json(tokens),
            "wall_ms": wall_ms,
            "answer": answer,
            "truncated": truncated,
        }));
    }

    /// Append one event line (best-effort).
    fn write(&self, event: &Value) {
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&self.path) {
            let _ = writeln!(f, "{event}");
        }
    }
}

fn usage_json(u: Usage) -> Value {
    json!({
        "prompt": u.prompt_tokens,
        "completion": u.completion_tokens,
        "total": u.total_tokens,
    })
}

/// Prune `dir` to at most `retain` `*.jsonl` sessions (keeping the newest by the
/// epoch prefix in the filename); `retain == 0` keeps everything. Called just
/// before a new session file is created, so it leaves room for the incoming one.
/// Only ever deletes `*.jsonl` files directly inside `dir`.
fn prune(dir: &Path, retain: u32) {
    if retain == 0 {
        return;
    }
    let mut sessions: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect(),
        Err(_) => return,
    };
    // Newest last: filenames start with the epoch-seconds prefix, so a plain
    // lexicographic sort orders them chronologically.
    sessions.sort();
    // Keep room for the session about to be created: keep the newest retain-1.
    let keep = (retain as usize).saturating_sub(1);
    if sessions.len() <= keep {
        return;
    }
    let remove = sessions.len() - keep;
    for p in sessions.into_iter().take(remove) {
        let _ = std::fs::remove_file(p);
    }
}

/// Current time in whole epoch seconds (UTC), `0` if the clock is before the
/// epoch (never, in practice).
fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Filename-safe slug: lowercase alphanumerics kept, every other run collapsed
/// to a single `-`, trimmed. Empty input yields `"unknown"`.
fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

/// Format a chat-completions **request** body into a single readable block
/// (every message in order). Kept for callers wanting the flat rendering;
/// the trace browser uses [`request_parts`] for its tiered view.
pub fn format_request(body: &Value) -> String {
    let mut out = String::from("→ request");
    let Some(msgs) = body.get("messages").and_then(Value::as_array) else {
        out.push_str(&format!("\n  {}", truncate(&body.to_string(), 800)));
        return out;
    };
    let model = body.get("model").and_then(Value::as_str).unwrap_or("?");
    let temp = num(body.get("temperature"));
    let maxt = num(body
        .get("max_completion_tokens")
        .or_else(|| body.get("max_tokens")));
    let ntools = body.get("tools").and_then(Value::as_array).map_or(0, Vec::len);
    out.push_str(&format!(
        "  model={model} temp={temp} max_tokens={maxt} tools={ntools} messages={}",
        msgs.len()
    ));
    for m in msgs {
        push_message(&mut out, m);
    }
    out
}

/// A request body split into the three tiers the trace browser renders: the
/// (collapsed) `system` prompt, the (collapsed) `context` — the prior exchange —
/// and the prominent `prompt`: the newest messages the model is being asked to
/// act on. The split point is the **last `assistant` message**; everything after
/// it is the prompt (on turn 1, with no assistant yet, that is the original
/// question). Each tier is pre-rendered text with role-prefixed lines;
/// `context_msgs` is the middle message count, for the section label.
pub struct RequestParts {
    /// `model=… · N msgs · K tools` — the one-line request header.
    pub header: String,
    /// Leading `system` message(s), role-prefixed. Empty if none.
    pub system: String,
    /// The messages between the system block and the prompt. Empty on turn 1.
    pub context: String,
    /// Number of messages in `context` (for the "context · N msgs" label).
    pub context_msgs: usize,
    /// The trailing block after the last `assistant` message — what this turn
    /// is acting on. Role-prefixed so it reads `[tool] …` / `[user] …` honestly.
    pub prompt: String,
}

/// Split a request `body` into [`RequestParts`] for the tiered browser view.
pub fn request_parts(body: &Value) -> RequestParts {
    let msgs: Vec<Value> = body
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let model = body.get("model").and_then(Value::as_str).unwrap_or("?");
    let ntools = body.get("tools").and_then(Value::as_array).map_or(0, Vec::len);
    let header = format!("model={model} · {} msgs · {ntools} tools", msgs.len());

    // Leading run of system messages | middle context | trailing prompt.
    let sys_end = msgs.iter().position(|m| role(m) != "system").unwrap_or(msgs.len());
    let last_assistant = msgs.iter().rposition(|m| role(m) == "assistant");
    // The prompt starts after the last assistant message, but never inside the
    // system block. If the request happens to end with an assistant message
    // (no trailing input), fall back to showing the last message as the prompt.
    let mut prompt_start = last_assistant.map_or(sys_end, |i| i + 1);
    if prompt_start >= msgs.len() && !msgs.is_empty() {
        prompt_start = msgs.len() - 1;
    }
    let prompt_start = prompt_start.clamp(sys_end, msgs.len());

    RequestParts {
        header,
        system: render_messages(&msgs[..sys_end]),
        context: render_messages(&msgs[sys_end..prompt_start]),
        context_msgs: prompt_start - sys_end,
        prompt: render_messages(&msgs[prompt_start..]),
    }
}

fn role(m: &Value) -> &str {
    m.get("role").and_then(Value::as_str).unwrap_or("?")
}

/// Render a slice of chat messages to role-prefixed lines (no trailing newline).
fn render_messages(msgs: &[Value]) -> String {
    let mut out = String::new();
    for m in msgs {
        push_message(&mut out, m);
    }
    out.trim_start_matches('\n').to_string()
}

/// Append one message's tool-calls and content to `out`, each role-prefixed and
/// on its own line. Shared by the flat and tiered request renderers.
fn push_message(out: &mut String, m: &Value) {
    let role = role(m);
    if let Some(tcs) = m.get("tool_calls").and_then(Value::as_array) {
        let calls: Vec<String> = tcs
            .iter()
            .map(|c| format!("{}({})", call_name(c), truncate(&call_args(c), 300)))
            .collect();
        if !calls.is_empty() {
            out.push_str(&format!("\n  [{role}] tool_calls: {}", calls.join(", ")));
        }
    }
    if let Some(content) = m.get("content").and_then(Value::as_str) {
        if !content.is_empty() {
            out.push_str(&format!("\n  [{role}] {}", truncate(content, 4000)));
        }
    }
}

/// Format a chat-completions **response** body, with optional elapsed millis.
pub fn format_response(body: &Value, ms: Option<u128>) -> String {
    let mut out = match ms {
        Some(ms) => format!("← response ({ms}ms)"),
        None => String::from("← response"),
    };
    if let Some(msg) = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
    {
        if let Some(content) = msg.get("content").and_then(Value::as_str) {
            if !content.is_empty() {
                out.push_str(&format!("\n  [assistant] {}", truncate(content, 4000)));
            }
        }
        for c in msg.get("tool_calls").and_then(Value::as_array).unwrap_or(&Vec::new()) {
            out.push_str(&format!(
                "\n  [assistant] call: {}({})",
                call_name(c),
                truncate(&call_args(c), 500)
            ));
        }
    } else {
        out.push_str(&format!("\n  {}", truncate(&body.to_string(), 800)));
    }
    if let Some(u) = body.get("usage") {
        out.push_str(&format!(
            "\n  usage: prompt={} completion={} total={}",
            num(u.get("prompt_tokens")),
            num(u.get("completion_tokens")),
            num(u.get("total_tokens")),
        ));
    }
    out
}

fn call_name(c: &Value) -> String {
    c.get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string()
}

/// Tool-call arguments — a JSON-encoded string (OpenAI/Ollama) or an object
/// (llama.cpp); render either compactly.
fn call_args(c: &Value) -> String {
    match c.get("function").and_then(|f| f.get("arguments")) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn num(v: Option<&Value>) -> String {
    v.map_or_else(|| "-".to_string(), ToString::to_string)
}

/// Char-safe truncation (bodies contain multibyte text; never slice mid-char).
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formats_request_messages_and_tool_calls() {
        let body = json!({
            "model": "qwen3.5:4b", "temperature": 0, "max_completion_tokens": 1024,
            "tools": [{"type":"function"}],
            "messages": [
                {"role": "user", "content": "where is X?"},
                {"role": "assistant", "tool_calls": [{"function": {"name": "Grove", "arguments": "{\"command\":\"symbols .\"}"}}]}
            ]
        });
        let s = format_request(&body);
        assert!(s.contains("model=qwen3.5:4b"));
        assert!(s.contains("[user] where is X?"));
        assert!(s.contains("Grove({\"command\":\"symbols .\"})"));
    }

    #[test]
    fn request_parts_splits_system_context_and_trailing_prompt() {
        let body = json!({
            "model": "qwen3.5:4b",
            "tools": [{"type":"function"}],
            "messages": [
                {"role": "system", "content": "You are a locator."},
                {"role": "user", "content": "where is diff"},
                {"role": "assistant", "tool_calls": [{"function": {"name": "Grep", "arguments": "{}"}}]},
                {"role": "tool", "content": "builtin/diff.c:1"}
            ]
        });
        let p = request_parts(&body);
        assert!(p.header.contains("4 msgs") && p.header.contains("1 tools"), "header: {}", p.header);
        assert!(p.system.contains("[system] You are a locator."));
        // context = the middle (question + prior assistant call); prompt = the
        // trailing block after the last assistant message (the fresh tool result).
        assert_eq!(p.context_msgs, 2);
        assert!(p.context.contains("[user] where is diff"));
        assert!(p.context.contains("[assistant] tool_calls: Grep"));
        assert!(p.prompt.contains("[tool] builtin/diff.c:1"), "prompt: {}", p.prompt);
        assert!(!p.prompt.contains("[system]"), "system not in prompt");
    }

    #[test]
    fn request_parts_first_turn_has_no_context_and_prompts_the_question() {
        // Turn 1: system + question, no assistant yet → prompt is the question,
        // context is empty.
        let body = json!({"messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "where is main"}
        ]});
        let p = request_parts(&body);
        assert_eq!(p.context_msgs, 0);
        assert!(p.context.is_empty());
        assert!(p.prompt.contains("[user] where is main"), "prompt: {}", p.prompt);
    }

    #[test]
    fn formats_response_content_and_usage() {
        let body = json!({
            "choices": [{"message": {"content": "found it", "role": "assistant"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
        });
        let s = format_response(&body, Some(42));
        assert!(s.contains("(42ms)"));
        assert!(s.contains("[assistant] found it"));
        assert!(s.contains("total=13"));
    }

    #[test]
    fn truncate_is_char_safe() {
        let s = "é".repeat(50);
        assert!(truncate(&s, 10).ends_with('…'));
        assert_eq!(truncate("ok", 10), "ok");
    }

    #[test]
    fn slug_is_filename_safe() {
        assert_eq!(slug("Claude Code"), "claude-code");
        assert_eq!(slug("cursor/vscode 1.0"), "cursor-vscode-1-0");
        assert_eq!(slug("  "), "unknown");
        assert_eq!(slug("!!!"), "unknown");
    }

    fn temp_root(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("grove_trace_{}_{tag}", std::process::id()))
    }

    #[test]
    fn writer_emits_the_four_event_kinds() {
        let root = temp_root("events");
        let _ = std::fs::remove_dir_all(&root);
        let meta = SessionMeta::new(
            "qwen3.5:4b", "standard", "ollama", "http://x/v1", "Claude Code", "1.2",
        );
        // The id carries the pid so concurrent same-second servers never collide.
        assert!(
            meta.session_id.ends_with(&format!("-{}", std::process::id())),
            "session_id ends with pid: {}",
            meta.session_id
        );
        let w = TraceWriter::open(&root, &meta, 50).expect("writer opens");
        let call = w.call_start("where is main");
        w.turn(
            call, 1,
            &json!({"model": "qwen3.5:4b", "messages": []}),
            &json!({"choices": [{"message": {"role": "assistant", "content": "here"}}]}),
            Some(Usage { prompt_tokens: 10, completion_tokens: 2, total_tokens: 12 }),
            42,
        );
        w.call_end(call, "here: src/main.rs:1", 1,
            Usage { prompt_tokens: 10, completion_tokens: 2, total_tokens: 12 }, 99, false);

        let path = traces_dir(&root).join(format!("{}.jsonl", meta.session_id));
        let body = std::fs::read_to_string(&path).unwrap();
        let kinds: Vec<String> = body
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap()["t"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(kinds, vec!["session", "call_start", "turn", "call_end"]);
        // Session header carries the client identity + model.
        let hdr: Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
        assert_eq!(hdr["client"]["name"], "Claude Code");
        assert_eq!(hdr["model"], "qwen3.5:4b");
        // The turn's usage is normalized to prompt/completion/total.
        let turn: Value = serde_json::from_str(body.lines().nth(2).unwrap()).unwrap();
        assert_eq!(turn["usage"]["total"], 12);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn retention_prunes_oldest_sessions() {
        let dir = temp_root("prune").join(".grove").join("traces");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Ten sessions with ascending epoch prefixes.
        for i in 100..110 {
            std::fs::write(dir.join(format!("{i}-client.jsonl")), "{}\n").unwrap();
        }
        // Keep 3 → prune leaves room for an incoming one, so 2 survive here.
        prune(&dir, 3);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, vec!["108-client.jsonl", "109-client.jsonl"]);
        std::fs::remove_dir_all(temp_root("prune")).ok();
    }

    #[test]
    fn retention_zero_keeps_all() {
        let dir = temp_root("keepall").join(".grove").join("traces");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            std::fs::write(dir.join(format!("{i}-c.jsonl")), "{}\n").unwrap();
        }
        prune(&dir, 0);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 5);
        std::fs::remove_dir_all(temp_root("keepall")).ok();
    }
}
