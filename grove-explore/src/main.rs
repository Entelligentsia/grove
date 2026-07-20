//! `grove-explore` — dedicated MCP server + TUI verbs for the explore surface.
//!
//! # Subcommands
//! * `serve [PATH]`              — MCP server (default when no subcommand given)
//! * `config [PATH]`             — full-screen config TUI
//! * `tap [PATH] [--no-enable]`  — enable tracing + trace browser

mod config_tui;
mod tap;
mod trace_tui;

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use grove_core::config::GroveConfig;
use grove_explore_core::{
    health_probe, run_explore_reporting, ExploreConfig, ExploreError, OpenAiCompatClient,
    ProgressReporter, SessionMeta, TraceWriter,
};

const SERVER_NAME: &str = "grove-explore";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL: &str = "2025-06-18";
const SUPPORTED_PROTOCOLS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "grove-explore", about = "grove explore surface: MCP server + TUI verbs")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run as an MCP server over stdio (the explore LLM surface).
    ///
    /// This is the default when grove-explore is invoked with no subcommand
    /// (as registered in `.mcp.json`).
    Serve {
        /// Project root (defaults to current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Open the full-screen config TUI to set up (or edit) the explore config.
    ///
    /// Requires an interactive terminal. Opens pre-populated when
    /// `.grove/config.json` already exists with an explore section.
    Config {
        /// Project directory (default: current).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Enable explore-mode tracing and browse recorded sessions in a TUI.
    ///
    /// Records to `.grove/traces/`; no proxy needed.
    Tap {
        /// Project directory holding `.grove/` (default: current).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Open the browser without turning tracing on in the config.
        #[arg(long = "no-enable")]
        no_enable: bool,
    },
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Serve { path: PathBuf::from(".") }) {
        Cmd::Serve { path } => {
            serve_main(path);
            Ok(())
        }
        Cmd::Config { path } => {
            let root = path.canonicalize().unwrap_or_else(|_| path.clone());
            let grove_cfg = GroveConfig::load(&root).ok();
            config_tui::run(&root, grove_cfg)?;
            Ok(())
        }
        Cmd::Tap { path, no_enable } => {
            let root = path.canonicalize().unwrap_or_else(|_| path.clone());
            tap::run(&root, no_enable)?;
            Ok(())
        }
    }
}

// ── Serve implementation ──────────────────────────────────────────────────────

fn serve_main(path: PathBuf) {
    let root = path.canonicalize().unwrap_or_else(|e| {
        eprintln!("grove-explore: cannot resolve path '{}': {e}", path.display());
        process::exit(1);
    });

    // ── Startup health gate ───────────────────────────────────────────────────
    // Each step is a hard fail (exit 1) — no fallback, no partial startup.

    // Step 1: load .grove/config.json (migrates explore.json if needed).
    let grove_cfg = GroveConfig::load(&root).unwrap_or_else(|e| {
        eprintln!(
            "grove-explore: could not load config ({e}); \
             run `grove init --as mcp-llm` to create .grove/config.json"
        );
        process::exit(1);
    });

    // Step 2: deserialize the explore section.
    let explore_val = match grove_cfg.explore {
        Some(v) => v,
        None => {
            eprintln!(
                "grove-explore: mode is mcp-llm but no explore section found in config; \
                 run `grove-explore config` or `grove init --as mcp-llm` to configure it"
            );
            process::exit(1);
        }
    };
    let cfg = serde_json::from_value::<ExploreConfig>(explore_val).unwrap_or_else(|e| {
        eprintln!(
            "grove-explore: invalid explore config ({e}); \
             run `grove-explore config` to fix the explore section in .grove/config.json"
        );
        process::exit(1);
    });

    // Step 3: confirm the provider is reachable before starting the loop.
    if let Err(e) = health_probe(&cfg) {
        eprintln!("grove-explore: provider unhealthy — {e}");
        process::exit(1);
    }

    // All gates passed — enter the MCP stdio loop.
    serve_explore(&cfg, &root);
}

// ── MCP stdio loop ────────────────────────────────────────────────────────────

fn serve_explore(cfg: &ExploreConfig, root: &Path) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    eprintln!("grove-explore mcp: ready on stdio");

    // Opened lazily on `initialize` when tap is enabled; None otherwise.
    let mut trace_writer: Option<TraceWriter> = None;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("grove-explore mcp: stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("grove-explore mcp: bad json: {e}");
                continue;
            }
        };

        // Notifications (no `id`) get no response.
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Open the session trace on initialize when tap is enabled.
        if method == "initialize" && trace_writer.is_none() && cfg.tap {
            trace_writer = open_session_trace(cfg, root, &params);
        }

        let response = match handle(method, &params, cfg, root, trace_writer.as_ref()) {
            Outcome::Notify => continue,
            Outcome::Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Outcome::Err { code, message } => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message },
            }),
        };
        if id.is_none() {
            continue;
        }
        if writeln!(stdout, "{response}").is_err() || stdout.flush().is_err() {
            break; // client gone
        }
    }
}

// ── Dispatcher ────────────────────────────────────────────────────────────────

enum Outcome {
    Ok(Value),
    Err { code: i64, message: String },
    Notify,
}

fn handle(
    method: &str,
    params: &Value,
    cfg: &ExploreConfig,
    root: &Path,
    trace: Option<&TraceWriter>,
) -> Outcome {
    match method {
        "initialize" => {
            let requested = params.get("protocolVersion").and_then(Value::as_str);
            let protocol = match requested {
                Some(v) if SUPPORTED_PROTOCOLS.contains(&v) => v,
                _ => DEFAULT_PROTOCOL,
            };
            Outcome::Ok(json!({
                "protocolVersion": protocol,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "title": "grove-explore",
                    "version": SERVER_VERSION
                },
                "instructions": explore_instructions(cfg),
            }))
        }
        "notifications/initialized" | "notifications/cancelled" => Outcome::Notify,
        "ping" => Outcome::Ok(json!({})),
        "tools/list" => Outcome::Ok(json!({ "tools": [explore_tool_spec()] })),
        "tools/call" => call_explore_tool(params, cfg, root, trace),
        other => Outcome::Err {
            code: -32601,
            message: format!("method not found: {other}"),
        },
    }
}

// ── Server instructions ───────────────────────────────────────────────────────

fn explore_instructions(cfg: &ExploreConfig) -> String {
    format!(
        "grove is in explore mode: the `explore` tool is a code LOCATOR backed by a small \
         local model ({model} at {base_url}) driving tree-sitter + text search. Ask it \
         targeted where-is / which-file / who-calls questions and it returns validated \
         location lines — one per line, most relevant first, each `lang:path#symbol@line` \
         (or `path:line` when a point has no enclosing symbol). It locates; it does not \
         explain. Because the model is light, keep each question narrow and single-focus: \
         decompose a broad task into a few focused calls, then open the cited locations and \
         synthesize the results yourself — don't delegate one large compound question. If \
         the provider becomes unreachable mid-session, the tool returns an actionable \
         isError result; restart `grove-explore` to reconnect.",
        model = cfg.model,
        base_url = cfg.base_url,
    )
}

// ── Tool spec ─────────────────────────────────────────────────────────────────

fn explore_tool_spec() -> Value {
    json!({
        "name": "explore",
        "description": "Locate WHERE code lives. Ask ONE narrow, single-focus question \
                         (e.g. \"where is session-cookie signing implemented\") and get back validated \
                         location lines — one per line, most relevant first, each \
                         `lang:path#symbol@line` (or `path:line` when a point has no enclosing symbol), \
                         and nothing else. It LOCATES; it does not explain. Backed by a small local \
                         model over structural + text search — not a full-analysis oracle. Keep each \
                         call targeted: for a broad task, make a few focused calls (\"where are the \
                         routes for X\", then \"which function validates Y\") and do your own synthesis.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "One narrow, single-focus locator question, e.g. \"where is the API-key health check defined\". Not a broad, multi-part task."
                }
            },
            "required": ["question"]
        },
        "annotations": { "title": "Explore the codebase", "readOnlyHint": true, "openWorldHint": false }
    })
}

// ── Argument keys accepted for the explore tool's `question` ─────────────────

const EXPLORE_QUESTION_KEYS: [&str; 4] = ["question", "query", "request", "prompt"];

fn resolve_explore_question(params: &Value) -> Option<String> {
    let args = params.get("arguments")?;
    EXPLORE_QUESTION_KEYS
        .iter()
        .find_map(|k| args.get(k).and_then(Value::as_str))
        .map(str::to_string)
}

// ── Progress reporter ─────────────────────────────────────────────────────────

struct StdoutProgress {
    token: Value,
}

impl ProgressReporter for StdoutProgress {
    fn report(&self, progress: usize, total: usize, message: &str) {
        let note = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": {
                "progressToken": self.token,
                "progress": progress,
                "total": total,
                "message": message,
            },
        });
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{note}");
        let _ = out.flush();
    }
}

fn progress_token(params: &Value) -> Option<Value> {
    let tok = params.get("_meta")?.get("progressToken")?;
    if tok.is_null() { None } else { Some(tok.clone()) }
}

// ── Explore tool call ─────────────────────────────────────────────────────────

fn call_explore_tool(
    params: &Value,
    cfg: &ExploreConfig,
    root: &Path,
    trace: Option<&TraceWriter>,
) -> Outcome {
    let question = match resolve_explore_question(params) {
        Some(q) => q,
        None => {
            return Outcome::Ok(tool_text(&json!("missing required argument: question"), true));
        }
    };
    let client = OpenAiCompatClient::new(cfg);
    let reporter = progress_token(params).map(|token| StdoutProgress { token });
    let noop = grove_explore_core::NoopReporter;
    let sink: &dyn ProgressReporter = match &reporter {
        Some(r) => r,
        None => &noop,
    };
    match run_explore_reporting(&question, root, cfg, &client, sink, trace) {
        Ok(answer) => Outcome::Ok(tool_text(&json!(answer.text), false)),
        Err(ExploreError::ProviderDown { url, detail }) => Outcome::Ok(tool_text(
            &json!(format!(
                "provider down ({url}): {detail}; \
                 check the endpoint / run `grove-explore config` / \
                 restart grove-explore to reconnect"
            )),
            true,
        )),
        Err(ExploreError::Client(msg)) => Outcome::Ok(tool_text(&json!(msg), true)),
    }
}

// ── Trace support ─────────────────────────────────────────────────────────────

fn open_session_trace(cfg: &ExploreConfig, root: &Path, params: &Value) -> Option<TraceWriter> {
    let ci = params.get("clientInfo");
    let name = ci.and_then(|c| c.get("name")).and_then(Value::as_str).unwrap_or("unknown");
    let version = ci.and_then(|c| c.get("version")).and_then(Value::as_str).unwrap_or("");
    let meta = SessionMeta::new(
        &cfg.model,
        &enum_str(&cfg.steering),
        &enum_str(&cfg.provider),
        &cfg.base_url,
        name,
        version,
    );
    TraceWriter::open(root, &meta, cfg.trace_retain)
}

fn enum_str<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|x| x.as_str().map(str::to_string))
        .unwrap_or_default()
}

// ── Tool result helper ────────────────────────────────────────────────────────

fn tool_text(value: &Value, is_error: bool) -> Value {
    let text = if is_error && value.is_string() {
        value.as_str().unwrap_or_default().to_string()
    } else {
        value.to_string()
    };
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": is_error,
    })
}
