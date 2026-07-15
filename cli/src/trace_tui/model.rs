//! Data types + JSONL parsing for the trace browser (Elm-style Model layer).
//!
//! A trace file (`.grove/traces/<id>.jsonl`) is a stream of events — one
//! `session` header, then `call_start` / `turn` / `call_end` per explore call.
//! [`parse_session`] folds those events back into a [`Session`] › [`Call`] ›
//! [`Turn`] tree the browser navigates.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use grove_core::explore::trace::{format_response, request_parts, traces_dir};

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
    /// Expand (→) — in the detail tree; descends a level elsewhere.
    Right,
    /// Collapse (←) — in the detail tree; goes back a level elsewhere.
    Left,
    Back,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Quit,
    /// Event-loop refresh with a freshly reloaded session list.
    Reload(Vec<Session>),
}

/// A node in the per-call turn tree. `usize` is the turn index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKey {
    /// A turn header (expands to its request/response nodes).
    Turn(usize),
    /// A turn's request body (expands to the tiered system/context/prompt view).
    Req(usize),
    /// The (collapsed-by-default) system prompt under a turn's request.
    ReqSystem(usize),
    /// The (collapsed-by-default) prior-exchange context under a turn's request.
    ReqContext(usize),
    /// A turn's response body (expands to the pretty-printed completion).
    Resp(usize),
    /// The call's final answer (expands to the grounded text).
    Answer,
}

/// One rendered row of the turn tree. `key` is `Some` for a selectable node,
/// `None` for an indented content line under an expanded node.
#[derive(Debug, Clone)]
pub struct TreeRow {
    pub key: Option<NodeKey>,
    pub depth: u16,
    pub text: String,
    pub expandable: bool,
    pub expanded: bool,
}

/// Flatten a call into the visible tree rows, honouring the `expanded` set.
/// Turn headers and the answer are always present; their request/response and
/// content lines appear only when their parent node is expanded.
pub fn build_tree_rows(c: &Call, expanded: &HashSet<NodeKey>) -> Vec<TreeRow> {
    let mut rows = Vec::new();

    for t in &c.turn_blocks {
        let key = NodeKey::Turn(t.turn_index);
        let open = expanded.contains(&key);
        rows.push(TreeRow {
            key: Some(key),
            depth: 0,
            expandable: true,
            expanded: open,
            text: format!("turn {} — {}  ({}ms)", t.turn_index, turn_summary(&t.response), t.wall_ms),
        });
        if !open {
            continue;
        }
        push_request(&mut rows, t.turn_index, &t.request, expanded);
        push_body(
            &mut rows,
            NodeKey::Resp(t.turn_index),
            "response",
            &format_response(&t.response, Some(t.wall_ms)),
            expanded,
        );
    }

    // Final answer (or a live placeholder).
    if c.ended {
        let key = NodeKey::Answer;
        let open = expanded.contains(&key);
        rows.push(TreeRow {
            key: Some(key),
            depth: 0,
            expandable: true,
            expanded: open,
            text: "final answer".to_string(),
        });
        if open {
            let ans = if c.answer.is_empty() { "(empty)" } else { &c.answer };
            for line in ans.lines() {
                rows.push(content_row(line, 1));
            }
        }
    } else {
        rows.push(TreeRow {
            key: Some(NodeKey::Answer),
            depth: 0,
            expandable: false,
            expanded: false,
            text: "● call in progress — no final answer yet".to_string(),
        });
    }

    rows
}

/// Push a turn's `request` as a tiered node: the collapsed system prompt and
/// collapsed prior-exchange context are tucked into their own sub-nodes, while
/// the prompt — the newest messages the model is acting on — is shown inline at
/// the bottom whenever the request node is open. This keeps the big, repeated
/// system prompt and growing history out of the way and surfaces what actually
/// changed this turn.
fn push_request(rows: &mut Vec<TreeRow>, turn: usize, request: &Value, expanded: &HashSet<NodeKey>) {
    let req_key = NodeKey::Req(turn);
    let open = expanded.contains(&req_key);
    let parts = request_parts(request);
    rows.push(TreeRow {
        key: Some(req_key),
        depth: 1,
        expandable: true,
        expanded: open,
        text: format!("request   {}", parts.header),
    });
    if !open {
        return;
    }

    // System prompt — its own collapsible node, collapsed by default.
    if !parts.system.is_empty() {
        push_collapsed_section(
            rows,
            NodeKey::ReqSystem(turn),
            "system prompt".to_string(),
            &parts.system,
            expanded,
        );
    }
    // Prior-exchange context — collapsible, collapsed by default. Omitted on the
    // first turn, where there is no prior exchange.
    if parts.context_msgs > 0 {
        let label = format!(
            "context · {} prior {}",
            parts.context_msgs,
            if parts.context_msgs == 1 { "msg" } else { "msgs" }
        );
        push_collapsed_section(rows, NodeKey::ReqContext(turn), label, &parts.context, expanded);
    }
    // The prompt — shown inline at the bottom whenever the request is open.
    let prompt = if parts.prompt.is_empty() { "(none)" } else { &parts.prompt };
    for line in prompt.lines() {
        rows.push(content_row(line, 2));
    }
}

/// Push a collapsible section node (depth 2) that reveals its pre-rendered body
/// as content rows (depth 3) only when expanded — collapsed by default.
fn push_collapsed_section(
    rows: &mut Vec<TreeRow>,
    key: NodeKey,
    label: String,
    body: &str,
    expanded: &HashSet<NodeKey>,
) {
    let open = expanded.contains(&key);
    rows.push(TreeRow {
        key: Some(key),
        depth: 2,
        expandable: true,
        expanded: open,
        text: label,
    });
    if open {
        for line in body.lines() {
            rows.push(content_row(line, 3));
        }
    }
}

/// Push a collapsible body node (request/response) and, when open, its
/// pretty-printed content as indented content rows.
fn push_body(rows: &mut Vec<TreeRow>, key: NodeKey, label: &str, body: &str, expanded: &HashSet<NodeKey>) {
    let open = expanded.contains(&key);
    rows.push(TreeRow {
        key: Some(key),
        depth: 1,
        expandable: true,
        expanded: open,
        text: label.to_string(),
    });
    if open {
        for line in body.lines() {
            rows.push(content_row(line, 2));
        }
    }
}

fn content_row(text: &str, depth: u16) -> TreeRow {
    TreeRow { key: None, depth, text: text.to_string(), expandable: false, expanded: false }
}

/// A compact one-line summary of what the model produced on a turn: the tool
/// calls it requested, or a snippet of its text, for the collapsed turn header.
fn turn_summary(resp: &Value) -> String {
    let msg = resp
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"));
    let Some(msg) = msg else {
        return "(no output)".to_string();
    };

    if let Some(calls) = msg.get("tool_calls").and_then(Value::as_array) {
        if !calls.is_empty() {
            let names: Vec<String> = calls
                .iter()
                .map(|c| c.get("function").and_then(|f| f.get("name")).and_then(Value::as_str).unwrap_or("?").to_string())
                .collect();
            return format!("calls: {}", names.join(", "));
        }
    }
    match msg.get("content").and_then(Value::as_str) {
        Some(s) if !s.trim().is_empty() => {
            let snippet: String = s.trim().chars().take(60).collect();
            format!("“{snippet}”")
        }
        _ => "(no output)".to_string(),
    }
}

/// The browser state.
#[derive(Debug, Clone)]
pub struct App {
    pub root: PathBuf,
    pub sessions: Vec<Session>,
    pub view: View,
    pub sel_session: usize,
    pub sel_call: usize,
    /// Cursor over the selectable nodes of the detail turn tree.
    pub tree_cursor: usize,
    /// Which tree nodes are expanded in the detail view.
    pub expanded: HashSet<NodeKey>,
}

impl App {
    pub fn new(root: &Path) -> Self {
        App {
            root: root.to_path_buf(),
            sessions: load_sessions(root),
            view: View::Sessions,
            sel_session: 0,
            sel_call: 0,
            tree_cursor: 0,
            expanded: HashSet::new(),
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

    /// The selectable node keys of the current call's turn tree, in row order.
    pub fn selectable_keys(&self) -> Vec<NodeKey> {
        match self.current_call() {
            Some(c) => build_tree_rows(c, &self.expanded)
                .into_iter()
                .filter_map(|r| r.key)
                .collect(),
            None => Vec::new(),
        }
    }

    /// The node key under the tree cursor, if any.
    pub fn cursor_key(&self) -> Option<NodeKey> {
        self.selectable_keys().get(self.tree_cursor).copied()
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
    sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));
    sessions
}

/// Parse a single trace file into a [`Session`] (folding its event stream).
/// Returns `None` only when neither a `session` header nor the filename yields
/// usable session identity. A torn or interleaved header (e.g. two servers that
/// collided on one file) does *not* discard the session: it is seeded from the
/// filename so its calls still render.
pub fn parse_session(path: &Path) -> Option<Session> {
    let body = std::fs::read_to_string(path).ok()?;
    // Seed from the filename first, so calls survive even if the header line is
    // unparseable. A valid `session` header below enriches this in place.
    let mut session: Option<Session> = session_from_filename(path);

    for line in body.lines() {
        let ev: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // tolerate a torn/interleaved line
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
                let header = Session {
                    id: str_field(&ev, "session_id"),
                    started_at: ev.get("started_at").and_then(Value::as_u64).unwrap_or(0),
                    client,
                    model: str_field(&ev, "model"),
                    mode: str_field(&ev, "mode"),
                    provider: str_field(&ev, "provider"),
                    base_url: str_field(&ev, "base_url"),
                    calls: Vec::new(),
                    live: false,
                };
                // Adopt the real header's metadata but keep any calls already
                // folded in (the header is line 1 in practice, so this is empty
                // then — it only matters for a header that arrived out of order).
                match session.as_mut() {
                    Some(s) => {
                        let calls = std::mem::take(&mut s.calls);
                        *s = header;
                        s.calls = calls;
                    }
                    None => session = Some(header),
                }
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

/// Reconstruct a bare [`Session`] from a trace filename
/// (`<epoch_secs>-<client-slug>[-<pid>].jsonl`), used as a fallback when the
/// header line is torn or interleaved. Recovers the start time and client slug;
/// model/mode/provider stay blank until (if ever) a valid header is folded in.
/// Returns `None` if the stem has no leading epoch prefix.
fn session_from_filename(path: &Path) -> Option<Session> {
    let stem = path.file_stem()?.to_str()?;
    let (epoch, rest) = stem.split_once('-')?;
    let started_at = epoch.parse::<u64>().ok()?;
    Some(Session {
        id: stem.to_string(),
        started_at,
        client: rest.to_string(),
        model: String::new(),
        mode: String::new(),
        provider: String::new(),
        base_url: String::new(),
        calls: Vec::new(),
        live: false,
    })
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
    fn parse_recovers_calls_when_header_is_torn() {
        use serde_json::json;
        let dir = std::env::temp_dir().join(format!("grove_tracetui_torn_hdr_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Header line is garbage (two servers colliding interleaved their writes),
        // but the call events below it are intact.
        let path = dir.join("1784091750-claude-code-42.jsonl");
        std::fs::write(
            &path,
            format!(
                "{{\"base_url\":\"http{{\"garbled\": interleaved}}\n{}\n{}\n{}\n",
                json!({"t":"call_start","call_id":1,"query":"where is auth"}),
                json!({"t":"turn","call_id":1,"turn":1,"request":{"messages":[]},
                    "response":{"choices":[]},"usage":{"prompt":5,"completion":1,"total":6},"wall_ms":20}),
                json!({"t":"call_end","call_id":1,"turns":1,
                    "tokens":{"prompt":5,"completion":1,"total":6},"wall_ms":30,
                    "answer":"js:auth.js#login@10","truncated":false}),
            ),
        )
        .unwrap();
        let s = parse_session(&path).expect("session recovered from filename");
        // Identity comes from the filename, not the unparseable header.
        assert_eq!(s.started_at, 1784091750);
        assert_eq!(s.client, "claude-code-42");
        // The real work survives instead of the whole session vanishing.
        assert_eq!(s.calls.len(), 1, "call is not lost to the torn header");
        assert_eq!(s.calls[0].query, "where is auth");
        assert_eq!(s.calls[0].turn_blocks.len(), 1);
        assert!(s.calls[0].ended);
        assert_eq!(s.total_tokens(), 6);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hms_is_utc_time_of_day() {
        assert_eq!(hms_utc(0), "00:00:00");
        assert_eq!(hms_utc(3661), "01:01:01");
        assert_eq!(hms_utc(86_400 + 60), "00:01:00");
    }

    fn call_with_turns(n: usize, ended: bool) -> Call {
        use serde_json::json;
        Call {
            call_id: 1,
            query: "q".into(),
            turns: n,
            tokens: Tokens::default(),
            wall_ms: 0,
            answer: "ans".into(),
            truncated: false,
            ended,
            turn_blocks: (1..=n)
                .map(|i| Turn {
                    turn_index: i,
                    // A realistic request: system prompt, the original question,
                    // a prior assistant tool-call, and the fresh tool result the
                    // turn is acting on (the trailing block).
                    request: json!({"model":"qwen","tools":[{"type":"function"}],"messages":[
                        {"role":"system","content":"You are a code locator."},
                        {"role":"user","content":"where is main"},
                        {"role":"assistant","tool_calls":[{"function":{"name":"Grep","arguments":"{}"}}]},
                        {"role":"tool","content":"src/main.rs:1: fn main()"}
                    ]}),
                    response: json!({"choices":[{"message":{"role":"assistant",
                        "tool_calls":[{"function":{"name":"Grove"}}]}}]}),
                    wall_ms: 10,
                })
                .collect(),
        }
    }

    #[test]
    fn tree_collapsed_shows_turn_headers_and_answer() {
        let c = call_with_turns(3, true);
        let rows = build_tree_rows(&c, &HashSet::new());
        let keys: Vec<NodeKey> = rows.iter().filter_map(|r| r.key).collect();
        assert_eq!(
            keys,
            vec![NodeKey::Turn(1), NodeKey::Turn(2), NodeKey::Turn(3), NodeKey::Answer]
        );
        // The collapsed turn header summarizes the tool call the model made.
        assert!(rows[0].text.contains("calls: Grove"), "turn summary: {}", rows[0].text);
        assert!(rows.iter().all(|r| !r.expanded), "all collapsed");
    }

    #[test]
    fn expanding_a_turn_reveals_request_and_response_nodes() {
        let c = call_with_turns(1, true);
        let mut exp = HashSet::new();
        exp.insert(NodeKey::Turn(1));
        let rows = build_tree_rows(&c, &exp);
        let keys: Vec<NodeKey> = rows.iter().filter_map(|r| r.key).collect();
        assert_eq!(keys, vec![NodeKey::Turn(1), NodeKey::Req(1), NodeKey::Resp(1), NodeKey::Answer]);
        // Expanding the request node reveals indented content lines (no key).
        exp.insert(NodeKey::Req(1));
        let rows = build_tree_rows(&c, &exp);
        assert!(rows.iter().any(|r| r.key.is_none() && r.depth == 2), "request content shown");
    }

    #[test]
    fn expanded_request_tiers_system_context_and_inline_prompt() {
        let c = call_with_turns(1, true);
        let mut exp = HashSet::new();
        exp.insert(NodeKey::Turn(1));
        exp.insert(NodeKey::Req(1));
        let rows = build_tree_rows(&c, &exp);

        // System and context are their own collapsible nodes (collapsed → no
        // content leaking), sitting above the inline prompt.
        assert!(
            rows.iter().any(|r| r.key == Some(NodeKey::ReqSystem(1)) && r.expandable && !r.expanded),
            "system prompt is a collapsed sub-node"
        );
        let ctx = rows.iter().find(|r| r.key == Some(NodeKey::ReqContext(1))).expect("context node");
        assert!(ctx.text.contains("2 prior msgs"), "context labels its size: {}", ctx.text);
        assert!(!ctx.expanded, "context collapsed by default");

        // The collapsed sections hide their bodies; the only visible content
        // rows are the prompt — the trailing tool result — shown inline.
        let content: Vec<&str> = rows.iter().filter(|r| r.key.is_none()).map(|r| r.text.as_str()).collect();
        assert!(content.iter().any(|l| l.contains("[tool]") && l.contains("src/main.rs:1")),
            "prompt (trailing tool result) shown inline: {content:?}");
        assert!(!content.iter().any(|l| l.contains("You are a code locator")),
            "system body stays hidden while collapsed: {content:?}");

        // Expanding the system node reveals its body.
        exp.insert(NodeKey::ReqSystem(1));
        let rows = build_tree_rows(&c, &exp);
        assert!(rows.iter().any(|r| r.key.is_none() && r.depth == 3 && r.text.contains("You are a code locator")),
            "system body shown when expanded");
    }

    #[test]
    fn in_progress_call_shows_live_answer_placeholder() {
        let c = call_with_turns(1, false);
        let rows = build_tree_rows(&c, &HashSet::new());
        let answer = rows.iter().find(|r| r.key == Some(NodeKey::Answer)).unwrap();
        assert!(!answer.expandable, "no answer to expand yet");
        assert!(answer.text.contains("in progress"), "live placeholder: {}", answer.text);
    }
}
