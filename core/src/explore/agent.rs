//! The inner explorer agent loop — a direct translation of the reference bench's
//! `agent/agent.py::_agent_loop` and `mcp_server.py::_instrumented_loop`.
//!
//! The loop is bounded by **turns only** (≤ [`MAX_TURNS`]); there is deliberately
//! **no cumulative byte budget** (the earlier grove reimplementation's 128 KiB
//! hard-abort is gone). At the turn limit the loop injects the bench's
//! forced-final-answer message ([`steering::FORCE_FINAL_ANSWER`]) and takes one
//! more model turn, so exhaustion produces an *answer*, not a "no answer
//! produced" sentinel.
//!
//! Arm selection ([`Mode`]):
//! - [`Mode::Standard`] → merit (all four tools, model chooses),
//! - [`Mode::Aggressive`] → coerce (grove-first steering),
//! - [`Mode::Balanced`] → plan-first (recon → `submit_plan` → execute, with the
//!   recon plan cached once per repo per process).

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use super::client::{ChatClient, ChatRequest, ClientError, Message};
use super::config::{ExploreConfig, Mode};
use super::{grounding, steering, toolset};

// --- Bench constants (from the vendored MCP env / mcp_server.py) ------------

/// Hard turn cap (`mcp_server.py::MAX_TURNS_CAP`).
pub const MAX_TURNS: usize = 6;
/// Grove-recon turns before Grove closes in plan-first (`FC_RECON_TURNS`).
pub const RECON_TURNS: usize = 2;
/// Generation cap (`FC_MAX_TOKENS`).
const MAX_COMPLETION_TOKENS: u32 = 1024;
/// Sampling temperature (`FC_TEMPERATURE`).
const TEMPERATURE: f32 = 0.0;
/// Nucleus sampling (`llm.py` default `top_p`).
const TOP_P: f32 = 0.95;

/// Cached hint prefix for the recon-once plan (`mcp_server.py::CACHED_HINT`).
const CACHED_HINT: &str = "PRIOR STRUCTURAL MAP of this repository, from an earlier recon pass (use as a starting hint — it may not fully cover THIS question; verify with tools):\n";

// ---------------------------------------------------------------------------
// Public types (unchanged contract consumed by cli/src/mcp.rs)
// ---------------------------------------------------------------------------

/// The successful result of an exploration run.
#[derive(Debug, Clone)]
pub struct ExploreAnswer {
    /// The grounded final answer (prose + validated `<final_answer>` citations).
    pub text: String,
    /// The number of turns consumed.
    pub turns: usize,
    /// True when the answer came from the forced-final-answer / turn-cap path.
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

// ---------------------------------------------------------------------------
// Recon-once plan cache (mcp_server.py::_PLAN_CACHE), process-global, keyed by
// canonical repo path.
// ---------------------------------------------------------------------------

fn plan_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string()
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum Phase {
    Recon,
    Execute,
}

/// A sink for per-turn progress, so a long delegated call can report liveness to
/// the waiting client (MCP `notifications/progress`). `progress`/`total` drive a
/// bar; `message` is a short human-facing status.
pub trait ProgressReporter {
    /// Report progress at `progress` of `total`, with a status `message`.
    fn report(&self, progress: usize, total: usize, message: &str);
}

/// A reporter that drops every update (the default for [`run_explore`] and tests).
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn report(&self, _progress: usize, _total: usize, _message: &str) {}
}

/// Explore `question` over `root`, delegating to the local model via `client`.
/// Convenience wrapper over [`run_explore_reporting`] with no progress sink.
pub fn run_explore(
    question: &str,
    root: &Path,
    cfg: &ExploreConfig,
    client: &dyn ChatClient,
) -> Result<ExploreAnswer, ExploreError> {
    run_explore_reporting(question, root, cfg, client, &NoopReporter)
}

/// Explore `question` over `root`, delegating to the local model via `client`
/// and reporting per-turn progress to `progress`. Direct port of
/// `_instrumented_loop` (which subsumes `_agent_loop` when plan-first is off).
pub fn run_explore_reporting(
    question: &str,
    root: &Path,
    cfg: &ExploreConfig,
    client: &dyn ChatClient,
    progress: &dyn ProgressReporter,
) -> Result<ExploreAnswer, ExploreError> {
    // Progress bar spans the worst case: one tick per turn, plus a final tick.
    let total = MAX_TURNS + 2;
    let qwen = cfg.model.to_lowercase().contains("qwen");
    let plan_first = cfg.mode == Mode::Balanced;

    let cached_plan = if plan_first {
        plan_cache().lock().unwrap().get(&cache_key(root)).cloned()
    } else {
        None
    };
    let do_recon = plan_first && cached_plan.is_none();

    let mut sys_content = steering::system_prompt(cfg.mode, root);
    if do_recon {
        sys_content.push_str(steering::PHASE1_NOTE);
    }

    let mut messages: Vec<Message> = vec![Message::system(sys_content), Message::user(question)];
    if let Some(plan) = &cached_plan {
        messages.push(Message::user(format!("{CACHED_HINT}{plan}")));
    }

    let mut phase = if do_recon { Phase::Recon } else { Phase::Execute };
    let mut grove_turns = 0usize;
    let mut n = 0usize;
    let mut last_text = String::new();
    // Human-facing status carried into the *next* progress tick (so each tick,
    // emitted just before the slow model call, describes the freshest activity).
    let mut activity = if do_recon {
        "planning: mapping structure".to_string()
    } else {
        "exploring the codebase".to_string()
    };

    loop {
        n += 1;
        if n > MAX_TURNS + 1 {
            break;
        }
        if n == MAX_TURNS + 1 {
            messages.push(Message::user(steering::FORCE_FINAL_ANSWER));
            activity = "wrapping up: final answer".to_string();
        }
        // Tick before the (slow) model call, so the client sees liveness during
        // generation and a message describing the most recent step.
        progress.report(n, total, &format!("turn {n}/{} · {activity}", MAX_TURNS + 1));

        // Toolset for this turn.
        let tools = if phase == Phase::Recon {
            toolset::recon_toolset(grove_turns < RECON_TURNS)
        } else {
            toolset::execute_toolset()
        };
        let allowed: Vec<String> = tools.iter().map(|t| t.function.name.clone()).collect();

        // Call the model.
        let req = ChatRequest::new(messages.clone())
            .with_tools(tools)
            .with_bench_sampling(TEMPERATURE, TOP_P, MAX_COMPLETION_TOKENS, None, qwen);
        let resp = client.chat(req).map_err(map_client_error)?;
        let step = match resp.first_message() {
            Some(m) => m.clone(),
            None => break,
        };
        last_text = step.content.clone().unwrap_or_default();
        messages.push(step.clone());

        if step.tool_calls.is_empty() {
            // A text-only turn ends the run in the execute phase (the final
            // answer). In recon, it is ignored and the loop continues (the model
            // is eventually forced to submit_plan).
            if phase == Phase::Execute {
                progress.report(total, total, "grounding answer");
                return Ok(ExploreAnswer {
                    text: grounding::get_final_answer(&last_text, root),
                    turns: n,
                    truncated: false,
                });
            }
            continue;
        }

        // Dispatch each tool call.
        let mut used_grove = false;
        let mut transition = false;
        for c in &step.tool_calls {
            let obs = if c.name == toolset::SUBMIT_PLAN && phase == Phase::Recon {
                let plan_args = serialize_args(&c.arguments);
                if !plan_args.is_empty() {
                    plan_cache()
                        .lock()
                        .unwrap()
                        .insert(cache_key(root), plan_args.clone());
                }
                messages.push(Message::tool(&c.id, steering::PLAN_RECORDED_NOTE));
                messages.push(Message::user(format!(
                    "{}\n\nYour recorded plan:\n{}",
                    steering::PHASE2_NOTE, plan_args
                )));
                transition = true;
                continue;
            } else if !allowed.contains(&c.name) {
                if phase == Phase::Recon {
                    steering::RECON_CLOSED_NOTE.to_string()
                } else {
                    "<system-reminder>Planning is done. Use Read/Grep/Glob/Grove to execute your plan, then emit <final_answer>.</system-reminder>".to_string()
                }
            } else if phase == Phase::Recon
                && c.name == toolset::GROVE
                && !toolset::RECON_VERBS.contains(&toolset::grove_verb(&c.arguments).as_str())
            {
                steering::RECON_VERB_NOTE.to_string()
            } else {
                let o = toolset::dispatch(&c.name, &c.arguments, root);
                if c.name == toolset::GROVE {
                    used_grove = true;
                }
                o
            };
            messages.push(Message::tool(&c.id, obs));
        }
        if used_grove {
            grove_turns += 1;
        }
        if transition {
            phase = Phase::Execute;
        }
        activity = summarize_activity(&step.tool_calls, transition);
    }

    progress.report(total, total, "grounding answer");
    // Fell out via the turn cap: return the best-effort last text, grounded.
    Ok(ExploreAnswer {
        text: grounding::get_final_answer(&last_text, root),
        turns: n.saturating_sub(1),
        truncated: true,
    })
}

/// A short human-facing summary of a turn's tool activity, for the next progress
/// tick (e.g. "Grove symbols, Read userProfileManager.js").
fn summarize_activity(calls: &[super::client::ToolCall], transitioned: bool) -> String {
    if transitioned {
        return "plan set — executing".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for c in calls {
        let part = match c.name.as_str() {
            toolset::GROVE => format!("Grove {}", toolset::grove_verb(&c.arguments)),
            toolset::READ => format!("Read {}", basename_arg(&c.arguments, "path")),
            toolset::GLOB => format!("Glob {}", str_arg(&c.arguments, "pattern")),
            toolset::GREP => format!("Grep {}", str_arg(&c.arguments, "pattern")),
            other => other.to_string(),
        };
        parts.push(part);
    }
    let joined = parts.join(", ");
    // Char-safe truncation (activity text may contain multibyte from paths/patterns).
    let s = if joined.chars().count() > 80 {
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

/// Serialize tool-call arguments back to a compact JSON string (the plan text
/// cached and echoed to the model, matching `c.arguments` in the reference,
/// which is already the raw JSON string).
fn serialize_args(args: &Value) -> String {
    if args.is_null() {
        String::new()
    } else {
        serde_json::to_string(args).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::client::{ChatResponse, Choice, Role, ToolCall};
    use crate::explore::config::Provider;
    use serde_json::json;
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
        }
    }

    fn cfg(mode: Mode) -> ExploreConfig {
        ExploreConfig {
            provider: Provider::Ollama,
            base_url: "http://localhost:11434/v1".into(),
            model: "qwen3.5:4b".into(),
            mode,
            allowed_tools: vec!["grove".into(), "rg".into()],
        }
    }

    #[test]
    fn standard_returns_first_text_only_turn_as_answer() {
        let client = FakeClient::new(vec![text_response("done\n<final_answer>\n</final_answer>")]);
        let ans = run_explore("q", Path::new("."), &cfg(Mode::Standard), &client).unwrap();
        assert!(!ans.truncated);
        assert_eq!(ans.turns, 1);
        assert!(ans.text.starts_with("done"));
    }

    #[test]
    fn standard_offers_the_four_execute_tools() {
        let client = FakeClient::new(vec![text_response("x")]);
        run_explore("q", Path::new("."), &cfg(Mode::Standard), &client).unwrap();
        let seen = &client.seen_tool_names.borrow()[0];
        assert_eq!(seen, &vec!["Read", "Glob", "Grep", "Grove"]);
    }

    #[test]
    fn turn_cap_forces_a_final_answer_not_a_sentinel() {
        // Always request a (disallowed→ignored) tool so it never terminates on
        // text; must break at the cap and still return grounded text.
        let mut responses = Vec::new();
        for _ in 0..(MAX_TURNS + 2) {
            responses.push(tool_call_response("Grove", json!({"command": "map ."})));
        }
        let client = FakeClient::new(responses);
        let ans = run_explore("q", Path::new("."), &cfg(Mode::Standard), &client).unwrap();
        assert!(ans.truncated, "hit the turn cap");
        // The forced-final-answer user message was injected before the last call.
        assert!(ans.turns >= MAX_TURNS);
    }

    #[test]
    fn balanced_recon_closes_grove_then_forces_submit_plan() {
        // Turn 1 & 2: Grove recon calls; turn 3: Grove should be closed (only
        // submit_plan offered); model submits plan; then answers.
        let client = FakeClient::new(vec![
            tool_call_response("Grove", json!({"command": "map ."})),
            tool_call_response("Grove", json!({"command": "symbols ."})),
            tool_call_response(
                "submit_plan",
                json!({"focus_files": "a.rs", "steps": "read a.rs"}),
            ),
            text_response("answer\n<final_answer>\n</final_answer>"),
        ]);
        let root = std::env::temp_dir().join(format!("grove-agent-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let ans = run_explore("q", &root, &cfg(Mode::Balanced), &client).unwrap();
        assert!(!ans.truncated);
        let seen = client.seen_tool_names.borrow();
        // Turn 1: Grove + submit_plan (recon, grove open).
        assert!(seen[0].contains(&"Grove".to_string()) && seen[0].contains(&"submit_plan".to_string()));
        // Turn 3 (after 2 grove recon turns): Grove closed → submit_plan only.
        assert_eq!(seen[2], vec!["submit_plan"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn progress_is_reported_each_turn_and_at_the_end() {
        use std::cell::RefCell;
        struct Recorder {
            ticks: RefCell<Vec<(usize, usize, String)>>,
        }
        impl ProgressReporter for Recorder {
            fn report(&self, progress: usize, total: usize, message: &str) {
                self.ticks
                    .borrow_mut()
                    .push((progress, total, message.to_string()));
            }
        }
        // Turn 1: a Grove call; turn 2: the final text answer.
        let client = FakeClient::new(vec![
            tool_call_response("Grove", json!({"command": "symbols ."})),
            text_response("done\n<final_answer>\n</final_answer>"),
        ]);
        let rec = Recorder {
            ticks: RefCell::new(Vec::new()),
        };
        run_explore_reporting("q", Path::new("."), &cfg(Mode::Standard), &client, &rec).unwrap();
        let ticks = rec.ticks.borrow();
        // At least: turn 1 pre-call, turn 2 pre-call, final "grounding answer".
        assert!(ticks.len() >= 3, "got {} ticks", ticks.len());
        assert!(ticks[0].2.contains("turn 1/"), "first tick: {:?}", ticks[0]);
        // Progress is monotonically non-decreasing.
        assert!(ticks.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(ticks.last().unwrap().2, "grounding answer");
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
        let err = run_explore("q", Path::new("."), &cfg(Mode::Standard), &DownClient).unwrap_err();
        assert!(matches!(err, ExploreError::ProviderDown { .. }));
    }
}
