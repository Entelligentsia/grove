//! The inner explorer loop — the `base-q4-v2-hf` reference harness
//! (`grove-explore-model/scripts/run_eval.py::run_question`, interim winner at
//! 80.6 on the 347-case holdout, served on llama.cpp).
//!
//! It is a **single-phase** bounded loop: every turn offers the full reference
//! toolset ([`toolset::all_tools`]) and the model explores until it emits a
//! tool-less final message (the answer — bare location lines) or a backstop
//! fires. There is no plan-first recon phase, no `submit_plan`, and no
//! merit/strict/plan-first arm selection — the output contract and steering are
//! the flat v2 prompt ([`steering::system_prompt`]).
//!
//! Backstops (any ends exploration and triggers the forced-answer turn):
//! - **turn cap** ([`MAX_TURNS`]),
//! - **thrash** — `≥ THRASH_LIMIT` consecutive unproductive calls, where a
//!   verbatim duplicate counts `+1` but a *novel* query that returns empty counts
//!   `+0.5` (H5: legitimate query-pivoting isn't punished as a loop),
//! - **context-token budget** ([`TOKEN_BUDGET`]) and **wall-time budget**
//!   ([`TIME_BUDGET_SECS`]).
//!
//! Convergence + recovery, all model-visible (H1/H2/H3/H4):
//! - a soft [`steering::NUDGE`] a couple of calls before the cap,
//! - a forced [`steering::FORCE_FINAL_ANSWER`] turn (no empty-exit offer, carries
//!   a concrete example) when the loop stops without an answer,
//! - **retry-on-leak**: a malformed (leaked) tool call is re-prompted rather than
//!   accepted as an answer ([`RETRY_ON_LEAK`]), and every observation is
//!   [`grounding::neutralize_xml`]-ed so source markup can't 500 the template.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use serde_json::{json, Value};

use super::client::{ChatClient, ClientError};
use super::wire::{ChatRequest, Message, Usage};
use super::config::ExploreConfig;
use super::trace::TraceWriter;
use super::{grounding, steering, toolset};

// --- Reference harness constants (run_eval.py defaults for the winning arm) --

/// Turn-cap backstop (`--max-turns`, winning arm: 12).
pub const MAX_TURNS: usize = 12;
/// Per-completion generation cap (`--max-tokens`).
const MAX_COMPLETION_TOKENS: u32 = 1024;
/// Sampling temperature (`--temp`).
const TEMPERATURE: f32 = 0.0;
/// Model thinking (`--think`, winning arm: on).
const THINK: bool = true;
/// Stop after this many consecutive unproductive tool calls (`--thrash-limit`).
const THRASH_LIMIT: f64 = 3.0;
/// Inject the soft wrap-up nudge when this many calls remain (`--nudge-at`).
const NUDGE_AT: usize = 2;
/// Context-size backstop: stop when a request's prompt tokens exceed this
/// (`--token-budget`).
const TOKEN_BUDGET: u32 = 28_000;
/// Per-question wall-clock cap in seconds (`--time-budget`).
const TIME_BUDGET_SECS: u64 = 90;
/// On a malformed (leaked) tool call, re-prompt up to this many times rather than
/// terminating (`--retry-on-leak`, winning arm: 1).
const RETRY_ON_LEAK: u32 = 1;

// ---------------------------------------------------------------------------
// Public types (unchanged contract consumed by cli/src/mcp.rs)
// ---------------------------------------------------------------------------

/// The successful result of an exploration run.
#[derive(Debug, Clone)]
pub struct ExploreAnswer {
    /// The grounded answer — bare location lines, FS-validated (see
    /// [`grounding::get_final_answer`]).
    pub text: String,
    /// The number of turns consumed.
    pub turns: usize,
    /// True when the answer came from the forced-answer / backstop path rather
    /// than a voluntary tool-less final message.
    pub truncated: bool,
}

/// An error from the exploration run.
#[derive(Debug)]
pub enum ExploreError {
    /// The inference server was unreachable / refused / timed out (D3).
    ProviderDown {
        /// The endpoint that could not be reached.
        url: String,
        /// The transport-level detail.
        detail: String,
    },
    /// Any other [`ClientError`] (HTTP error, protocol error, encode error).
    Client(String),
}

impl fmt::Display for ExploreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExploreError::ProviderDown { url, detail } => {
                write!(f, "inference server unreachable at {url}: {detail}")
            }
            ExploreError::Client(msg) => write!(f, "chat client error: {msg}"),
        }
    }
}

impl std::error::Error for ExploreError {}

fn map_client_error(e: ClientError) -> ExploreError {
    match e {
        ClientError::Connection { url, detail } => ExploreError::ProviderDown { url, detail },
        other => ExploreError::Client(other.to_string()),
    }
}

/// A sink for per-turn progress, so a long delegated call can report liveness to
/// the waiting client (MCP `notifications/progress`).
pub trait ProgressReporter {
    /// Report progress at `progress` of `total`, with a status `message`.
    fn report(&self, progress: usize, total: usize, message: &str);
}

/// A reporter that drops every update (the default for [`run_explore`] and tests).
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn report(&self, _progress: usize, _total: usize, _message: &str) {}
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/// Explore `question` over `root`, delegating to the local model via `client`.
/// Convenience wrapper over [`run_explore_reporting`] with no progress sink.
pub fn run_explore(
    question: &str,
    root: &Path,
    cfg: &ExploreConfig,
    client: &dyn ChatClient,
) -> Result<ExploreAnswer, ExploreError> {
    run_explore_reporting(question, root, cfg, client, &NoopReporter, None)
}

/// Explore `question` over `root`, reporting per-turn progress. Direct port of
/// `run_eval.py::run_question` (the winning `harness_fixes`-on arm).
///
/// When `trace` is `Some`, each model round-trip is recorded to the session's
/// structured trace; `None` disables tracing with zero overhead.
pub fn run_explore_reporting(
    question: &str,
    root: &Path,
    cfg: &ExploreConfig,
    client: &dyn ChatClient,
    progress: &dyn ProgressReporter,
    trace: Option<&TraceWriter>,
) -> Result<ExploreAnswer, ExploreError> {
    let total = MAX_TURNS + 2;
    let sys = steering::system_prompt(cfg.steering, root);
    let mut messages: Vec<Message> = vec![Message::system(sys), Message::user(question)];

    let call_id = trace.map(|tw| tw.call_start(question)).unwrap_or(0);
    let call_t0 = std::time::Instant::now();
    let mut agg = Usage::default();

    let mut seen_sigs: HashSet<String> = HashSet::new();
    let mut consec_unprod: f64 = 0.0;
    let mut nudged = false;
    let mut leak_retries_left = RETRY_ON_LEAK;
    let mut ctx_tokens: u32 = 0;
    let mut answer_raw: Option<String> = None;
    let mut truncated = true; // set false only on a voluntary tool-less final turn
    let mut turns_used = 0usize;
    let mut activity = "exploring the codebase".to_string();

    for turn in 0..MAX_TURNS {
        turns_used = turn + 1;
        // Don't START a new turn past the wall budget (a slow generation can't be
        // interrupted, but we won't pile another on top).
        if turn > 0 && call_t0.elapsed().as_secs() > TIME_BUDGET_SECS {
            break;
        }
        progress.report(
            turn + 1,
            total,
            &format!("turn {}/{MAX_TURNS} · {activity}", turn + 1),
        );

        let req = ChatRequest::new(messages.clone())
            .with_tools(toolset::all_tools())
            .with_tool_choice(json!("auto"))
            .with_explore_sampling(TEMPERATURE, MAX_COMPLETION_TOKENS, THINK);
        let req_trace = trace.map(|_| request_trace(&req, &cfg.model));
        let t0 = std::time::Instant::now();
        let resp = client.chat(req).map_err(map_client_error)?;
        let wall = t0.elapsed().as_millis();
        if let Some(u) = resp.usage {
            agg.prompt_tokens = agg.prompt_tokens.saturating_add(u.prompt_tokens);
            agg.completion_tokens = agg.completion_tokens.saturating_add(u.completion_tokens);
            agg.total_tokens = agg.total_tokens.saturating_add(u.total_tokens);
            if u.prompt_tokens > 0 {
                ctx_tokens = u.prompt_tokens;
            }
        }
        if let (Some(tw), Some(req_v)) = (trace, &req_trace) {
            let resp_v = serde_json::to_value(&resp).unwrap_or(Value::Null);
            tw.turn(call_id, turn + 1, req_v, &resp_v, resp.usage, wall);
        }

        let step = match resp.first_message() {
            Some(m) => m.clone(),
            None => break,
        };
        let content = step.content.clone().unwrap_or_default();

        if step.tool_calls.is_empty() {
            // A malformed tool call that leaked into content is NOT an answer:
            // re-prompt to re-issue it properly and keep the loop alive (E1/H4).
            if leak_retries_left > 0
                && grounding::has_leak(&content)
                && grounding::extract_final(&content).is_none()
            {
                leak_retries_left -= 1;
                messages.push(Message::assistant(grounding::neutralize_xml(&content)));
                messages.push(Message::user(steering::LEAK_RETRY));
                continue;
            }
            // Voluntary final answer.
            if grounding::extract_final(&content).is_some() {
                truncated = false;
            }
            answer_raw = Some(content);
            break;
        }

        // Record the assistant's tool-call turn (content neutralized — the model
        // can emit template-breaking tags in its own reasoning).
        let mut step = step.clone();
        step.content = Some(grounding::neutralize_xml(&content));
        messages.push(step.clone());

        // Dispatch each call, feed neutralized observations back, and score
        // productivity for the thrash detector.
        for c in &step.tool_calls {
            let obs = grounding::neutralize_xml(&toolset::dispatch(&c.name, &c.arguments, root));
            let sig = format!("{}:{}", c.name, compact_args(&c.arguments));
            let is_dup = !seen_sigs.insert(sig);
            let is_empty = toolset::is_empty_obs(&obs);
            if is_dup {
                consec_unprod += 1.0; // verbatim duplicate — full thrash
            } else if is_empty {
                consec_unprod += 0.5; // H5: novel-but-empty probe — half thrash
            } else {
                consec_unprod = 0.0;
            }
            messages.push(Message::tool(&c.id, obs));
        }
        activity = summarize_activity(&step.tool_calls);

        // Backstops (any triggers the forced-answer turn).
        if consec_unprod >= THRASH_LIMIT
            || ctx_tokens >= TOKEN_BUDGET
            || call_t0.elapsed().as_secs() > TIME_BUDGET_SECS
            || turn >= MAX_TURNS - 1
        {
            break;
        }

        // Soft wrap-up nudge once, a couple of calls before the cap.
        if !nudged && (MAX_TURNS - 1 - turn) <= NUDGE_AT {
            messages.push(Message::user(steering::NUDGE));
            nudged = true;
        }
    }

    // Forced-answer turn: if the loop stopped without an answer, elicit one with
    // no tools. Retry on a persistent leak up to RETRY_ON_LEAK times.
    if answer_raw.is_none() {
        messages.push(Message::user(steering::FORCE_FINAL_ANSWER));
        for att in 0..=RETRY_ON_LEAK {
            progress.report(total - 1, total, "wrapping up: final answer");
            let req = ChatRequest::new(messages.clone()).with_explore_sampling(
                TEMPERATURE,
                MAX_COMPLETION_TOKENS.max(1024),
                THINK,
            );
            let req_trace = trace.map(|_| request_trace(&req, &cfg.model));
            let t0 = std::time::Instant::now();
            let resp = client.chat(req).map_err(map_client_error)?;
            let wall = t0.elapsed().as_millis();
            if let Some(u) = resp.usage {
                agg.prompt_tokens = agg.prompt_tokens.saturating_add(u.prompt_tokens);
                agg.completion_tokens = agg.completion_tokens.saturating_add(u.completion_tokens);
                agg.total_tokens = agg.total_tokens.saturating_add(u.total_tokens);
            }
            if let (Some(tw), Some(req_v)) = (trace, &req_trace) {
                let resp_v = serde_json::to_value(&resp).unwrap_or(Value::Null);
                tw.turn(call_id, turns_used + 1, req_v, &resp_v, resp.usage, wall);
            }
            let fcontent = resp
                .first_message()
                .and_then(|m| m.content.clone())
                .unwrap_or_default();
            let salvaged = grounding::extract_final(&fcontent);
            if grounding::has_leak(&fcontent) && salvaged.is_none() && att < RETRY_ON_LEAK {
                messages.push(Message::assistant(grounding::neutralize_xml(&fcontent)));
                messages.push(Message::user(steering::FORCE_STRICT));
                continue;
            }
            answer_raw = salvaged.is_some().then_some(fcontent);
            break;
        }
    }

    progress.report(total, total, "grounding answer");
    let text = answer_raw
        .map(|c| grounding::get_final_answer(&c, root))
        .unwrap_or_default();
    if let Some(tw) = trace {
        tw.call_end(call_id, &text, turns_used, agg, call_t0.elapsed().as_millis(), truncated);
    }
    Ok(ExploreAnswer { text, turns: turns_used, truncated })
}

/// Snapshot a request body for the trace, filling in the `model` the client sets
/// at send-time.
fn request_trace(req: &ChatRequest, model: &str) -> Value {
    let mut v = serde_json::to_value(req).unwrap_or(Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("model".to_string(), Value::String(model.to_string()));
    }
    v
}

/// A stable signature of a tool call's arguments for duplicate detection.
fn compact_args(args: &Value) -> String {
    if args.is_null() {
        String::new()
    } else {
        serde_json::to_string(args).unwrap_or_default()
    }
}

/// A short human-facing summary of a turn's tool activity, for the next progress
/// tick (e.g. "grove symbols, Read query.py").
fn summarize_activity(calls: &[super::wire::ToolCall]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in calls {
        let part = match c.name.as_str() {
            toolset::READ => format!("Read {}", basename_arg(&c.arguments, "file_path")),
            toolset::GLOB => format!("Glob {}", str_arg(&c.arguments, "pattern")),
            toolset::GREP => format!("Grep {}", str_arg(&c.arguments, "pattern")),
            other => format!("grove {}", other.trim_start_matches("mcp__grove__")),
        };
        parts.push(part);
    }
    let joined = parts.join(", ");
    let s: String = if joined.chars().count() > 80 {
        let mut t: String = joined.chars().take(77).collect();
        t.push('…');
        t
    } else {
        joined
    };
    if s.is_empty() {
        "exploring the codebase".to_string()
    } else {
        s
    }
}

fn str_arg(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(30)
        .collect()
}

fn basename_arg(args: &Value, key: &str) -> String {
    let p = args.get(key).and_then(Value::as_str).unwrap_or("");
    Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::wire::{ChatResponse, Choice, Role, ToolCall};
    use crate::explore::config::{Provider, Steering};
    use std::cell::RefCell;

    /// A scripted client returning canned responses in order.
    struct FakeClient {
        scripted: RefCell<std::collections::VecDeque<ChatResponse>>,
        seen_tool_names: RefCell<Vec<Vec<String>>>,
    }

    impl FakeClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            FakeClient {
                scripted: RefCell::new(responses.into()),
                seen_tool_names: RefCell::new(Vec::new()),
            }
        }
    }

    impl ChatClient for FakeClient {
        fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ClientError> {
            self.seen_tool_names
                .borrow_mut()
                .push(req.tools.iter().map(|t| t.function.name.clone()).collect());
            Ok(self
                .scripted
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| text_response("(end)")))
        }
    }

    fn text_response(s: &str) -> ChatResponse {
        ChatResponse {
            choices: vec![Choice {
                message: Message {
                    role: Role::Assistant,
                    content: Some(s.to_string()),
                    tool_calls: vec![],
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn tool_call_response(name: &str, args: Value) -> ChatResponse {
        ChatResponse {
            choices: vec![Choice {
                message: Message {
                    role: Role::Assistant,
                    content: None,
                    tool_calls: vec![ToolCall {
                        id: "call_1".into(),
                        name: name.into(),
                        arguments: args,
                    }],
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn cfg() -> ExploreConfig {
        ExploreConfig {
            provider: Provider::LlamaCpp,
            base_url: "http://localhost:8080/v1".into(),
            model: "qwen3.5-4b".into(),
            steering: Steering::Standard,
            allowed_tools: vec!["grove".into(), "rg".into()],
            tap: false,
            trace_retain: 50,
        }
    }

    #[test]
    fn voluntary_location_line_answer_is_not_truncated() {
        // A real, resolvable location line so grounding keeps it.
        let dir = std::env::temp_dir().join(format!("grove-agent-vol-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "fn a(){}\n").unwrap();
        let client = FakeClient::new(vec![text_response("rust:a.rs#a@1")]);
        let ans = run_explore("where is a", &dir, &cfg(), &client).unwrap();
        assert!(!ans.truncated);
        assert_eq!(ans.turns, 1);
        assert_eq!(ans.text, "rust:a.rs#a@1");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn every_turn_offers_the_full_reference_toolset() {
        let client = FakeClient::new(vec![text_response("done")]);
        run_explore("q", Path::new("."), &cfg(), &client).unwrap();
        let seen = &client.seen_tool_names.borrow()[0];
        assert_eq!(
            seen,
            &vec![
                "Glob",
                "Grep",
                "Read",
                "mcp__grove__outline",
                "mcp__grove__symbols",
                "mcp__grove__source",
                "mcp__grove__callers",
                "mcp__grove__map",
                "mcp__grove__definition",
            ]
        );
    }

    #[test]
    fn turn_cap_forces_an_answer_via_the_no_tools_turn() {
        // Always request a (repeated) tool so it never terminates on text; the
        // thrash detector or turn cap must break, then the forced turn answers.
        let mut responses = Vec::new();
        for _ in 0..(MAX_TURNS + 2) {
            responses.push(tool_call_response(
                "mcp__grove__map",
                json!({"dir": "."}),
            ));
        }
        responses.push(text_response("src/x.rs:1"));
        let client = FakeClient::new(responses);
        let ans = run_explore("q", Path::new("."), &cfg(), &client).unwrap();
        assert!(ans.truncated, "answer came from the forced/backstop path");
        // The last request (forced turn) carries NO tools.
        let seen = client.seen_tool_names.borrow();
        assert!(seen.last().unwrap().is_empty(), "forced turn has no tools");
    }

    #[test]
    fn duplicate_calls_trip_the_thrash_backstop_early() {
        // Three identical (duplicate) calls → consec_unprod ≥ 3 → break → forced.
        let mut responses = Vec::new();
        for _ in 0..5 {
            responses.push(tool_call_response(
                "mcp__grove__symbols",
                json!({"dir": ".", "name": "zzz"}),
            ));
        }
        responses.push(text_response("(nothing)"));
        let client = FakeClient::new(responses);
        let ans = run_explore("q", Path::new("."), &cfg(), &client).unwrap();
        assert!(ans.truncated);
        // Broke well before the 12-turn cap (3 dup calls → thrash).
        assert!(ans.turns <= 4, "thrash broke early, turns={}", ans.turns);
    }

    #[test]
    fn leaked_tool_call_is_retried_not_taken_as_answer() {
        // Turn 1: a leaked tool-call in content (no real answer) → retry prompt.
        // Turn 2: a real location line.
        let dir = std::env::temp_dir().join(format!("grove-agent-leak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "fn a(){}\n").unwrap();
        let client = FakeClient::new(vec![
            text_response("<tool_call>{\"name\":\"Grep\"}</tool_call>"),
            text_response("rust:a.rs#a@1"),
        ]);
        let ans = run_explore("q", &dir, &cfg(), &client).unwrap();
        assert_eq!(ans.text, "rust:a.rs#a@1");
        assert!(!ans.truncated);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn provider_down_maps_to_provider_down_error() {
        struct DownClient;
        impl ChatClient for DownClient {
            fn chat(&self, _req: ChatRequest) -> Result<ChatResponse, ClientError> {
                Err(ClientError::Connection {
                    url: "http://x".into(),
                    detail: "refused".into(),
                })
            }
        }
        let err = run_explore("q", Path::new("."), &cfg(), &DownClient).unwrap_err();
        assert!(matches!(err, ExploreError::ProviderDown { .. }));
    }
}
