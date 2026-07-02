//! Tool schema registry, toolset builders, gating, and dispatch.
//!
//! This module owns three responsibilities:
//!
//! 1. **Schema registry** — [`build_full_toolset`] and [`build_recon_toolset`]
//!    return [`Tool`] vecs ready to embed in a [`ChatRequest`]. Names match
//!    the grove op function names exactly.
//!
//! 2. **Gating** — [`is_in_toolset`] lets the loop check whether the model
//!    named a tool that is actually active, so a corrective tool-result can be
//!    injected for hallucinated calls.
//!
//! 3. **Dispatch** — [`dispatch_tool`] routes a named call either to the
//!    in-crate `grove_core::ops::*` functions (structural ops) or to an
//!    allowlisted shell binary via [`std::process::Command`] (no shell
//!    interpolation, binary validated before spawn).

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

use crate::ops;
use super::client::Tool;

// ---------------------------------------------------------------------------
// Limits (also used by agent.rs)
// ---------------------------------------------------------------------------

/// Hard turn limit for the agent loop.
pub const MAX_TURNS: usize = 25;
/// Total accumulated tool-result byte budget (128 KiB).
pub const MAX_TOOL_RESULT_BYTES: usize = 128 * 1_024;
/// Number of recon turns in Balanced phase 1 before forcing `submit_plan`.
pub const BALANCED_RECON_TURNS: usize = 2;
/// Per-call shell output cap (16 KiB).
const MAX_SHELL_OUTPUT_BYTES: usize = 16 * 1_024;

// ---------------------------------------------------------------------------
// Op name sets
// ---------------------------------------------------------------------------

/// All 7 structural grove ops exposed to the model.
pub(crate) const STRUCTURAL_OPS: &[&str] =
    &["outline", "symbols", "source", "check", "callers", "map", "definition"];

/// Subset used during Balanced phase 1 (high-level recon).
pub(crate) const RECON_OPS: &[&str] = &["map", "symbols", "outline", "definition"];

// ---------------------------------------------------------------------------
// JSON-schema helpers
// ---------------------------------------------------------------------------

fn string_param(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn optional_string_param(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn bool_param(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

/// Build an `object` schema with the given required and optional properties.
fn obj_schema(
    required: &[(&str, Value)],
    optional: &[(&str, Value)],
) -> Value {
    let mut props = serde_json::Map::new();
    let mut req_names: Vec<Value> = Vec::new();
    for (name, schema) in required {
        props.insert(name.to_string(), schema.clone());
        req_names.push(json!(name));
    }
    for (name, schema) in optional {
        props.insert(name.to_string(), schema.clone());
    }
    let mut obj = json!({
        "type": "object",
        "properties": props,
        "additionalProperties": false
    });
    if !req_names.is_empty() {
        obj["required"] = Value::Array(req_names);
    }
    obj
}

// ---------------------------------------------------------------------------
// Per-op Tool declarations
// ---------------------------------------------------------------------------

fn tool_outline() -> Tool {
    Tool::function(
        "outline",
        "List definitions in a source file as a compact skeleton (kind, name, signature, line).",
        obj_schema(
            &[("file", string_param("Path to the source file to outline."))],
            &[
                ("kind", optional_string_param(
                    "Only definitions of this kind (e.g. function, class, method).",
                )),
            ],
        ),
    )
}

fn tool_symbols() -> Tool {
    Tool::function(
        "symbols",
        "Find symbols across a directory. Filter by kind and/or name.",
        obj_schema(
            &[("dir", string_param("Directory to search."))],
            &[
                ("kind", optional_string_param("Only this kind, e.g. function, class, method.")),
                ("name", optional_string_param("Exact symbol name (case-insensitive).")),
                ("name_contains", bool_param("Use substring matching for name instead of exact equality.")),
                ("refs", bool_param("Include references, not just definitions.")),
            ],
        ),
    )
}

fn tool_source() -> Tool {
    Tool::function(
        "source",
        "Return the exact full source of one symbol by its id or file+name.",
        obj_schema(
            &[],
            &[
                ("id", optional_string_param("A symbol-id like lang:relpath#name@line.")),
                ("file", optional_string_param("Source file path (use with name).")),
                ("name", optional_string_param("Symbol name to find in file.")),
            ],
        ),
    )
}

fn tool_check() -> Tool {
    Tool::function(
        "check",
        "Parse a source file and report syntax errors with positions.",
        obj_schema(
            &[("file", string_param("Path to the source file to check."))],
            &[],
        ),
    )
}

fn tool_callers() -> Tool {
    Tool::function(
        "callers",
        "Find every reference to a symbol by name across a directory.",
        obj_schema(
            &[
                ("name", string_param("Function/method name to find calls to.")),
            ],
            &[
                ("dir", optional_string_param("Directory to search (default: current).")),
            ],
        ),
    )
}

fn tool_map() -> Tool {
    Tool::function(
        "map",
        "Return a compact structural map of a directory: every definition with its outgoing references.",
        obj_schema(
            &[("dir", string_param("Directory to map."))],
            &[
                ("kind", optional_string_param("Only definitions of this kind.")),
                ("name", optional_string_param("Only definitions whose name equals this (case-insensitive).")),
                ("name_contains", bool_param("Substring matching for name.")),
            ],
        ),
    )
}

fn tool_definition() -> Tool {
    Tool::function(
        "definition",
        "Find where a symbol is defined (go-to-def) by name across a directory.",
        obj_schema(
            &[("name", string_param("Exact symbol name to resolve."))],
            &[
                ("dir", optional_string_param("Directory to search (default: current).")),
            ],
        ),
    )
}

fn tool_submit_plan() -> Tool {
    Tool::function(
        "submit_plan",
        "Commit your exploration plan and transition to phase 2 (execute). \
         Call this when you have a concrete, step-by-step plan for answering the question.",
        obj_schema(
            &[("plan", string_param("Your detailed exploration plan as a plain-text string."))],
            &[],
        ),
    )
}

/// Build a shell tool declaration for an allowed binary.
fn tool_shell(binary: &str) -> Tool {
    Tool::function(
        binary,
        format!(
            "Run `{binary}` in the project root. Pass arguments as a JSON array of strings under \
             the `args` key. Output is captured and returned (truncated if large)."
        ),
        obj_schema(
            &[("args", json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "Arguments to pass to the binary."
            }))],
            &[],
        ),
    )
}

// ---------------------------------------------------------------------------
// Toolset builders
// ---------------------------------------------------------------------------

/// Return Tool declarations for all 7 structural grove ops plus any
/// shell binaries in `allowed_shell`.
///
/// Used for Standard and Aggressive modes, and for Balanced phase 2.
pub fn build_full_toolset(allowed_shell: &[String]) -> Vec<Tool> {
    let mut tools = vec![
        tool_outline(),
        tool_symbols(),
        tool_source(),
        tool_check(),
        tool_callers(),
        tool_map(),
        tool_definition(),
    ];
    // Verify every name in STRUCTURAL_OPS is present (catches accidental drift).
    debug_assert!(
        STRUCTURAL_OPS
            .iter()
            .all(|op| tools.iter().any(|t| &t.function.name == op)),
        "STRUCTURAL_OPS out of sync with build_full_toolset"
    );
    for binary in allowed_shell {
        tools.push(tool_shell(binary));
    }
    tools
}

/// Return Tool declarations for the recon subset (RECON_OPS) plus `submit_plan`.
///
/// Used for Balanced phase 1. Shell binaries are deliberately excluded to keep
/// the recon phase focused on structural understanding.
pub fn build_recon_toolset() -> Vec<Tool> {
    let tools = vec![
        tool_map(),
        tool_symbols(),
        tool_outline(),
        tool_definition(),
        tool_submit_plan(),
    ];
    // Verify every name in RECON_OPS is present (catches accidental drift).
    debug_assert!(
        RECON_OPS
            .iter()
            .all(|op| tools.iter().any(|t| &t.function.name == op)),
        "RECON_OPS out of sync with build_recon_toolset"
    );
    tools
}

/// Return a toolset containing only `submit_plan`.
///
/// Used to force plan commitment after `BALANCED_RECON_TURNS` in Balanced mode.
pub fn build_submit_only_toolset() -> Vec<Tool> {
    vec![tool_submit_plan()]
}

/// Return true if `name` is the function name of any tool in `active`.
pub fn is_in_toolset(name: &str, active: &[Tool]) -> bool {
    active.iter().any(|t| t.function.name == name)
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Dispatch a tool call to the appropriate handler and return the result as a
/// JSON string (or an error/refusal message string).
///
/// **Routing:**
/// - Structural grove ops → `crate::ops::*` in-process (no subprocess).
/// - Shell binaries in `allowed_shell` → `std::process::Command` with args as a
///   `Vec<String>` (no shell interpolation), cwd = `root`. Binary is validated
///   against the allowlist before `Command::new` is ever called.
/// - Unrecognized names → return a corrective refusal string.
///
/// Shell output is truncated to `MAX_SHELL_OUTPUT_BYTES`.
pub fn dispatch_tool(
    name: &str,
    args: &Value,
    root: &Path,
    allowed_shell: &[String],
) -> String {
    match name {
        "outline" => dispatch_outline(args, root),
        "symbols" => dispatch_symbols(args, root),
        "source" => dispatch_source(args),
        "check" => dispatch_check(args, root),
        "callers" => dispatch_callers(args, root),
        "map" => dispatch_map(args, root),
        "definition" => dispatch_definition(args, root),
        other => {
            // Check shell allowlist — binary must be in allowed_shell.
            if allowed_shell.iter().any(|b| b == other) {
                dispatch_shell(other, args, root)
            } else {
                format!(
                    "{{\"error\": \"tool '{other}' is not available in the active toolset. \
                     Use one of the offered tools.\"}}",
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Structural op dispatchers
// ---------------------------------------------------------------------------

fn dispatch_outline(args: &Value, root: &Path) -> String {
    let file_str = match args["file"].as_str() {
        Some(s) => s,
        None => return r#"{"error": "outline: missing required argument 'file'"}"#.to_string(),
    };
    let file = root.join(file_str);
    let kind = args["kind"].as_str();
    match ops::outline(&file, kind) {
        Ok(syms) => serde_json::to_string(&syms).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

fn dispatch_symbols(args: &Value, root: &Path) -> String {
    let dir_str = match args["dir"].as_str() {
        Some(s) => s,
        None => return r#"{"error": "symbols: missing required argument 'dir'"}"#.to_string(),
    };
    let dir = root.join(dir_str);
    let kind = args["kind"].as_str();
    let name = args["name"].as_str();
    let refs = args["refs"].as_bool().unwrap_or(false);
    let name_contains = args["name_contains"].as_bool().unwrap_or(false);
    match ops::symbols(&dir, kind, name, refs, name_contains) {
        Ok(syms) => serde_json::to_string(&syms).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

fn dispatch_source(args: &Value) -> String {
    // Accept either id or file+name
    let id_or_file = args["id"]
        .as_str()
        .or_else(|| args["file"].as_str())
        .unwrap_or("");
    if id_or_file.is_empty() {
        return r#"{"error": "source: provide 'id' or 'file' argument"}"#.to_string();
    }
    let name = args["name"].as_str();
    match ops::source(id_or_file, name) {
        Ok(result) => serde_json::to_string(&result).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

fn dispatch_check(args: &Value, root: &Path) -> String {
    let file_str = match args["file"].as_str() {
        Some(s) => s,
        None => return r#"{"error": "check: missing required argument 'file'"}"#.to_string(),
    };
    let file = root.join(file_str);
    match ops::check(&file) {
        Ok(defects) => serde_json::to_string(&defects).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

fn dispatch_callers(args: &Value, root: &Path) -> String {
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return r#"{"error": "callers: missing required argument 'name'"}"#.to_string(),
    };
    let dir = args["dir"].as_str().map(|d| root.join(d));
    let search_dir = dir.as_deref().unwrap_or(root);
    match ops::callers(search_dir, name) {
        Ok(sites) => serde_json::to_string(&sites).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

fn dispatch_map(args: &Value, root: &Path) -> String {
    let dir_str = match args["dir"].as_str() {
        Some(s) => s,
        None => return r#"{"error": "map: missing required argument 'dir'"}"#.to_string(),
    };
    let dir = root.join(dir_str);
    let kind = args["kind"].as_str();
    let name = args["name"].as_str();
    let name_contains = args["name_contains"].as_bool().unwrap_or(false);
    match ops::map(&dir, kind, name, name_contains) {
        Ok(file_maps) => serde_json::to_string(&file_maps).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

fn dispatch_definition(args: &Value, root: &Path) -> String {
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return r#"{"error": "definition: missing required argument 'name'"}"#.to_string(),
    };
    let dir = args["dir"].as_str().map(|d| root.join(d));
    let search_dir = dir.as_deref().unwrap_or(root);
    match ops::definition(search_dir, name) {
        Ok(syms) => serde_json::to_string(&syms).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
    }
}

// ---------------------------------------------------------------------------
// Shell dispatcher
// ---------------------------------------------------------------------------

fn dispatch_shell(binary: &str, args: &Value, root: &Path) -> String {
    // Build args vec — accept either `args: [...]` or fall back to empty.
    let shell_args: Vec<String> = match args["args"].as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        None => Vec::new(),
    };

    let result = Command::new(binary)
        .args(&shell_args)
        .current_dir(root)
        .output();

    match result {
        Ok(output) => {
            let mut combined = String::new();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            combined.push_str(&stdout);
            if !stderr.is_empty() {
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str("[stderr] ");
                combined.push_str(&stderr);
            }
            if combined.len() > MAX_SHELL_OUTPUT_BYTES {
                combined.truncate(MAX_SHELL_OUTPUT_BYTES);
                combined.push_str("\n[output truncated]");
            }
            combined
        }
        Err(e) => format!("{{\"error\": \"failed to run '{binary}': {e}\"}}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_toolset_contains_all_seven_ops() {
        let tools = build_full_toolset(&[]);
        let names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
        for op in super::STRUCTURAL_OPS {
            assert!(
                names.contains(op),
                "full toolset missing structural op '{op}'"
            );
        }
        assert_eq!(tools.len(), super::STRUCTURAL_OPS.len(), "exactly 7 ops, no extras");
    }

    #[test]
    fn aggressive_toolset_same_as_standard() {
        // Aggressive mode uses prompt-only steering (AC-4) — the tool schema
        // is identical to Standard, not a different set.
        let standard = build_full_toolset(&[]);
        let aggressive = build_full_toolset(&[]);
        let std_names: Vec<&str> = standard.iter().map(|t| t.function.name.as_str()).collect();
        let agg_names: Vec<&str> = aggressive.iter().map(|t| t.function.name.as_str()).collect();
        assert_eq!(std_names, agg_names, "aggressive schema == standard schema");
    }

    #[test]
    fn balanced_phase1_toolset_recon_plus_submit_plan() {
        let tools = build_recon_toolset();
        let names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
        for op in super::RECON_OPS {
            assert!(names.contains(op), "recon toolset missing '{op}'");
        }
        assert!(names.contains(&"submit_plan"), "recon toolset must include submit_plan");
        // No full structural ops beyond RECON_OPS.
        for non_recon in ["source", "check", "callers"] {
            assert!(
                !names.contains(&non_recon),
                "recon toolset must NOT include '{non_recon}'"
            );
        }
    }

    #[test]
    fn shell_binary_not_in_allowlist_is_refused() {
        let root = std::env::temp_dir();
        let args = json!({ "args": ["--version"] });
        let result = dispatch_tool("curl", &args, &root, &[]);
        assert!(
            result.contains("not available"),
            "expected refusal for non-allowlisted tool, got: {result}"
        );
    }

    #[test]
    fn is_in_toolset_matches_by_function_name() {
        let tools = build_full_toolset(&[]);
        assert!(is_in_toolset("outline", &tools));
        assert!(is_in_toolset("map", &tools));
        assert!(!is_in_toolset("submit_plan", &tools));
        assert!(!is_in_toolset("nonexistent", &tools));
    }

    #[test]
    fn submit_only_toolset_has_exactly_one_tool() {
        let tools = build_submit_only_toolset();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "submit_plan");
    }

    #[test]
    fn full_toolset_includes_allowed_shell_binaries() {
        let allowed = vec!["grep".to_string(), "rg".to_string()];
        let tools = build_full_toolset(&allowed);
        let names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
        assert!(names.contains(&"grep"), "grep should be in toolset");
        assert!(names.contains(&"rg"), "rg should be in toolset");
        // 7 structural ops + 2 shell = 9 total
        assert_eq!(tools.len(), 9);
    }
}
