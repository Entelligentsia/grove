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

use grove_core::{ops, registry};

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
Prefer these tools over grep or reading whole files: `map` for a directory-level \
dependency graph (definitions + which other symbols each one references — replaces \
many symbols+source round-trips with one call), `outline` to see a file's \
definitions, `symbols`/`definition` to locate code, `source` to read one \
symbol's body, `callers` to find call sites, and `check` after an edit to \
verify syntax. Every result carries a stable symbol-id \
(`<lang>:<relpath>#<name>@<line>`, line 1-based) you can pass between tools. {roster}\n\n\
Breadth control: prefer `map` for architectural overviews — it returns the \
definition-and-reference graph of a directory in one call. Use `source` only \
for the few load-bearing definitions you need to read in full. Avoid fetching \
many sources in sequence; instead map first, then read only the symbols that matter."
    )
}

/// The tool catalogue, with LLM-facing descriptions. Descriptions are
/// language-agnostic — grove serves every installed grammar, and the resolved
/// language list is provided once via `initialize`'s `instructions`.
///
/// Every `inputSchema` is a plain `{type: object, properties, required?}`. Do
/// **not** use a top-level `anyOf`/`oneOf` to express "one arg form or the
/// other": some MCP clients can't normalize that and silently drop the tool
/// from the registered set (it was why `source`/`definition` once went missing
/// while the four flat-schema tools registered fine). For mutually-exclusive
/// argument forms, leave the alternatives optional here and enforce the choice
/// in `call_tool`, which returns a clear `isError` when neither is supplied.
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
                    "detail": { "type": "integer", "enum": [0, 1, 2], "description": "0 terse (kind/name/parent/line) · 1 default (adds id/signature) · 2 full (adds byte offsets). Default 1." }
                },
                "required": ["file"]
            },
            "annotations": { "title": "Outline a file", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "symbols",
            "description": "Find symbols across a directory (gitignore-aware), in any language grove has a grammar for. Filter by kind (function, method, class, struct, …) and/or a name. `name` matches exactly (case-insensitive); set `nameContains` true for substring matching. Use this to locate where something is defined instead of grepping. Returns JSON with stable symbol-ids. For a structured overview with reference edges, prefer `map` — it returns definitions and their outgoing references in one call, replacing many `symbols`+`source` round-trips.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir": { "type": "string", "description": "Directory to search" },
                    "kind": { "type": "string", "description": "Only this kind, e.g. function, class, method (language-dependent)" },
                    "name": { "type": "string", "description": "Only definitions whose name equals this exactly (case-insensitive)" },
                    "nameContains": { "type": "boolean", "description": "Substring matching for `name` instead of exact equality (case-insensitive). Default false." },
                    "refs": { "type": "boolean", "description": "Include references, not just definitions (default false)" }
                },
                "required": ["dir"]
            },
            "annotations": { "title": "Find symbols", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "source",
            "description": "Return the exact full source of one symbol — by its symbol-id (from outline/symbols/map), or by file + name. Use sparingly — read one symbol at a time, only when you need its full implementation. For architectural understanding, prefer `map` (directory-level dependency graph in one call) or `outline` (file skeleton without bodies). If several definitions share the name, returns the first plus `other_candidates` (their ids) so you can disambiguate without another call. Returns JSON { id, source, other_candidates? }.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "A symbol-id like <lang>:<relpath>#<name>@<line> (e.g. py:app/main.py#run@41); line is 1-based" },
                    "file": { "type": "string", "description": "Alternatively, the source file (with `name`)" },
                    "name": { "type": "string", "description": "…and the symbol name to find in it" }
                }
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
            "description": "Find every reference to a symbol by name across a directory, with enclosing function and source line. Returns two kinds of results: 'structural' (tree-sitter resolved, high precision) and 'textual' (whole-word grep, high recall — covers type annotations, imports, and references the tags query misses). Use this to answer 'what calls this?' or 'where is this used?'. Name-based — matches any symbol with this name, not resolved by receiver type. For a broad call-graph view of a directory, prefer `map`. Returns JSON.",
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
            "name": "map",
            "description": "Return a compact structural map of a directory: every definition grouped by file, with each definition's outgoing references (which other symbols it calls or uses). No source bodies — just the dependency graph. Use this instead of many symbols+source calls when you need a broad picture of how code connects. Prefer map for architectural understanding; use source only for the few load-bearing definitions you need to read in full. Filter by kind or name (`name` is exact, case-insensitive; set `nameContains` true for substring) to narrow the view. Returns JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir": { "type": "string", "description": "Directory to map" },
                    "kind": { "type": "string", "description": "Only definitions of this kind, e.g. function, class, method" },
                    "name": { "type": "string", "description": "Only definitions whose name equals this exactly (case-insensitive)" },
                    "nameContains": { "type": "boolean", "description": "Substring matching for `name` instead of exact equality (case-insensitive). Default false." }
                },
                "required": ["dir"]
            },
            "annotations": { "title": "Map a directory's structure", "readOnlyHint": true, "openWorldHint": false }
        },
        {
            "name": "definition",
            "description": "Find where a symbol is defined (go-to-def). Two modes. `name`: exact-name lookup across the directory — may return several candidates when a name is reused. `at` (file:line:col, 1-based): resolve the identifier under a *usage site* — the precise mode, preferred whenever you have a position. It is scope-aware (a local/parameter binding wins over a same-named global) and follows import edges across files (an imported symbol resolves to its definition in the target file), so it returns the single binding the cursor actually refers to instead of a candidate list. Falls back to name lookup when it can't resolve (e.g. a re-exported or dynamically imported symbol), so it is never worse than `name`. Returns JSON definitions with signature, parent, and id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Exact symbol name to resolve (provide this or `at`)" },
                    "at": { "type": "string", "description": "Usage site to resolve, scope-aware + cross-file: file:line:col (1-based). Prefer this when you have a position." },
                    "dir": { "type": "string", "description": "Directory to search (default: current)" }
                }
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
                args.get("nameContains").and_then(Value::as_bool).unwrap_or(false),
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
        "map" => match str_arg("dir") {
            Some(d) => ops::map(
                &PathBuf::from(d),
                str_arg("kind").as_deref(),
                str_arg("name").as_deref(),
                args.get("nameContains").and_then(Value::as_bool).unwrap_or(false),
            )
            .and_then(to_json),
            None => missing("dir"),
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
            Some(d @ 0..=2) => Ok(d as u8),
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

    /// A unique fixture dir per test (`tag`), so parallel tests never share or
    /// delete each other's files.
    fn fixture(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("grove_mcp_test_{}_{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.rs"), "fn helper() {}\nfn main() {\n    helper();\n}\n").unwrap();
        dir
    }

    #[test]
    fn both_modes_return_resolved_and_definitions() {
        let dir = fixture("both_modes");

        // name mode — previously a bare array.
        let by_name = definition(json!({ "name": "helper", "dir": dir.to_str().unwrap() }));
        assert_eq!(by_name["resolved"], json!("helper"));
        assert!(by_name["definitions"].is_array(), "definitions must be an array, got {by_name}");
        assert_eq!(by_name["definitions"].as_array().unwrap().len(), 1);

        // at mode — the call site of `helper` on line 3, col 5 (1-based).
        let at = format!("{}:3:5", dir.join("lib.rs").display());
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

    // ---- helpers ----

    fn ok(o: Outcome) -> Value {
        match o {
            Outcome::Ok(v) => v,
            Outcome::Err { message, .. } => panic!("expected Ok, got Err: {message}"),
            Outcome::Notify => panic!("expected Ok, got Notify"),
        }
    }

    /// Parse the JSON payload out of a successful (non-error) tool result.
    fn tool_payload(o: Outcome) -> Value {
        let v = ok(o);
        assert_eq!(v["isError"], json!(false), "tool errored: {v}");
        serde_json::from_str(v["content"][0]["text"].as_str().expect("text")).expect("json")
    }

    fn tool_error_msg(o: Outcome) -> String {
        let v = ok(o);
        assert_eq!(v["isError"], json!(true), "expected a tool error: {v}");
        v["content"][0]["text"].as_str().unwrap_or("").to_string()
    }

    // ---- protocol layer (handle) ----

    #[test]
    fn initialize_echoes_supported_version_else_default() {
        let v = ok(handle("initialize", &json!({ "protocolVersion": "2024-11-05" })));
        assert_eq!(v["protocolVersion"], json!("2024-11-05"), "echo a supported version");
        assert_eq!(v["serverInfo"]["name"], json!("grove"));
        assert!(v["instructions"].as_str().unwrap().contains("symbol-id"));

        let v2 = ok(handle("initialize", &json!({ "protocolVersion": "1999-01-01" })));
        assert_eq!(v2["protocolVersion"], json!(DEFAULT_PROTOCOL), "unknown -> our latest");
    }

    #[test]
    fn ping_and_tools_list_and_notifications() {
        assert_eq!(ok(handle("ping", &Value::Null)), json!({}));

        let v = ok(handle("tools/list", &Value::Null));
        let names: Vec<&str> = v["tools"].as_array().unwrap().iter()
            .map(|t| t["name"].as_str().unwrap()).collect();
        for expected in ["outline", "symbols", "source", "check", "callers", "map", "definition"] {
            assert!(names.contains(&expected), "tools/list missing {expected}");
        }

        assert!(matches!(handle("notifications/initialized", &Value::Null), Outcome::Notify));
        assert!(matches!(handle("notifications/cancelled", &Value::Null), Outcome::Notify));
    }

    #[test]
    fn every_tool_schema_is_client_registerable() {
        // Regression guard: a top-level `anyOf`/`oneOf` in an inputSchema makes
        // some MCP clients silently drop the tool (source/definition once went
        // missing this way). Every schema must be a plain object.
        let tools = tool_specs();
        let arr = tools.as_array().unwrap();
        assert_eq!(arr.len(), 7, "all seven tools advertised");
        for t in arr {
            let name = t["name"].as_str().unwrap();
            let schema = &t["inputSchema"];
            assert_eq!(schema["type"], json!("object"), "{name}: schema must be type object");
            assert!(schema["properties"].is_object(), "{name}: schema needs properties");
            assert!(schema.get("anyOf").is_none(), "{name}: no top-level anyOf (clients drop it)");
            assert!(schema.get("oneOf").is_none(), "{name}: no top-level oneOf");
        }
    }

    #[test]
    fn source_and_definition_still_enforce_their_arg_choice() {
        // The schema no longer encodes the XOR, so the runtime must: calling
        // with no recognized args is a clean tool error, not a panic/success.
        assert!(tool_error_msg(call("source", json!({}))).contains("provide either `id`"));
        assert!(tool_error_msg(call("definition", json!({}))).contains("provide either `name` or `at`"));
    }

    #[test]
    fn unknown_method_is_method_not_found() {
        match handle("does/not/exist", &Value::Null) {
            Outcome::Err { code, message } => {
                assert_eq!(code, -32601);
                assert!(message.contains("method not found"));
            }
            _ => panic!("expected an Err outcome"),
        }
    }

    fn call(name: &str, args: Value) -> Outcome {
        call_tool(&json!({ "name": name, "arguments": args }))
    }

    // ---- tool dispatch (call_tool) ----

    #[test]
    fn outline_tool_returns_definitions() {
        let dir = fixture("outline");
        let payload = tool_payload(call("outline", json!({ "file": dir.join("lib.rs").to_str().unwrap() })));
        let names: Vec<&str> = payload.as_array().unwrap().iter()
            .map(|s| s["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"helper") && names.contains(&"main"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn outline_tool_missing_file_is_error() {
        let msg = tool_error_msg(call("outline", json!({})));
        assert!(msg.contains("missing required argument: file"), "got: {msg}");
    }

    #[test]
    fn symbols_tool_lists_definitions() {
        let dir = fixture("symbols");
        let payload = tool_payload(call("symbols", json!({ "dir": dir.to_str().unwrap() })));
        assert!(payload.as_array().unwrap().iter().any(|s| s["name"] == json!("helper")));
        assert!(matches!(call("symbols", json!({})),
            Outcome::Ok(v) if v["isError"] == json!(true)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn symbols_tool_name_exact_unless_name_contains() {
        // `name` is exact by default (issue #37); `nameContains` opts into substring.
        let dir = fixture("symbols_name");
        let exact = tool_payload(call("symbols", json!({ "dir": dir.to_str().unwrap(), "name": "help" })));
        assert!(
            exact.as_array().unwrap().is_empty(),
            "exact 'help' must not match 'helper'"
        );
        let substr = tool_payload(call("symbols", json!({ "dir": dir.to_str().unwrap(), "name": "help", "nameContains": true })));
        assert!(
            substr.as_array().unwrap().iter().any(|s| s["name"] == json!("helper")),
            "nameContains=true must substring-match 'help' onto 'helper'"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn source_tool_by_file_name_and_by_id_and_neither() {
        let dir = fixture("source");
        let file = dir.join("lib.rs");

        let by_fn = tool_payload(call("source", json!({ "file": file.to_str().unwrap(), "name": "helper" })));
        assert!(by_fn["source"].as_str().unwrap().contains("fn helper"));

        let id = by_fn["id"].as_str().unwrap().to_string();
        let by_id = tool_payload(call("source", json!({ "id": id })));
        assert!(by_id["source"].as_str().unwrap().contains("fn helper"));

        let msg = tool_error_msg(call("source", json!({})));
        assert!(msg.contains("provide either `id`"), "got: {msg}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn check_tool_reports_ok_and_missing_arg() {
        let dir = fixture("check");
        let payload = tool_payload(call("check", json!({ "file": dir.join("lib.rs").to_str().unwrap() })));
        assert_eq!(payload.as_array().unwrap().len(), 0, "clean file -> no defects");
        assert!(tool_error_msg(call("check", json!({}))).contains("missing required argument: file"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn callers_tool_finds_sites_and_requires_name() {
        let dir = fixture("callers");
        let payload = tool_payload(call("callers", json!({ "name": "helper", "dir": dir.to_str().unwrap() })));
        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert!(tool_error_msg(call("callers", json!({}))).contains("missing required argument: name"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn definition_tool_requires_name_or_at() {
        let msg = tool_error_msg(call("definition", json!({})));
        assert!(msg.contains("provide either `name` or `at`"), "got: {msg}");
    }

    #[test]
    fn unknown_tool_is_invalid_params() {
        match call("nope", json!({})) {
            Outcome::Err { code, message } => {
                assert_eq!(code, -32602);
                assert!(message.contains("unknown tool"));
            }
            _ => panic!("expected an Err outcome"),
        }
    }

    #[test]
    fn tool_text_marks_errors_and_wraps_values() {
        let okv = tool_text(&json!({ "a": 1 }), false);
        assert_eq!(okv["isError"], json!(false));
        assert_eq!(okv["content"][0]["type"], json!("text"));

        let errv = tool_text(&json!("boom"), true);
        assert_eq!(errv["isError"], json!(true));
        assert_eq!(errv["content"][0]["text"], json!("boom"), "error strings are unquoted");
    }

    #[test]
    fn map_tool_returns_definitions_with_references() {
        let dir = fixture("map");
        let payload = tool_payload(call("map", json!({ "dir": dir.to_str().unwrap() })));
        let arr = payload.as_array().expect("map returns an array of file maps");
        assert!(!arr.is_empty(), "map should return at least one file map");
        let fm = &arr[0];
        let entries = fm["entries"].as_array().expect("entries is an array");
        // The fixture has `helper` and `main` — `main` calls `helper`.
        let main_entry = entries.iter().find(|e| e["name"] == "main").expect("main definition");
        assert_eq!(main_entry["references"].as_array().expect("references is an array").len(), 1,
            "main should reference helper");
        assert!(tool_error_msg(call("map", json!({}))).contains("missing required argument: dir"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
