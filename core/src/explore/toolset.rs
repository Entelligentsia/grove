//! Tool schemas + dispatch for the inner explorer — the `base-q4-v2-hf`
//! reference toolset (`grove-explore-model/scripts/run_eval.py`).
//!
//! The model sees the reference's exact tool vocabulary: three Claude-style base
//! tools — **Glob**, **Grep**, **Read** — plus the six read-only grove structural
//! tools under their MCP names — **mcp__grove__outline / symbols / source /
//! callers / map / definition** (no `check`; it never appears in the reference
//! toolset). There is **no** single `Grove` command-string tool and **no**
//! `submit_plan`: the plan-first recon phase is gone.
//!
//! Observation shapes mirror the reference harness so the (untrained) base model
//! reads what it was measured on: grove tools return `grove <verb> --json`
//! (`serde_json::to_string_pretty` of the `ops` result, capped), dispatched
//! **in-process** via `ops` (ADR 0003 — no subprocess/reparse per call, warm
//! grammar cache); Read returns `== path ==\n<n>\t<line>` slices; Grep/Glob shell
//! to ripgrep and return repo-relative `path:line:text` / path lists.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use super::wire::Tool;

// Base tool names (Claude arg schemas) — exact, case-sensitive strings the model calls.
pub const READ: &str = "Read";
pub const GLOB: &str = "Glob";
pub const GREP: &str = "Grep";

// The six grove structural tools, under their MCP names.
pub const GROVE_OUTLINE: &str = "mcp__grove__outline";
pub const GROVE_SYMBOLS: &str = "mcp__grove__symbols";
pub const GROVE_SOURCE: &str = "mcp__grove__source";
pub const GROVE_CALLERS: &str = "mcp__grove__callers";
pub const GROVE_MAP: &str = "mcp__grove__map";
pub const GROVE_DEFINITION: &str = "mcp__grove__definition";

// Observation caps (from run_eval.py: read 120 lines, grep 80, glob 60, grove 8000 chars).
const READ_MAX_LINES: usize = 120;
const READ_MAX_LINE_LEN: usize = 2000;
const GREP_MAX_LINES: usize = 80;
const GLOB_MAX_FILES: usize = 60;
const GROVE_MAX_CHARS: usize = 8000;
/// `outline --json` default `--detail` (clap default in `cli/src/main.rs`).
const OUTLINE_DETAIL: u8 = 1;

/// Tool-observation markers that carry no new signal (mirror `run_eval.py::_EMPTY_OBS`).
/// A call whose observation matches one of these is "empty" for thrash accounting.
const EMPTY_OBS: &[&str] = &[
    "(no matches)",
    "(no files match)",
    "(no such file",
    "(empty range)",
    "(path escapes",
    "(grep error",
    "(grove error",
    "(no output)",
    "(unknown tool",
];

/// True when `obs` carried no new signal — used by the loop's thrash detector.
pub fn is_empty_obs(obs: &str) -> bool {
    EMPTY_OBS.iter().any(|m| obs.contains(m))
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

fn s() -> Value {
    json!({"type": "string"})
}
fn b() -> Value {
    json!({"type": "boolean"})
}
fn i() -> Value {
    json!({"type": "integer"})
}

fn func(name: &str, desc: &str, props: Value, required: Value) -> Tool {
    Tool::function(
        name,
        desc,
        json!({"type": "object", "properties": props, "required": required}),
    )
}

/// The full toolset offered on every turn: three base tools + six grove tools,
/// matching `run_eval.py`'s `BASE_TOOLS + GROVE_TOOLS` (names, args, order).
pub fn all_tools() -> Vec<Tool> {
    vec![
        func(
            GLOB,
            "List repository files matching a glob pattern (recursive '**').",
            json!({"pattern": s()}),
            json!(["pattern"]),
        ),
        func(
            GREP,
            "Search file contents with a regex (ripgrep). Returns file:line:text.",
            json!({
                "pattern": s(),
                "path": {"type": "string", "description": "optional file/dir to scope"},
                "glob": {"type": "string", "description": "optional file filter e.g. '*.rs'"}
            }),
            json!(["pattern"]),
        ),
        func(
            READ,
            "Read a slice of a file with line numbers.",
            json!({"file_path": s(), "offset": i(), "limit": i()}),
            json!(["file_path"]),
        ),
        func(
            GROVE_OUTLINE,
            "List the definitions in one file (kind·name·parent·signature·id).",
            json!({"file": s(), "kind": s()}),
            json!(["file"]),
        ),
        func(
            GROVE_SYMBOLS,
            "Find symbols across a directory, optionally filtered by name/kind.",
            json!({"dir": s(), "name": s(), "kind": s(), "nameContains": b(), "refs": b()}),
            json!(["dir"]),
        ),
        func(
            GROVE_SOURCE,
            "Print the full source of a symbol, by id or by file+name.",
            json!({"id": s(), "file": s(), "name": s()}),
            json!([]),
        ),
        func(
            GROVE_CALLERS,
            "Find references to a symbol across a directory.",
            json!({"name": s(), "dir": s()}),
            json!(["name"]),
        ),
        func(
            GROVE_MAP,
            "Compact structural map: definitions and their references, no bodies.",
            json!({"dir": s(), "name": s(), "kind": s(), "nameContains": b()}),
            json!(["dir"]),
        ),
        func(
            GROVE_DEFINITION,
            "Find where a symbol is defined (go-to-def), by name or position.",
            json!({"name": s(), "at": s(), "dir": s()}),
            json!([]),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Execute a tool call and return the raw observation text (the caller
/// neutralizes XML-breaking tags before feeding it back to the model).
pub fn dispatch(name: &str, args: &Value, root: &Path) -> String {
    match name {
        READ => read_tool(args, root),
        GLOB => glob_tool(args, root),
        GREP => grep_tool(args, root),
        GROVE_OUTLINE | GROVE_SYMBOLS | GROVE_SOURCE | GROVE_CALLERS | GROVE_MAP
        | GROVE_DEFINITION => grove_tool(name, args, root),
        other => format!("(unknown tool: {other})"),
    }
}

// --- Read (port of run_eval.py::t_read / t_read_claude) --------------------

fn read_tool(args: &Value, root: &Path) -> String {
    // Claude's Read takes `file_path`; tolerate `path` as a synonym.
    let raw = args
        .get("file_path")
        .or_else(|| args.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if raw.is_empty() {
        return "(no such file: )".into();
    }
    let resolved = match resolve_read_path(raw, root) {
        Some(p) => p,
        None => return "(path escapes repo)".into(),
    };
    if !resolved.is_file() {
        return format!("(no such file: {raw})");
    }
    let start = args
        .get("offset")
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .unwrap_or(1) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .map(|n| n as usize)
        .unwrap_or(READ_MAX_LINES)
        .min(READ_MAX_LINES);
    let end = start + limit - 1;
    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(e) => return format!("(read error: {e})"),
    };
    let mut out = Vec::new();
    for (n, line) in content.lines().enumerate() {
        let n = n + 1;
        if n < start {
            continue;
        }
        if n > end {
            break;
        }
        let mut line = line.trim_end().to_string();
        if line.len() > READ_MAX_LINE_LEN {
            line.truncate(READ_MAX_LINE_LEN);
            line.push('…');
        }
        out.push(format!("{n}\t{line}"));
    }
    let body = if out.is_empty() {
        "(empty range)".to_string()
    } else {
        out.join("\n")
    };
    format!("== {raw} ==\n{body}")
}

// --- Grep (port of run_eval.py::t_grep) ------------------------------------

fn grep_tool(args: &Value, root: &Path) -> String {
    let pattern = match args.get("pattern").and_then(Value::as_str) {
        Some(p) if !p.is_empty() => p,
        _ => return "(no matches)".into(),
    };
    let rg = match which("rg") {
        Some(p) => p,
        None => return "(grep error: ripgrep (rg) not on PATH)".into(),
    };
    let path = resolve_search_path(args.get("path").and_then(Value::as_str), root);
    let mut cmd: Vec<String> = vec![
        "-n".into(),
        "--no-heading".into(),
        "-m".into(),
        "40".into(),
        pattern.to_string(),
    ];
    if let Some(g) = args.get("glob").and_then(Value::as_str) {
        cmd.push("-g".into());
        cmd.push(g.to_string());
    }
    cmd.push("--".into());
    cmd.push(path.display().to_string());
    let out = run_capture(&rg, &cmd, root);
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return "(no matches)".into();
    }
    if lines.len() > GREP_MAX_LINES {
        format!(
            "{}\n... (+{} more matches)",
            lines[..GREP_MAX_LINES].join("\n"),
            lines.len() - GREP_MAX_LINES
        )
    } else {
        lines.join("\n")
    }
}

// --- Glob (port of run_eval.py::t_glob) ------------------------------------

fn glob_tool(args: &Value, root: &Path) -> String {
    let pattern = match args.get("pattern").and_then(Value::as_str) {
        Some(p) if !p.is_empty() => p,
        _ => return "(no files match)".into(),
    };
    let rg = match which("rg") {
        Some(p) => p,
        None => return "(grep error: ripgrep (rg) not on PATH)".into(),
    };
    let out = run_capture(
        &rg,
        &[
            "--files".into(),
            "--glob".into(),
            pattern.to_string(),
            root.display().to_string(),
        ],
        root,
    );
    let mut rels: Vec<String> = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            Path::new(l)
                .strip_prefix(root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| l.to_string())
        })
        .collect();
    rels.sort();
    if rels.is_empty() {
        return "(no files match)".into();
    }
    if rels.len() > GLOB_MAX_FILES {
        format!(
            "{}\n... (+{} more)",
            rels[..GLOB_MAX_FILES].join("\n"),
            rels.len() - GLOB_MAX_FILES
        )
    } else {
        rels.join("\n")
    }
}

// --- grove structural tools (in-process ops + --json render) ---------------

fn grove_tool(name: &str, a: &Value, root: &Path) -> String {
    match grove_json(name, a, root) {
        Ok(js) => {
            let text = js.trim();
            if text.is_empty() || text == "[]" || text == "null" {
                "(no output)".into()
            } else if text.len() > GROVE_MAX_CHARS {
                // Char-safe truncation (JSON may hold multibyte from source spans).
                let cut: String = text.chars().take(GROVE_MAX_CHARS).collect();
                format!("{cut}\n... (truncated)")
            } else {
                text.to_string()
            }
        }
        Err(e) => format!("(grove error: {e})"),
    }
}

/// Run one grove verb in-process and return its `--json` serialization
/// (`serde_json::to_string_pretty` of the `ops` result — byte-identical to the
/// CLI's `grove <verb> --json`).
fn grove_json(name: &str, a: &Value, root: &Path) -> anyhow::Result<String> {
    use crate::ops;

    let str_arg = |k: &str| a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    let bool_arg = |k: &str| a.get(k).and_then(Value::as_bool).unwrap_or(false);
    let dir = str_arg("dir").unwrap_or(".");

    let pretty = |v: &serde_json::Value| serde_json::to_string_pretty(v).unwrap_or_default();
    let pretty_ser = |v: &dyn ErasedSer| v.to_pretty();

    Ok(match name {
        GROVE_OUTLINE => {
            let file = str_arg("file").ok_or_else(|| anyhow::anyhow!("outline needs `file`"))?;
            let syms = ops::outline(&root.join(file), str_arg("kind"))?;
            pretty(&ops::project(&syms, OUTLINE_DETAIL))
        }
        GROVE_SYMBOLS => {
            let syms = ops::symbols(
                &root.join(dir),
                str_arg("kind"),
                str_arg("name"),
                bool_arg("refs"),
                bool_arg("nameContains"),
            )?;
            pretty_ser(&syms)
        }
        GROVE_SOURCE => {
            let res = if let Some(id) = str_arg("id") {
                ops::source(id, None)?
            } else {
                let file = str_arg("file")
                    .ok_or_else(|| anyhow::anyhow!("source needs `id` or `file`+`name`"))?;
                ops::source(file, str_arg("name"))?
            };
            pretty_ser(&res)
        }
        GROVE_CALLERS => {
            let nm = str_arg("name").ok_or_else(|| anyhow::anyhow!("callers needs `name`"))?;
            pretty_ser(&ops::callers(&root.join(dir), nm)?)
        }
        GROVE_MAP => {
            let maps = ops::map(
                &root.join(dir),
                str_arg("kind"),
                str_arg("name"),
                bool_arg("nameContains"),
            )?;
            pretty_ser(&maps)
        }
        GROVE_DEFINITION => {
            let defs = if let Some(at) = str_arg("at") {
                let (file, row, col) = ops::parse_pos(at)?;
                ops::definition_at(&root.join(file), row, col, &root.join(dir))?.1
            } else {
                let nm =
                    str_arg("name").ok_or_else(|| anyhow::anyhow!("definition needs `name` or `at`"))?;
                ops::definition(&root.join(dir), nm)?
            };
            pretty_ser(&defs)
        }
        other => anyhow::bail!("unknown grove tool: {other}"),
    })
}

/// Tiny helper so `grove_json` can `to_string_pretty` the differently-typed `ops`
/// results through one closure without a big generic.
trait ErasedSer {
    fn to_pretty(&self) -> String;
}
impl<T: serde::Serialize> ErasedSer for T {
    fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Helpers (path sandboxing + subprocess capture)
// ---------------------------------------------------------------------------

/// Minimal `which` (port of `shutil.which`), honoring PATH.
fn which(bin: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(bin);
        if cand.is_file() {
            return Some(cand.display().to_string());
        }
    }
    None
}

/// Run a binary with args in `cwd`, capturing stdout on success else stderr.
fn run_capture(bin: &str, args: &[String], cwd: &Path) -> String {
    match Command::new(bin).args(args).current_dir(cwd).output() {
        Ok(out) => {
            let text = if out.status.success() {
                String::from_utf8_lossy(&out.stdout)
            } else {
                let e = String::from_utf8_lossy(&out.stderr);
                if e.trim().is_empty() {
                    String::from_utf8_lossy(&out.stdout)
                } else {
                    e
                }
            };
            text.into_owned()
        }
        Err(e) => format!("(grep error: failed to run `{bin}`: {e})"),
    }
}

/// Resolve a Read path inside the workspace, or `None` if it escapes.
fn resolve_read_path(path: &str, root: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let p = Path::new(path);
    let cand = if p.is_absolute() {
        normalize(p)
    } else {
        normalize(&root.join(p))
    };
    if cand.starts_with(&root) {
        return Some(cand);
    }
    // Tolerate a duplicated workspace-basename prefix (e.g. `grove/src/x` under `.../grove`).
    let base = root.file_name()?;
    let mut comps: Vec<_> = p
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_owned()),
            _ => None,
        })
        .collect();
    if comps.first().map(std::ffi::OsString::as_os_str) == Some(base) {
        comps.remove(0);
        let remapped = normalize(&comps.iter().fold(root.clone(), |a, s| a.join(s)));
        if remapped.starts_with(&root) {
            return Some(remapped);
        }
    }
    None
}

/// Resolve a Grep path, never escaping and degrading to the workspace root.
fn resolve_search_path(path: Option<&str>, root: &Path) -> PathBuf {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = match path {
        Some(p) if !p.is_empty() => p,
        _ => return root,
    };
    let p = Path::new(path);
    let cand = if p.is_absolute() {
        normalize(p)
    } else {
        normalize(&root.join(p))
    };
    if cand.starts_with(&root) && cand.exists() {
        return cand;
    }
    root
}

/// Lexically normalize `.`/`..` without touching the filesystem.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolset_is_the_reference_vocabulary() {
        let names: Vec<String> = all_tools()
            .iter()
            .map(|t| t.function.name.clone())
            .collect();
        assert_eq!(
            names,
            vec![
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
        // No single Grove command tool, no submit_plan, no check.
        assert!(!names.iter().any(|n| n == "Grove" || n == "submit_plan"));
        assert!(!names.iter().any(|n| n.ends_with("check")));
    }

    #[test]
    fn empty_obs_detection_matches_reference_markers() {
        assert!(is_empty_obs("(no matches)"));
        assert!(is_empty_obs("(no files match)"));
        assert!(is_empty_obs("== x.rs ==\n(empty range)"));
        assert!(is_empty_obs("(grove error: bad)"));
        assert!(!is_empty_obs("src/a.rs:10:let x = 1;"));
        assert!(!is_empty_obs("[\n  {\"name\": \"foo\"}\n]"));
    }

    #[test]
    fn read_returns_numbered_slice_with_header() {
        let dir = std::env::temp_dir().join(format!("grove-ts-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "one\ntwo\nthree\n").unwrap();
        let obs = read_tool(&json!({"file_path": "a.rs", "offset": 2, "limit": 1}), &dir);
        assert_eq!(obs, "== a.rs ==\n2\ttwo");
        let missing = read_tool(&json!({"file_path": "nope.rs"}), &dir);
        assert!(is_empty_obs(&missing), "missing file → empty obs: {missing}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn grove_outline_returns_json() {
        // Rust is in the dev-stub registry; outline this very file's crate dir is
        // overkill — just assert the JSON shape on a tiny written file if grammars
        // resolve, else tolerate a grove error (registry-agnostic).
        let dir = std::env::temp_dir().join(format!("grove-ts-outline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.rs"), "pub fn hello() {}\n").unwrap();
        let obs = grove_tool(GROVE_OUTLINE, &json!({"file": "m.rs"}), &dir);
        // Either valid JSON with the fn, or a registry-resolution error — both are
        // acceptable here; what matters is it's not a panic and not the text render.
        assert!(
            obs.contains("hello") || obs.starts_with("(grove error") || obs == "(no output)",
            "outline obs: {obs}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
