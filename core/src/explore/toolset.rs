//! Tool schemas + dispatch — a direct port of the reference bench's
//! `agent/tool/{read,glob,grep,grove}.py` and `mcp_server.py`'s `submit_plan`.
//!
//! The inner explorer sees exactly four tools — **Read**, **Glob**, **Grep**,
//! **Grove** — plus **submit_plan** during the plan-first recon phase. This is
//! the toolset the study validated; it is deliberately NOT grove's 7 structural
//! MCP tools. `Grove` is a single command-string tool that runs the read-only
//! structural verbs **in-process** via `ops` + [`crate::render`] (ADR 0003) —
//! same text the CLI prints, but no subprocess spawn or reparse per call.
//! `Grep`/`Glob` shell to ripgrep; `Read` is in-process.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use super::client::Tool;

// Tool names (exact, case-sensitive — the model calls these strings).
pub const READ: &str = "Read";
pub const GLOB: &str = "Glob";
pub const GREP: &str = "Grep";
pub const GROVE: &str = "Grove";
pub const SUBMIT_PLAN: &str = "submit_plan";

/// Grove verbs allowed at all (read-only exploration), from `tool/grove.py`.
pub const ALLOWED_VERBS: &[&str] = &["outline", "symbols", "source", "callers", "definition", "map"];
/// Grove verbs allowed during plan-first recon, from `mcp_server.py::RECON_VERBS`.
pub const RECON_VERBS: &[&str] = &["map", "symbols", "outline", "definition"];

// Read caps (read.py).
const MAX_READ_LINES: usize = 2000;
const MAX_READ_LINE_LEN: usize = 2000;
// Output line caps (grove.py: 120; grep/glob: 100).
const GROVE_MAX_LINES: usize = 120;
const RG_MAX_LINES: usize = 100;

// Tool descriptions, embedded verbatim from the vendored prompts.
const DESC_READ: &str = include_str!("prompts/tool_read.md");
const DESC_GLOB: &str = include_str!("prompts/tool_glob.md");
const DESC_GREP: &str = include_str!("prompts/tool_grep.md");
const DESC_GROVE: &str = include_str!("prompts/tool_grove.md");

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

fn read_schema() -> Tool {
    Tool::function(
        READ,
        DESC_READ,
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The path of the file to read."},
                "offset": {"type": "integer", "description": "1-indexed start line. Only for large files."},
                "limit": {"type": "integer", "description": "Number of lines to read. Only for large files."}
            },
            "required": ["path"]
        }),
    )
}

fn glob_schema() -> Tool {
    Tool::function(
        GLOB,
        DESC_GLOB,
        json!({
            "type": "object",
            "properties": {
                "directory": {"type": "string", "description": "Directory to search in. Defaults to the workspace root."},
                "pattern": {"type": "string", "description": "The glob pattern to match files."}
            },
            "required": ["pattern"]
        }),
    )
}

fn grep_schema() -> Tool {
    Tool::function(
        GREP,
        DESC_GREP,
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "The regular expression to search for in file contents."},
                "path": {"type": "string", "description": "File or directory to search in. Defaults to the workspace root."},
                "glob": {"type": "string", "description": "Glob to filter files (e.g. \"*.rs\", \"*.{ts,tsx}\")."},
                "output_mode": {"type": "string", "enum": ["content", "files_with_matches", "count"], "description": "content shows matching lines; files_with_matches shows paths; count shows counts. Default files_with_matches."},
                "-B": {"type": "number", "description": "Lines before each match (content mode)."},
                "-A": {"type": "number", "description": "Lines after each match (content mode)."},
                "-C": {"type": "number", "description": "Lines around each match (content mode)."},
                "-n": {"type": "boolean", "description": "Show line numbers (content mode). Default true."},
                "-i": {"type": "boolean", "description": "Case-insensitive search."},
                "type": {"type": "string", "description": "File type to search (rg --type): rs, py, js, go, java, …"},
                "head_limit": {"type": "number", "minimum": 0, "description": "Limit output to first N lines/entries."},
                "multiline": {"type": "boolean", "description": "Enable multiline mode. Default false."}
            },
            "required": ["pattern"]
        }),
    )
}

fn grove_schema() -> Tool {
    Tool::function(
        GROVE,
        DESC_GROVE,
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "grove CLI arguments WITHOUT the leading 'grove'. e.g. \"symbols . --kind function --name-contains --name rename\", \"outline merge-ort.c\", or \"source c:merge-ort.c#detect_regular_renames@1600\". Allowed verbs: outline, symbols, source, callers, definition, map."
                }
            },
            "required": ["command"]
        }),
    )
}

/// The `submit_plan` schema, verbatim from `mcp_server.py::PLAN_SCHEMA`.
pub fn submit_plan_schema() -> Tool {
    Tool::function(
        SUBMIT_PLAN,
        "Record your focus area and unlock the execution tools (Read, Grep, Glob, and Grove source/callers). Call this after 1-2 Grove structure calls, once you know where the answer lives.",
        json!({
            "type": "object",
            "properties": {
                "focus_files": {"type": "string", "description": "the 2-5 files/dirs to investigate"},
                "focus_symbols": {"type": "string", "description": "key functions/types, with grove ids where known"},
                "steps": {"type": "string", "description": "ordered sub-goals; for each, what to find and which tool (source/Read/Grep)"}
            },
            "required": ["focus_files", "steps"]
        }),
    )
}

/// Full execution toolset (merit/strict, and plan-first phase 2).
pub fn execute_toolset() -> Vec<Tool> {
    vec![read_schema(), glob_schema(), grep_schema(), grove_schema()]
}

/// Recon toolset (plan-first phase 1): Grove (structure) while `grove_open`,
/// always `submit_plan`. Mirrors `_instrumented_loop`'s schema selection.
pub fn recon_toolset(grove_open: bool) -> Vec<Tool> {
    let mut tools = Vec::new();
    if grove_open {
        tools.push(grove_schema());
    }
    tools.push(submit_plan_schema());
    tools
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Extract the leading grove verb from a `Grove` `command` argument.
pub fn grove_verb(args: &Value) -> String {
    args.get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

/// Execute a tool call and return the observation text. `submit_plan` is handled
/// by the agent loop and never reaches here.
pub fn dispatch(name: &str, args: &Value, root: &Path) -> String {
    match name {
        READ => read_tool(args, root),
        GLOB => glob_tool(args, root),
        GREP => grep_tool(args, root),
        GROVE => grove_tool(args, root),
        other => format!("<system-reminder>Tool `{other}` not found.</system-reminder>"),
    }
}

// --- Read (port of tool/read.py) -------------------------------------------

fn read_tool(args: &Value, root: &Path) -> String {
    let file_path = match args.get("path").and_then(Value::as_str) {
        Some(p) if !p.is_empty() => p,
        _ => return "<system-reminder>Error: file path is required</system-reminder>".into(),
    };
    let offset = args.get("offset").and_then(Value::as_i64).unwrap_or(1);
    if offset <= 0 {
        return "<system-reminder>Error: offset must be a positive integer</system-reminder>".into();
    }
    let limit = args.get("limit").and_then(Value::as_i64);
    if let Some(l) = limit {
        if l <= 0 {
            return "<system-reminder>Error: limit must be a positive integer</system-reminder>"
                .into();
        }
    }
    let resolved = match resolve_read_path(file_path, root) {
        Some(p) => p,
        None => {
            return format!(
                "<system-reminder>Permission error: `{}` is not within the working directory `{}`</system-reminder>",
                file_path,
                root.display()
            )
        }
    };
    if !resolved.exists() {
        return format!(
            "<system-reminder>Error: {} does not exist</system-reminder>",
            resolved.display()
        );
    }
    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(e) => return format!("<system-reminder>Error reading file: {e}</system-reminder>"),
    };
    let raw_lines: Vec<&str> = content.split_inclusive('\n').collect();
    if raw_lines.is_empty() {
        return "File is empty.".into();
    }
    let offset = offset as usize;
    if offset > raw_lines.len() {
        return "File is empty.".into();
    }
    let mut end_line = match limit {
        Some(l) => offset + l as usize - 1,
        None => raw_lines.len(),
    };
    if end_line > raw_lines.len() {
        end_line = raw_lines.len();
    }
    let total = end_line.saturating_sub(offset) + 1;
    if total > MAX_READ_LINES {
        end_line = offset + MAX_READ_LINES - 1;
    }
    let mut out = String::new();
    for (i, raw) in raw_lines.iter().enumerate().take(end_line).skip(offset - 1) {
        let mut line = (*raw).to_string();
        if line.len() > MAX_READ_LINE_LEN {
            line.truncate(MAX_READ_LINE_LEN);
            line.push_str("...\n");
        }
        out.push_str(&format!("{}|{}", i + 1, line));
    }
    if total > MAX_READ_LINES {
        out.push_str("...");
    }
    format!(
        "```{}:{}-{}\n{}\n```",
        resolved.display(),
        offset,
        end_line,
        out
    )
}

// --- Glob (port of tool/glob.py) -------------------------------------------

fn glob_tool(args: &Value, root: &Path) -> String {
    let pattern = match args.get("pattern").and_then(Value::as_str) {
        Some(p) => p,
        None => return "<system-reminder>Error: pattern is required</system-reminder>".into(),
    };
    let dir = resolve_search_path(args.get("directory").and_then(Value::as_str), root);
    let rg = match rg_path() {
        Some(p) => p,
        None => return "<system-reminder>Glob requires ripgrep (rg) on PATH.</system-reminder>".into(),
    };
    let out = run_capture(
        &rg,
        &[
            "--files".into(),
            dir.display().to_string(),
            "--glob".into(),
            pattern.to_string(),
        ],
        root,
    );
    let mut lines: Vec<String> = out.lines().map(str::to_string).collect();
    if lines.len() > RG_MAX_LINES {
        lines.truncate(RG_MAX_LINES);
        lines.push(format!(
            "Results are truncated: showing first {RG_MAX_LINES} results. Consider using a more specific path or pattern."
        ));
    }
    if lines.is_empty() {
        return "No files found".into();
    }
    lines.join("\n")
}

// --- Grep (port of tool/grep.py) -------------------------------------------

fn grep_tool(args: &Value, root: &Path) -> String {
    let pattern = match args.get("pattern").and_then(Value::as_str) {
        Some(p) => p,
        None => return "<system-reminder>Error: pattern is required</system-reminder>".into(),
    };
    let rg = match rg_path() {
        Some(p) => p,
        None => return "<system-reminder>Grep requires ripgrep (rg) on PATH.</system-reminder>".into(),
    };
    let path = resolve_search_path(args.get("path").and_then(Value::as_str), root);
    let mut cmd: Vec<String> = vec![pattern.to_string(), path.display().to_string()];
    if let Some(g) = args.get("glob").and_then(Value::as_str) {
        cmd.push("--glob".into());
        cmd.push(g.to_string());
    }
    if args.get("-i").and_then(Value::as_bool).unwrap_or(false) {
        cmd.push("--ignore-case".into());
    }
    if let Some(t) = args.get("type").and_then(Value::as_str) {
        cmd.push("--type".into());
        cmd.push(t.to_string());
    }
    if args.get("multiline").and_then(Value::as_bool).unwrap_or(false) {
        cmd.push("--multiline".into());
        cmd.push("--multiline-dotall".into());
    }
    let output_mode = args
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("files_with_matches");
    match output_mode {
        "content" => {
            if let Some(b) = args.get("-B").and_then(Value::as_i64) {
                cmd.push("-B".into());
                cmd.push(b.to_string());
            }
            if let Some(a) = args.get("-A").and_then(Value::as_i64) {
                cmd.push("-A".into());
                cmd.push(a.to_string());
            }
            let c = args.get("-C").and_then(Value::as_i64).unwrap_or(3);
            cmd.push("-C".into());
            cmd.push(c.to_string());
            if args.get("-n").and_then(Value::as_bool).unwrap_or(true) {
                cmd.push("-n".into());
            }
        }
        "count" => cmd.push("--count-matches".into()),
        _ => cmd.push("--files-with-matches".into()),
    }
    cmd.push("--heading".into());
    cmd.push("--color".into());
    cmd.push("never".into());
    let out = run_capture(&rg, &cmd, root);
    if out.trim().is_empty() {
        return "No matches found".into();
    }
    let mut limit = RG_MAX_LINES;
    if let Some(h) = args.get("head_limit").and_then(Value::as_i64) {
        if h > 0 && (h as usize) < limit {
            limit = h as usize;
        }
    }
    let lines: Vec<&str> = out.lines().collect();
    if lines.len() > limit {
        let mut s = lines[..limit].join("\n");
        s.push_str(&format!("\nResults truncated to first {limit} lines"));
        s
    } else {
        out
    }
}

// --- Grove (port of tool/grove.py — shell to the grove binary) -------------

fn grove_tool(args: &Value, root: &Path) -> String {
    let cmd = args
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if cmd.is_empty() {
        return "<system-reminder>Grove: `command` is required.</system-reminder>".into();
    }
    let mut parts: Vec<String> = match shell_words_split(cmd) {
        Ok(p) => p,
        Err(e) => return format!("<system-reminder>Grove: could not parse command ({e}).</system-reminder>"),
    };
    if parts.first().map(String::as_str) == Some("grove") {
        parts.remove(0);
    }
    let verb = parts.first().cloned().unwrap_or_default();
    if !ALLOWED_VERBS.contains(&verb.as_str()) {
        return format!(
            "<system-reminder>Grove: verb must be one of {ALLOWED_VERBS:?}. Got: {}.</system-reminder>",
            if verb.is_empty() { "(none)" } else { &verb }
        );
    }
    // Sandbox: keep path args inside the workspace (symbol ids with '/../' aside).
    for a in &parts[1..] {
        if a.starts_with('-') {
            continue;
        }
        if a.starts_with('/') || a == ".." || a.starts_with("../") || a.contains("/../") {
            return format!(
                "<system-reminder>Grove: path `{a}` must be inside the workspace (relative, no '..').</system-reminder>"
            );
        }
    }
    // In-process dispatch: call `ops` + `core::render` directly instead of
    // spawning the grove binary (ADR 0003) — keeps the process-wide grammar
    // cache warm and avoids a reparse per tool call. The verb is already
    // validated against ALLOWED_VERBS and paths are sandboxed above.
    let out = match dispatch_grove(&verb, &parts[1..], root) {
        Ok(s) => s,
        Err(e) => return format!("<system-reminder>Grove: {e}</system-reminder>"),
    };
    if out.trim().is_empty() {
        return "No results.".into();
    }
    let lines: Vec<&str> = out.lines().collect();
    if lines.len() > GROVE_MAX_LINES {
        format!(
            "{}\n...(truncated {} more lines)",
            lines[..GROVE_MAX_LINES].join("\n"),
            lines.len() - GROVE_MAX_LINES
        )
    } else {
        out
    }
}

/// Run one read-only structural verb in-process and return its rendered text
/// (identical to the CLI's stdout — see [`crate::render`]). `args` is the token
/// list after the verb; only the six verbs' documented flags are parsed (`core`
/// stays clap-free), and unknown flags are tolerated (skipped).
fn dispatch_grove(verb: &str, args: &[String], root: &Path) -> anyhow::Result<String> {
    use crate::{ops, render};

    let mut pos: Vec<&str> = Vec::new();
    let (mut kind, mut name, mut dir, mut at): (Option<&str>, Option<&str>, Option<&str>, Option<&str>) =
        (None, None, None, None);
    let (mut name_contains, mut refs) = (false, false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--kind" => { kind = args.get(i + 1).map(String::as_str); i += 2; }
            "--name" => { name = args.get(i + 1).map(String::as_str); i += 2; }
            "--name-contains" | "--name-substr" => { name_contains = true; i += 1; }
            "--refs" => { refs = true; i += 1; }
            "-d" | "--dir" => { dir = args.get(i + 1).map(String::as_str); i += 2; }
            "--at" => { at = args.get(i + 1).map(String::as_str); i += 2; }
            "--detail" => { i += 2; } // text output ignores --detail
            a if a.starts_with('-') => { i += 1; } // tolerate unknown flags
            a => { pos.push(a); i += 1; }
        }
    }

    // `dir`-defaulting verbs mirror the CLI's `default_value = "."`.
    let dir_or_cwd = || dir.unwrap_or(".");
    Ok(match verb {
        "outline" => {
            let file = pos.first().ok_or_else(|| anyhow::anyhow!("outline needs a file"))?;
            render::outline(&ops::outline(&root.join(file), kind)?)
        }
        "symbols" => {
            let d = pos.first().copied().unwrap_or(".");
            render::symbols(&ops::symbols(&root.join(d), kind, name, refs, name_contains)?)
        }
        "source" => {
            let id_or_file = pos.first().ok_or_else(|| anyhow::anyhow!("source needs an id or file"))?;
            render::source(&ops::source(id_or_file, pos.get(1).copied())?)
        }
        "callers" => {
            let nm = pos.first().ok_or_else(|| anyhow::anyhow!("callers needs a name"))?;
            render::callers(&ops::callers(&root.join(dir_or_cwd()), nm)?)
        }
        "definition" => {
            let defs = match at {
                Some(p) => {
                    let (file, row, col) = ops::parse_pos(p)?;
                    ops::definition_at(&root.join(file), row, col, &root.join(dir_or_cwd()))?.1
                }
                None => {
                    let nm = pos.first().ok_or_else(|| anyhow::anyhow!("definition needs a name or --at"))?;
                    ops::definition(&root.join(dir_or_cwd()), nm)?
                }
            };
            render::definition(&defs)
        }
        "map" => {
            let d = pos.first().copied().unwrap_or(".");
            render::map(&ops::map(&root.join(d), kind, name, name_contains)?)
        }
        other => anyhow::bail!("unknown verb `{other}`"), // unreachable: ALLOWED_VERBS gates this
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rg_path() -> Option<String> {
    which("rg")
}

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
        Err(e) => format!("<system-reminder>failed to run `{bin}`: {e}</system-reminder>"),
    }
}

/// Split a command line into words (port of `shlex.split`, POSIX quoting).
fn shell_words_split(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut has_token = false;
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                has_token = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                has_token = true;
            }
            '\\' if in_double => {
                if let Some(&n) = chars.peek() {
                    cur.push(n);
                    chars.next();
                }
            }
            '\\' if !in_single && !in_double => {
                if let Some(&n) = chars.peek() {
                    cur.push(n);
                    chars.next();
                }
                has_token = true;
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if has_token {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if in_single || in_double {
        return Err("unbalanced quotes".into());
    }
    if has_token {
        out.push(cur);
    }
    Ok(out)
}

/// Resolve a Read path inside the workspace, or `None` if it truly escapes.
/// Port of `resolve_read_path` (strict; remap only a duplicated workspace base).
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

/// Resolve a Glob/Grep path, never escaping and never hard-failing (degrades to
/// the workspace root). Port of `resolve_search_path`.
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
    if let Some(base) = root.file_name() {
        let comps: Vec<_> = p
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_owned()),
                _ => None,
            })
            .collect();
        if comps.first().map(std::ffi::OsString::as_os_str) == Some(base) {
            let remapped = normalize(&comps[1..].iter().fold(root.clone(), |a, s| a.join(s)));
            if remapped.starts_with(&root) && remapped.exists() {
                return remapped;
            }
        }
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
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recon_closes_grove_after_budget() {
        assert_eq!(recon_toolset(true).len(), 2); // Grove + submit_plan
        assert_eq!(recon_toolset(false).len(), 1); // submit_plan only
        assert_eq!(recon_toolset(false)[0].function.name, SUBMIT_PLAN);
    }

    #[test]
    fn execute_toolset_is_the_four_tools() {
        let names: Vec<_> = execute_toolset()
            .iter()
            .map(|t| t.function.name.clone())
            .collect();
        assert_eq!(names, vec![READ, GLOB, GREP, GROVE]);
    }

    #[test]
    fn grove_verb_extracts_leading_word() {
        assert_eq!(grove_verb(&json!({"command": "symbols . --name x"})), "symbols");
        assert_eq!(grove_verb(&json!({"command": ""})), "");
    }

    #[test]
    fn grove_rejects_disallowed_verb() {
        let out = grove_tool(&json!({"command": "serve"}), Path::new("."));
        assert!(out.contains("verb must be one of"), "{out}");
    }

    #[test]
    fn grove_rejects_absolute_path_arg() {
        let out = grove_tool(&json!({"command": "outline /etc/passwd"}), Path::new("."));
        assert!(out.contains("must be inside the workspace"), "{out}");
    }

    #[test]
    fn dispatch_grove_parses_flags_and_positionals() {
        let empty: Vec<String> = vec![];
        let r = Path::new(".");
        // Missing positionals surface a clear per-verb error (no grammar needed).
        assert!(dispatch_grove("outline", &empty, r).unwrap_err().to_string().contains("needs a file"));
        assert!(dispatch_grove("source", &empty, r).unwrap_err().to_string().contains("id or file"));
        assert!(dispatch_grove("callers", &empty, r).unwrap_err().to_string().contains("needs a name"));
        assert!(dispatch_grove("definition", &empty, r).unwrap_err().to_string().contains("name or --at"));
        // A flag and its value are consumed as a flag — never mistaken for the
        // file positional (the parser separates them correctly).
        let flagged = vec!["--kind".to_string(), "function".to_string()];
        let e = dispatch_grove("outline", &flagged, r).unwrap_err().to_string();
        assert!(e.contains("needs a file"), "flag value must not become a positional: {e}");
    }

    #[test]
    fn shell_split_handles_quotes() {
        assert_eq!(
            shell_words_split("symbols . --name \"foo bar\"").unwrap(),
            vec!["symbols", ".", "--name", "foo bar"]
        );
        assert!(shell_words_split("a \"unbalanced").is_err());
    }

    #[test]
    fn read_rejects_escaping_path() {
        let out = read_tool(&json!({"path": "/etc/passwd"}), Path::new("."));
        assert!(out.contains("Permission error") || out.contains("does not exist"), "{out}");
    }

    // --- Tool-body coverage against a real fixture tree ---------------------
    //
    // The guards above check input validation and sandboxing; these run the
    // Read/Glob/Grep bodies end-to-end so the actual file I/O and rg invocation
    // are exercised, not just the rejection paths (review 2026-07-03).

    /// Create a throwaway fixture directory seeded with `files` (relpath → body).
    /// The label + pid keep concurrent tests from colliding.
    fn fixture(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("grove_toolset_{label}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (rel, body) in files {
            let path = dir.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, body).unwrap();
        }
        dir
    }

    #[test]
    fn read_returns_numbered_lines_and_range_header() {
        let dir = fixture("read_body", &[("src.rs", "alpha\nbeta\ngamma\n")]);
        let out = read_tool(&json!({"path": "src.rs"}), &dir);
        assert!(out.contains(":1-3"), "range header missing: {out}");
        assert!(out.contains("1|alpha"), "{out}");
        assert!(out.contains("3|gamma"), "{out}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_honors_offset_and_limit() {
        let dir = fixture("read_window", &[("src.rs", "alpha\nbeta\ngamma\n")]);
        let out = read_tool(&json!({"path": "src.rs", "offset": 2, "limit": 1}), &dir);
        assert!(out.contains(":2-2"), "{out}");
        assert!(out.contains("2|beta"), "{out}");
        assert!(!out.contains("alpha"), "window should exclude line 1: {out}");
        assert!(!out.contains("gamma"), "window should exclude line 3: {out}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_reports_missing_and_empty_files() {
        let dir = fixture("read_edges", &[("empty.rs", "")]);
        let missing = read_tool(&json!({"path": "nope.rs"}), &dir);
        assert!(missing.contains("does not exist"), "{missing}");
        let empty = read_tool(&json!({"path": "empty.rs"}), &dir);
        assert!(empty.contains("File is empty"), "{empty}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn glob_lists_matching_files_only() {
        let dir = fixture("glob_body", &[("keep.rs", "fn a() {}\n"), ("skip.txt", "x\n")]);
        let out = glob_tool(&json!({"pattern": "*.rs"}), &dir);
        if which("rg").is_none() {
            assert!(out.contains("requires ripgrep"), "{out}");
        } else {
            assert!(out.contains("keep.rs"), "{out}");
            assert!(!out.contains("skip.txt"), "{out}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn grep_finds_content_and_reports_no_matches() {
        let dir = fixture("grep_body", &[("src.rs", "let needle = 1;\nlet hay = 2;\n")]);
        let hit = grep_tool(
            &json!({"pattern": "needle", "output_mode": "content"}),
            &dir,
        );
        if which("rg").is_none() {
            assert!(hit.contains("requires ripgrep"), "{hit}");
        } else {
            assert!(hit.contains("needle"), "{hit}");
            let miss = grep_tool(&json!({"pattern": "zzz_absent_zzz"}), &dir);
            assert!(miss.contains("No matches found"), "{miss}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dispatch_routes_read_to_the_body() {
        let dir = fixture("dispatch_body", &[("src.rs", "one\ntwo\n")]);
        let out = dispatch(READ, &json!({"path": "src.rs"}), &dir);
        assert!(out.contains("1|one"), "dispatch should reach read_tool: {out}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
