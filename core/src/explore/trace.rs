//! Shared pretty-printer for explore-mode LLM traffic.
//!
//! Used by both taps:
//! - the **in-process tap** (`ExploreConfig::tap`) — the agent loop appends each
//!   request/response here to `.grove/explore-trace.log`, no proxy needed;
//! - the standalone **`grove tap`** proxy — formats the raw wire bodies it relays.
//!
//! Input is an OpenAI chat-completions request or response body as a
//! [`serde_json::Value`]; output is a compact, human-readable block.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The in-process trace log path for a project rooted at `root`.
pub fn trace_path(root: &Path) -> PathBuf {
    root.join(".grove").join("explore-trace.log")
}

/// Append a block to the trace log (best-effort — tracing must never break a
/// run). Creates `.grove/` and the file as needed.
pub fn append(root: &Path, block: &str) {
    let path = trace_path(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{block}");
    }
}

/// Format a chat-completions **request** body into a readable block.
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
        let role = m.get("role").and_then(Value::as_str).unwrap_or("?");
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
    out
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
}
