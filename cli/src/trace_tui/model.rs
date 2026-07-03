//! Data types + JSONL parsing for the trace browser (Elm-style Model layer).
//!
//! A trace file (`.grove/traces/<id>.jsonl`) is a stream of events — one
//! `session` header, then `call_start` / `turn` / `call_end` per explore call.
//! [`parse_session`] folds those events back into a [`Session`] › [`Call`] ›
//! [`Turn`] tree the browser navigates.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use grove_core::explore::trace::traces_dir;

/// Token counts (prompt, completion, total).
#[derive(Debug, Clone, Copy, Default)]
pub struct Tokens {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

impl Tokens {
    fn from_json(v: &Value) -> Self {
        Tokens {
            prompt: v.get("prompt").and_then(Value::as_u64).unwrap_or(0) as u32,
            completion: v.get("completion").and_then(Value::as_u64).unwrap_or(0) as u32,
            total: v.get("total").and_then(Value::as_u64).unwrap_or(0) as u32,
        }
    }
}

/// One model round-trip within a call. Per-turn token usage is carried inside
/// `response` (rendered by the response pretty-printer), so it isn't duplicated
/// here.
#[derive(Debug, Clone)]
pub struct Turn {
    pub turn_index: usize,
    pub request: Value,
    pub response: Value,
    pub wall_ms: u128,
}

/// One `explore` invocation.
#[derive(Debug, Clone)]
pub struct Call {
    pub call_id: u32,
    pub query: String,
    pub turns: usize,
    pub tokens: Tokens,
    pub wall_ms: u128,
    pub answer: String,
    pub truncated: bool,
    pub ended: bool,
    pub turn_blocks: Vec<Turn>,
}

/// One `grove serve` session.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub started_at: u64,
    pub client: String,
    pub model: String,
    pub mode: String,
    pub provider: String,
    pub base_url: String,
    pub calls: Vec<Call>,
    pub live: bool,
}

impl Session {
    /// Total tokens across every call in the session.
    pub fn total_tokens(&self) -> u32 {
        self.calls.iter().map(|c| c.tokens.total).sum()
    }
}

/// Which level of the drill-down is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Sessions,
    Calls,
    Detail,
}

/// High-level actions the event loop acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
}

/// Messages the event loop feeds to `update`.
#[derive(Debug, Clone)]
pub enum Msg {
    Up,
    Down,
    Enter,
    Back,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Quit,
    /// Event-loop refresh with a freshly reloaded session list.
    Reload(Vec<Session>),
}

/// The browser state.
#[derive(Debug, Clone)]
pub struct App {
    pub root: PathBuf,
    pub sessions: Vec<Session>,
    pub view: View,
    pub sel_session: usize,
    pub sel_call: usize,
    pub detail_scroll: usize,
    /// Whether the detail view sticks to the newest content (live follow).
    pub detail_follow: bool,
}

impl App {
    pub fn new(root: &Path) -> Self {
        App {
            root: root.to_path_buf(),
            sessions: load_sessions(root),
            view: View::Sessions,
            sel_session: 0,
            sel_call: 0,
            detail_scroll: 0,
            detail_follow: true,
        }
    }

    /// The session under the cursor, if any.
    pub fn current_session(&self) -> Option<&Session> {
        self.sessions.get(self.sel_session)
    }

    /// The call under the cursor, if any.
    pub fn current_call(&self) -> Option<&Call> {
        self.current_session()?.calls.get(self.sel_call)
    }
}

/// Load and parse every trace session under `root`, newest first.
pub fn load_sessions(root: &Path) -> Vec<Session> {
    let dir = traces_dir(root);
    let mut sessions: Vec<Session> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .filter_map(|e| parse_session(&e.path()))
            .collect(),
        Err(_) => Vec::new(),
    };
    // Newest first.
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    sessions
}

/// Parse a single trace file into a [`Session`] (folding its event stream).
/// Returns `None` if the file has no `session` header.
pub fn parse_session(path: &Path) -> Option<Session> {
    let body = std::fs::read_to_string(path).ok()?;
    let mut session: Option<Session> = None;

    for line in body.lines() {
        let ev: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // tolerate a torn final line
        };
        match ev.get("t").and_then(Value::as_str) {
            Some("session") => {
                let client_name = ev
                    .get("client")
                    .and_then(|c| c.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let client_ver = ev
                    .get("client")
                    .and_then(|c| c.get("version"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let client = if client_ver.is_empty() {
                    client_name.to_string()
                } else {
                    format!("{client_name} {client_ver}")
                };
                session = Some(Session {
                    id: str_field(&ev, "session_id"),
                    started_at: ev.get("started_at").and_then(Value::as_u64).unwrap_or(0),
                    client,
                    model: str_field(&ev, "model"),
                    mode: str_field(&ev, "mode"),
                    provider: str_field(&ev, "provider"),
                    base_url: str_field(&ev, "base_url"),
                    calls: Vec::new(),
                    live: false,
                });
            }
            Some("call_start") => {
                if let Some(s) = session.as_mut() {
                    s.calls.push(Call {
                        call_id: u32_field(&ev, "call_id"),
                        query: str_field(&ev, "query"),
                        turns: 0,
                        tokens: Tokens::default(),
                        wall_ms: 0,
                        answer: String::new(),
                        truncated: false,
                        ended: false,
                        turn_blocks: Vec::new(),
                    });
                }
            }
            Some("turn") => {
                if let Some(s) = session.as_mut() {
                    let cid = u32_field(&ev, "call_id");
                    if let Some(call) = s.calls.iter_mut().find(|c| c.call_id == cid) {
                        call.turn_blocks.push(Turn {
                            turn_index: ev.get("turn").and_then(Value::as_u64).unwrap_or(0) as usize,
                            request: ev.get("request").cloned().unwrap_or(Value::Null),
                            response: ev.get("response").cloned().unwrap_or(Value::Null),
                            wall_ms: ev.get("wall_ms").and_then(Value::as_u64).unwrap_or(0) as u128,
                        });
                    }
                }
            }
            Some("call_end") => {
                if let Some(s) = session.as_mut() {
                    let cid = u32_field(&ev, "call_id");
                    if let Some(call) = s.calls.iter_mut().find(|c| c.call_id == cid) {
                        call.turns = ev.get("turns").and_then(Value::as_u64).unwrap_or(0) as usize;
                        call.tokens = ev.get("tokens").map(Tokens::from_json).unwrap_or_default();
                        call.wall_ms = ev.get("wall_ms").and_then(Value::as_u64).unwrap_or(0) as u128;
                        call.answer = str_field(&ev, "answer");
                        call.truncated =
                            ev.get("truncated").and_then(Value::as_bool).unwrap_or(false);
                        call.ended = true;
                    }
                }
            }
            _ => {}
        }
    }

    let mut session = session?;
    session.live = is_live(path);
    Some(session)
}

/// A session is "live" when its file was written very recently (a running
/// `grove serve` is still appending to it).
fn is_live(path: &Path) -> bool {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    match modified {
        Some(m) => SystemTime::now()
            .duration_since(m)
            .map(|d| d.as_secs() < 15)
            .unwrap_or(false),
        None => false,
    }
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn u32_field(v: &Value, key: &str) -> u32 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0) as u32
}

/// Format an epoch-seconds timestamp as `HH:MM:SS` (UTC), for compact display.
pub fn hms_utc(epoch_secs: u64) -> String {
    let s = epoch_secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_session(dir: &Path, name: &str, lines: &[Value]) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn parse_folds_events_into_a_call_tree() {
        use serde_json::json;
        let dir = std::env::temp_dir().join(format!("grove_tracetui_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = write_session(
            &dir,
            "100-claude-code.jsonl",
            &[
                json!({"t":"session","session_id":"100-claude-code","started_at":100,
                    "client":{"name":"Claude Code","version":"1.0"},"model":"qwen","mode":"standard",
                    "provider":"ollama","base_url":"http://x/v1"}),
                json!({"t":"call_start","call_id":1,"query":"where is main"}),
                json!({"t":"turn","call_id":1,"turn":1,"request":{"messages":[]},
                    "response":{"choices":[]},"usage":{"prompt":10,"completion":2,"total":12},"wall_ms":40}),
                json!({"t":"call_end","call_id":1,"turns":1,
                    "tokens":{"prompt":10,"completion":2,"total":12},"wall_ms":50,
                    "answer":"src/main.rs:1","truncated":false}),
            ],
        );
        let s = parse_session(&path).expect("session parses");
        assert_eq!(s.client, "Claude Code 1.0");
        assert_eq!(s.model, "qwen");
        assert_eq!(s.calls.len(), 1);
        let c = &s.calls[0];
        assert_eq!(c.query, "where is main");
        assert_eq!(c.turns, 1);
        assert_eq!(c.tokens.total, 12);
        assert_eq!(c.turn_blocks.len(), 1);
        assert!(c.ended);
        assert_eq!(s.total_tokens(), 12);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_tolerates_torn_final_line_and_missing_end() {
        use serde_json::json;
        let dir = std::env::temp_dir().join(format!("grove_tracetui_torn_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("200-c.jsonl");
        // A header + an in-flight call whose turn line was cut off mid-write.
        std::fs::write(
            &path,
            format!(
                "{}\n{}\n{{\"t\":\"turn\",\"call_id\":1,\"req",
                json!({"t":"session","session_id":"200-c","started_at":200,
                    "client":{"name":"x","version":""},"model":"m","mode":"standard",
                    "provider":"ollama","base_url":"u"}),
                json!({"t":"call_start","call_id":1,"query":"q"}),
            ),
        )
        .unwrap();
        let s = parse_session(&path).expect("still parses despite torn line");
        assert_eq!(s.calls.len(), 1);
        assert!(!s.calls[0].ended, "call never ended");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hms_is_utc_time_of_day() {
        assert_eq!(hms_utc(0), "00:00:00");
        assert_eq!(hms_utc(3661), "01:01:01");
        assert_eq!(hms_utc(86_400 + 60), "00:01:00");
    }
}
