//! MCP server — the second face. Speaks newline-delimited JSON-RPC 2.0 over
//! stdio (the MCP stdio transport), exposing the four ops as tools.
//!
//! Deliberately dependency-light: the protocol surface for a tools-only server
//! is small, so we hand-roll it rather than pull an async SDK. stdout is the
//! protocol channel — all diagnostics go to stderr.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{json, Value};

use crate::{ops, registry};

const SERVER_NAME: &str = "grove";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL: &str = "2025-06-18";

/// MCP revisions grove's minimal tools surface is compatible with. On
/// `initialize` we echo the client's version when it's one of these, otherwise
/// we answer with our latest (`DEFAULT_PROTOCOL`) rather than parroting an
/// unknown version back (which would falsely claim support).
const SUPPORTED_PROTOCOLS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// Run the stdio server loop until EOF on stdin.
pub fn serve() -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    eprintln!("grove mcp: ready on stdio");

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("grove mcp: bad json: {e}");
                continue;
            }
        };

        // Notifications (no `id`) get no response.
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        let response = match handle(method, &params) {
            Outcome::Notify => continue,
            Outcome::Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Outcome::Err { code, message } => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message },
            }),
        };
        if id.is_none() {
            continue; // method was a request-shaped notification; still no reply
        }
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

enum Outcome {
    Ok(Value),
    Err { code: i64, message: String },
    Notify,
}

fn handle(method: &str, params: &Value) -> Outcome {
    match method {
        "initialize" => {
            // Answer with the client's version only if we support it; otherwise
            // our latest. Never echo an unknown version (would imply support).
            let requested = params.get("protocolVersion").and_then(Value::as_str);
            let protocol = match requested {
                Some(v) if SUPPORTED_PROTOCOLS.contains(&v) => v,
                _ => DEFAULT_PROTOCOL,
            };
            Outcome::Ok(json!({
                "protocolVersion": protocol,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": SERVER_NAME, "title": "grove", "version": SERVER_VERSION },
                "instructions": instructions(),
            }))
        }
        "notifications/initialized" | "notifications/cancelled" => Outcome::Notify,
        "ping" => Outcome::Ok(json!({})),
        "tools/list" => Outcome::Ok(json!({ "tools": tool_specs() })),
        "tools/call" => call_tool(params),
        other => Outcome::Err {
            code: -32601,
            message: format!("method not found: {other}"),
        },
    }
}

/// Server-level steering, returned from `initialize`. This is the protocol-layer
/// home for grove's adoption nudge (VISION §6.4.1) and the one place the resolved
/// language roster lives, so tool descriptions stay generic and token-cheap.
fn instructions() -> String {
    let langs = registry::available();
    let roster = if langs.is_empty() {
        "No grammars are installed yet — run `grove fetch <lang>` (or `grove init`).".to_string()
    } else {
        format!("Languages available here: {}.", langs.join(", "))
    };
    format!(
        "grove gives byte-precise, token-cheap structural access to this codebase. \
Prefer these tools over grep or reading whole files: `outline` to see a file's \
definitions, `symbols`/`definition` to locate code, `source` to read one \
symbol's body, `callers` to find call sites, and `check` after an edit to \
verify syntax. Every result carries a stable symbol-id \
(`<lang>:<relpath>#<name>@<row>`) you can pass between tools. {roster}"
    )
}

/// The tool catalogue, with LLM-facing descriptions. Descriptions are
/// language-agnostic — grove serves every installed grammar, and the resolved
/// language list is provided once via `initialize`'s `instructions`.
fn tool_specs() -> Value {
    json!([
        {
            "name": "outline",
            "description": "List the definitions in one source file as a compact skeleton (kind, name, signature line, position, a stable symbol-id, and the owning `parent` container — the type/class/trait/module a member belongs to, so you can group members without guessing from line numbers). Works on any source file in a language grove has a grammar for. Use this INSTEAD of reading a whole file. For large, definition-dense files, pass `kind` to narrow and/or a lower `detail` to stay token-cheap. Returns JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to a source file" },
                    "kind": { "type": "string", "description": "Only this kind (kinds are language-dependent), e.g. class, struct, function, method, module" },
                    "detail": { "type": "integer", "enum": [0, 1, 2], "description": "0 terse (kind/name/parent/row) · 1 default (adds id/signature) · 2 full (adds byte offsets). Default 1." }
                },
                "required": ["file"]
            },
            "annotations": { "title": "Outline a file", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "symbols",
            "description": "Find symbols across a directory (gitignore-aware), in any language grove has a grammar for. Filter by kind (function, method, class, struct, …) and/or a name substring. Use this to locate where something is defined instead of grepping. Returns JSON with stable symbol-ids.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir": { "type": "string", "description": "Directory to search" },
                    "kind": { "type": "string", "description": "Only this kind, e.g. function, class, method (language-dependent)" },
                    "name": { "type": "string", "description": "Only names containing this substring (case-insensitive)" },
                    "refs": { "type": "boolean", "description": "Include references, not just definitions (default false)" }
                },
                "required": ["dir"]
            },
            "annotations": { "title": "Find symbols", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "source",
            "description": "Return the exact full source of one symbol — by its symbol-id (from outline/symbols), or by file + name. Use this to read a single function/class/type body without loading the whole file. If several definitions share the name, returns the first plus `other_candidates` (their ids) so you can disambiguate without another call. Returns JSON { id, source, other_candidates? }.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "A symbol-id like <lang>:<relpath>#<name>@<row> (e.g. py:app/main.py#run@41)" },
                    "file": { "type": "string", "description": "Alternatively, the source file" },
                    "name": { "type": "string", "description": "…and the symbol name to find in it" }
                },
                "anyOf": [ { "required": ["id"] }, { "required": ["file", "name"] } ]
            },
            "annotations": { "title": "Read a symbol's source", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "check",
            "description": "Parse a source file and report syntax errors (ERROR / MISSING nodes) with positions, in any language grove has a grammar for. Use this AFTER editing a file to confirm you did not break its syntax. Reports syntactic breakage only, not type or semantic errors. Returns JSON; empty array means the file is syntactically valid.",
            "inputSchema": {
                "type": "object",
                "properties": { "file": { "type": "string", "description": "Path to a source file" } },
                "required": ["file"]
            },
            "annotations": { "title": "Check syntax", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "callers",
            "description": "Find every call site of a function/method by name across a directory, each with its enclosing function and source line. Use this to answer 'what calls this?'. Note: name-based — it matches calls to any function/method with this name, not resolved by receiver type. Returns JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Function/method name to find calls to" },
                    "dir": { "type": "string", "description": "Directory to search (default: current)" }
                },
                "required": ["name"]
            },
            "annotations": { "title": "Find callers", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "definition",
            "description": "Find where a symbol is defined (go-to-def). Either pass `name` for an exact-name lookup, or `at` (file:row:col, 0-based) to resolve the identifier under a usage site and jump to its definition. Returns JSON definitions with signature, parent, and id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Exact symbol name to resolve" },
                    "at": { "type": "string", "description": "Usage site to resolve instead: file:row:col (0-based)" },
                    "dir": { "type": "string", "description": "Directory to search (default: current)" }
                },
                "anyOf": [ { "required": ["name"] }, { "required": ["at"] } ]
            },
            "annotations": { "title": "Go to definition", "readOnlyHint": true, "openWorldHint": false }
        }
    ])
}

fn call_tool(params: &Value) -> Outcome {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    let str_arg = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);

    let result: Result<Value> = match name {
        "outline" => match str_arg("file") {
            Some(f) => match outline_detail(&args) {
                Ok(detail) => ops::outline(&PathBuf::from(f), str_arg("kind").as_deref())
                    .map(|syms| ops::project(&syms, detail)),
                Err(e) => Err(e),
            },
            None => missing("file"),
        },
        "symbols" => match str_arg("dir") {
            Some(d) => ops::symbols(
                &PathBuf::from(d),
                str_arg("kind").as_deref(),
                str_arg("name").as_deref(),
                args.get("refs").and_then(Value::as_bool).unwrap_or(false),
            )
            .and_then(to_json),
            None => missing("dir"),
        },
        "source" => {
            if let Some(id) = str_arg("id") {
                ops::source(&id, None).and_then(to_json)
            } else if let (Some(file), Some(n)) = (str_arg("file"), str_arg("name")) {
                ops::source(&file, Some(&n)).and_then(to_json)
            } else {
                Err(anyhow::anyhow!("provide either `id`, or both `file` and `name`"))
            }
        }
        "check" => match str_arg("file") {
            Some(f) => ops::check(&PathBuf::from(f)).and_then(to_json),
            None => missing("file"),
        },
        "callers" => match str_arg("name") {
            Some(n) => {
                let dir = str_arg("dir").unwrap_or_else(|| ".".to_string());
                ops::callers(&PathBuf::from(dir), &n).and_then(to_json)
            }
            None => missing("name"),
        },
        "definition" => {
            // One shape for both modes — `{resolved, definitions}` — so a client
            // never has to branch on which argument it passed. In `at` mode
            // `resolved` is the name grove resolved at the position; in `name`
            // mode it is simply the name the caller supplied.
            let dir = PathBuf::from(str_arg("dir").unwrap_or_else(|| ".".to_string()));
            if let Some(at) = str_arg("at") {
                match ops::parse_pos(&at) {
                    Ok((file, row, col)) => ops::definition_at(&file, row, col, &dir)
                        .map(|(name, defs)| json!({ "resolved": name, "definitions": defs })),
                    Err(e) => Err(e),
                }
            } else if let Some(name) = str_arg("name") {
                ops::definition(&dir, &name)
                    .map(|defs| json!({ "resolved": name, "definitions": defs }))
            } else {
                Err(anyhow::anyhow!("provide either `name` or `at`"))
            }
        }
        other => {
            return Outcome::Err {
                code: -32602,
                message: format!("unknown tool: {other}"),
            }
        }
    };

    match result {
        Ok(value) => Outcome::Ok(tool_text(&value, false)),
        // Tool-level failures are reported as a result with isError, per MCP,
        // so the model sees the message and can recover rather than the whole
        // call erroring out.
        Err(e) => Outcome::Ok(tool_text(&json!(e.to_string()), true)),
    }
}

fn to_json<T: serde::Serialize>(v: T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

fn missing(field: &str) -> Result<Value> {
    Err(anyhow::anyhow!("missing required argument: {field}"))
}

/// Resolve the `outline` `detail` tier, validating it against the advertised
/// `enum [0,1,2]`. Absent means the default (1). Anything else is an error —
/// never a silent `as u8` truncation (`256` -> `0`) that serves the wrong tier.
fn outline_detail(args: &Value) -> Result<u8> {
    match args.get("detail") {
        None | Some(Value::Null) => Ok(1),
        Some(v) => match v.as_u64() {
            Some(d @ (0 | 1 | 2)) => Ok(d as u8),
            _ => Err(anyhow::anyhow!("`detail` must be 0, 1, or 2 (got {v})")),
        },
    }
}

/// Wrap a JSON value as an MCP tool result (a single text content block).
/// Uses compact (non-pretty) JSON — agents parse it fine, and on large payloads
/// pretty-printing's whitespace was a real fraction of the token cost.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the `definition` tool and return its parsed JSON payload.
    fn definition(args: Value) -> Value {
        let params = json!({ "name": "definition", "arguments": args });
        match call_tool(&params) {
            Outcome::Ok(v) => {
                assert_eq!(v["isError"], json!(false), "tool errored: {v}");
                let text = v["content"][0]["text"].as_str().expect("text content");
                serde_json::from_str(text).expect("payload is JSON")
            }
            Outcome::Err { message, .. } => panic!("expected Outcome::Ok, got Err: {message}"),
            Outcome::Notify => panic!("expected Outcome::Ok, got Notify"),
        }
    }

    fn fixture() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("grove_mcp_def_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.rs"), "fn helper() {}\nfn main() {\n    helper();\n}\n").unwrap();
        dir
    }

    #[test]
    fn both_modes_return_resolved_and_definitions() {
        let dir = fixture();

        // name mode — previously a bare array.
        let by_name = definition(json!({ "name": "helper", "dir": dir.to_str().unwrap() }));
        assert_eq!(by_name["resolved"], json!("helper"));
        assert!(by_name["definitions"].is_array(), "definitions must be an array, got {by_name}");
        assert_eq!(by_name["definitions"].as_array().unwrap().len(), 1);

        // at mode — the call site of `helper` on row 2, col 4.
        let at = format!("{}:2:4", dir.join("lib.rs").display());
        let by_at = definition(json!({ "at": at, "dir": dir.to_str().unwrap() }));
        assert_eq!(by_at["resolved"], json!("helper"));
        assert!(by_at["definitions"].is_array());

        // Both modes expose the same keys — a client never branches on the shape.
        let keys = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(keys(&by_name), keys(&by_at));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn outline_detail_validates_range() {
        assert_eq!(outline_detail(&json!({})).unwrap(), 1, "absent -> default 1");
        assert_eq!(outline_detail(&json!({ "detail": 0 })).unwrap(), 0);
        assert_eq!(outline_detail(&json!({ "detail": 2 })).unwrap(), 2);

        // The exact bug: 256 wrapped to 0, 257 to 1 under `as u8`.
        assert!(outline_detail(&json!({ "detail": 256 })).is_err());
        assert!(outline_detail(&json!({ "detail": 257 })).is_err());
        assert!(outline_detail(&json!({ "detail": 3 })).is_err());
        assert!(outline_detail(&json!({ "detail": "2" })).is_err());
        assert!(outline_detail(&json!({ "detail": -1 })).is_err());
    }

    #[test]
    fn outline_out_of_range_is_tool_error() {
        let params = json!({
            "name": "outline",
            "arguments": { "file": "src/mcp.rs", "detail": 256 }
        });
        match call_tool(&params) {
            Outcome::Ok(v) => {
                assert_eq!(v["isError"], json!(true), "out-of-range detail must be a tool error");
                let msg = v["content"][0]["text"].as_str().unwrap_or("");
                assert!(msg.contains("detail"), "error should mention detail, got: {msg}");
            }
            _ => panic!("expected Outcome::Ok with isError=true"),
        }
    }
}
