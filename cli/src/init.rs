//! `grove init` — make a project one where the agent actually uses grove.
//!
//! Two halves, per VISION §6.4.1: registration (`.mcp.json`) makes the tools
//! *available*; a steering directive (`CLAUDE.md`) makes them *adopted*. Plus a
//! `grove.lock` pinning the grammars the project needs. Idempotent: re-running
//! updates the grove pieces without clobbering anything else.

use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde_json::{json, Value};

use grove_core::config::{GroveConfig, Mode};
use grove_core::harness::{GROVE_START as CLAUDE_START, GROVE_END as CLAUDE_END, MCP_SERVER_KEY};
use grove_core::init::provision_project;
use grove_core::registry;
use grove_core::ExploreConfig;

/// Which integration `grove init` wires up. Grammar provisioning (fetch +
/// `grove.lock`) happens for every target; this only selects the harness glue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// Register the MCP server (`.mcp.json`) + a CLAUDE.md steering block.
    #[default]
    Mcp,
    /// Grammars + a CLAUDE.md steering block (so a cold agent actually reaches
    /// for grove instead of grep). The skill *artifact* is distributed via the
    /// skills tool (`npx skills add Entelligentsia/grove`); init provisions and
    /// steers — steering to the grove skill / CLI rather than MCP tools.
    Skill,
    /// Both of the above.
    Both,
    /// Register grove in explore-mode (`.mcp.json` with `serve --explore`) + sentinel
    /// steering blocks in `CLAUDE.md` and `AGENTS.md` directing the agent through
    /// `mcp__grove__explore`. Runs the config TUI on first init (when
    /// `.grove/explore.json` is absent); skips TUI on re-runs and `--dry-run`.
    McpLlm,
    /// Grammars + `grove.lock` only — no `.mcp.json`, no CLAUDE.md steering.
    /// For embedding hosts (e.g. an editor or agent runtime) that register
    /// grove's tools themselves and supply their own steering; the project's
    /// own files are left untouched.
    Grammars,
}

impl Target {
    fn writes_mcp(self) -> bool {
        matches!(self, Target::Mcp | Target::Both)
    }
    fn is_skill(self) -> bool {
        matches!(self, Target::Skill | Target::Both)
    }
    /// Whether this target writes a CLAUDE.md steering block. Every target but
    /// `Grammars` does — `Grammars` provisions grammars + the lock and nothing
    /// else, leaving project-owned files (CLAUDE.md, `.mcp.json`) untouched.
    fn writes_steering(self) -> bool {
        !matches!(self, Target::Grammars)
    }

    /// Bridge from the CLI `Target` enum (clap-facing) to the store `Mode`
    /// enum (config-facing). 1-to-1 mapping.
    pub fn to_mode(self) -> Mode {
        match self {
            Target::Mcp => Mode::Mcp,
            Target::Skill => Mode::Skill,
            Target::Both => Mode::Both,
            Target::McpLlm => Mode::McpLlm,
            Target::Grammars => Mode::Grammars,
        }
    }
}


pub fn run(root: &Path, target: Target, dry_run: bool) -> Result<()> {
    println!("grove init  scanning {} (as {:?})\n", root.display(), target);

    // Load prior config (old mode) — used for reconcile_harness and explore-config preservation.
    let old_cfg = GroveConfig::load(root).ok();
    let old_mode: Option<Mode> = old_cfg.as_ref().map(|c| c.mode);

    // Non-TTY guard for McpLlm first-run: the TUI requires an interactive terminal
    // to collect the explore backend configuration. Re-runs (when config.json
    // already exists with mcp-llm mode or explore.json exists) and --dry-run
    // bypass this guard so CI re-runs are never blocked after the first
    // interactive session.
    let already_configured = old_mode == Some(Mode::McpLlm)
        || root.join(".grove").join("explore.json").exists();
    if target == Target::McpLlm
        && !dry_run
        && !already_configured
        && !std::io::stdout().is_terminal()
    {
        anyhow::bail!(
            "`grove init --as mcp-llm` requires an interactive terminal for the \
             first-run configuration. Run it in a real terminal, or pre-create \
             `.grove/explore.json` to skip the TUI on subsequent runs."
        );
    }

    // Provision grammars + the lock (clap-free core). An empty return is the
    // contract for "nothing provisioned" — a dry-run / no-files / no-cached-
    // grammars short-circuit; core already printed its terminal line, so stop.
    let provisioned = provision_project(root, dry_run)?;

    // For McpLlm dry-run: print the harness files that would be written BEFORE
    // the is_empty() early-return that provision_project triggers on --dry-run.
    if target == Target::McpLlm && dry_run {
        println!("  would write  .mcp.json (explore-mode registration)");
        println!("  would write  CLAUDE.md (explore-mode steering)");
        println!("  would write  AGENTS.md (explore-mode steering)");
    }

    if provisioned.is_empty() {
        return Ok(());
    }

    // First-run TUI: launch the config TUI to create .grove/explore.json when
    // it doesn't exist yet. Skipped on re-runs (file already there).
    if target == Target::McpLlm && !root.join(".grove").join("explore.json").exists() {
        crate::config_tui::run(root, None)?;
    }

    // Reconcile the harness glue to match new_mode, then extend with provisioning
    // actions — preserving report order: `.mcp.json`, `CLAUDE.md`, `grove.lock`.
    let new_mode = target.to_mode();
    let mut wrote = reconcile_harness(root, old_mode, new_mode)?;
    wrote.extend(provisioned);

    // Save the new config.json so active_mode reflects the chosen target.
    let explore = if target == Target::McpLlm {
        // Prefer config.json (TUI writes here); fall back to legacy explore.json,
        // then to the explore section of the pre-run config.
        GroveConfig::load(root).ok().and_then(|c| c.explore)
            .or_else(|| ExploreConfig::load(root).ok())
            .or_else(|| old_cfg.as_ref().and_then(|c| c.explore.clone()))
    } else {
        // Preserve existing explore config across mode switches.
        old_cfg.as_ref().and_then(|c| c.explore.clone())
    };
    let new_cfg = GroveConfig { version: 1, mode: new_mode, explore };
    new_cfg.save(root)?;

    println!("\n  wrote");
    for w in &wrote {
        println!("             ✓ {w}");
    }
    if target == Target::McpLlm {
        println!("\n  ready      `mcp__grove__explore` is your single code-exploration surface.");
        println!("             While the local model is healthy, all code questions route there.");
        println!("             If the provider goes down, grove auto-surfaces the 7 structural");
        println!("             tools (outline · symbols · source · callers · map · definition · check).");
    } else if target.writes_mcp() {
        println!("\n  ready      your agent now has grove's tools across its loop:");
        println!("             outline · symbols · source · callers · map · definition · check");
        println!("\n  try it     start a fresh Claude Code session here and ask a");
        println!("             \"where/what/who-calls\" question — it routes through grove.");
    }
    if target.is_skill() {
        println!("\n  skill      grammars + steering are ready. Install the cross-harness");
        println!("             skill with:  npx skills add Entelligentsia/grove");
        println!("             (it steers your agent to grove's MCP tools when present,");
        println!("              else the grove CLI — same engine either way.)");
    }
    if !target.writes_steering() {
        println!("\n  grammars   provisioned + pinned (grove.lock). No project files were");
        println!("             modified — the embedding host registers grove's tools and");
        println!("             supplies its own steering.");
    }
    Ok(())
}

/// Reconcile all three harness artifacts (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`)
/// toward `new_mode`. The `old_mode` parameter is accepted for forward-compatibility
/// (future skip-if-already-correct optimisations) but the reconciliation always
/// drives from on-disk state to `new_mode`. Stripped artifacts are silently
/// reconciled (no entry in the returned list); only written artifacts are listed.
fn reconcile_harness(
    root: &Path,
    _old_mode: Option<Mode>,
    new_mode: Mode,
) -> Result<Vec<String>> {
    let mut wrote = Vec::new();

    // Load langs for steering content (from grove.lock written by provision_project).
    let langs = registry::locked_langs(&root.join("grove.lock"))?;

    // ── .mcp.json ──────────────────────────────────────────────────────────────
    match new_mode {
        Mode::Mcp | Mode::Both => {
            wrote.push(write_mcp_json(root)?);
        }
        Mode::McpLlm => {
            wrote.push(write_mcp_json_explore(root)?);
        }
        Mode::Skill | Mode::Grammars => {
            strip_grove_entry_from_mcp_json(root)?;
        }
    }

    // ── CLAUDE.md ──────────────────────────────────────────────────────────────
    match new_mode {
        Mode::Grammars => {
            strip_steering_block(root, "CLAUDE.md")?;
        }
        _ => {
            // Map Mode back to a Target for write_claude_md content selection.
            let target = mode_to_target(new_mode);
            wrote.push(write_claude_md(root, &langs, target)?);
        }
    }

    // ── AGENTS.md ──────────────────────────────────────────────────────────────
    match new_mode {
        Mode::McpLlm => {
            wrote.push(write_agents_md(root, &langs, Target::McpLlm)?);
        }
        _ => {
            strip_steering_block(root, "AGENTS.md")?;
        }
    }

    Ok(wrote)
}

/// Map a `Mode` back to the corresponding `Target` variant for content generation.
/// This is the inverse of `Target::to_mode`.
fn mode_to_target(mode: Mode) -> Target {
    match mode {
        Mode::Mcp => Target::Mcp,
        Mode::Skill => Target::Skill,
        Mode::Both => Target::Both,
        Mode::McpLlm => Target::McpLlm,
        Mode::Grammars => Target::Grammars,
    }
}

/// Remove the `<!-- grove:start -->…<!-- grove:end -->` sentinel block from
/// `filename` (under `root`), preserving all host-authored content outside the
/// block. The blank-line separator that `write_steering_md` inserts before the
/// block is also removed, so repeated A→B→A cycles never accrete blank lines.
/// If the file is absent or has no grove block, returns `Ok(())` (no-op).
/// If the block is the entire file, the file is written empty (not deleted).
/// Ensures a single trailing newline when non-empty content remains.
fn strip_steering_block(root: &Path, filename: &str) -> Result<()> {
    let path = root.join(filename);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };

    // No grove block present — no-op.
    if !text.contains(CLAUDE_START) || !text.contains(CLAUDE_END) {
        return Ok(());
    }

    let start = text.find(CLAUDE_START).unwrap();
    let end = text.find(CLAUDE_END).unwrap() + CLAUDE_END.len();

    // Compute the content before the block, trimming the \n\n separator that
    // write_steering_md inserts when appending below existing content.
    let before = text[..start].trim_end_matches('\n');
    let after = &text[end..];

    let result = if before.is_empty() && after.trim().is_empty() {
        // Block was the entire file — leave it empty.
        String::new()
    } else if before.is_empty() {
        // Block was at the top; preserve everything after.
        let s = after.trim_start_matches('\n');
        if s.is_empty() {
            String::new()
        } else {
            format!("{s}\n")
        }
    } else {
        // Host content precedes the block; re-join with single trailing newline.
        let after_trimmed = after.trim();
        if after_trimmed.is_empty() {
            format!("{before}\n")
        } else {
            format!("{before}\n\n{after_trimmed}\n")
        }
    };

    std::fs::write(&path, result).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Remove the `"grove"` key from `.mcp.json`'s `mcpServers` object, preserving
/// all other servers. If the file is absent, has no `mcpServers` object, or has
/// no `"grove"` entry, returns `Ok(())` (no-op). Errors on malformed JSON.
fn strip_grove_entry_from_mcp_json(root: &Path) -> Result<()> {
    let path = root.join(".mcp.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(()), // file absent — no-op
    };

    let mut doc: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;

    // Check if grove entry exists before modifying.
    if doc.get("mcpServers").and_then(|s| s.get(MCP_SERVER_KEY)).is_none() {
        return Ok(()); // no grove entry — no-op
    }

    if let Some(servers) = doc.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove(MCP_SERVER_KEY);
    }

    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Add (or refresh) the grove MCP server in `.mcp.json`, preserving other servers.
fn write_mcp_json(root: &Path) -> Result<String> {
    write_mcp_json_with(root, &["serve"], ".mcp.json (registration — the tools exist)")
}

/// Register the grove MCP server in `.mcp.json` with the given `serve` args,
/// preserving any other servers already present. Shared by the standard and
/// explore-mode entry points, which differ only in the args and status message.
fn write_mcp_json_with(root: &Path, args: &[&str], message: &str) -> Result<String> {
    let path = root.join(".mcp.json");
    let mut doc: Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?,
        Err(_) => json!({}),
    };
    if !doc.is_object() {
        doc = json!({});
    }
    let exe = std::env::current_exe().context("locating the grove binary")?;
    doc["mcpServers"][MCP_SERVER_KEY] = json!({
        "command": exe.to_string_lossy(),
        "args": args,
    });
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(message.to_string())
}

/// Write/refresh the grove steering section in `CLAUDE.md`, between markers, so
/// re-running is idempotent and never disturbs the rest of the file.
fn write_claude_md(root: &Path, langs: &[String], target: Target) -> Result<String> {
    write_steering_md(
        root,
        "CLAUDE.md",
        &claude_section(langs, target),
        "CLAUDE.md (steering — the tools get used)",
    )
}

/// Splice `section` (bracketed by `CLAUDE_START`/`CLAUDE_END`) into `filename`
/// between those sentinels — replacing an existing block, appending below other
/// content, or creating the file. Idempotent, and never disturbs the rest of the
/// file. Shared by `write_claude_md` and `write_agents_md`, which differ only in
/// the target file, the section body, and the status message.
fn write_steering_md(
    root: &Path,
    filename: &str,
    section: &str,
    message: &str,
) -> Result<String> {
    let path = root.join(filename);
    let existing = std::fs::read_to_string(&path).ok();
    let updated = match existing {
        Some(text) if text.contains(CLAUDE_START) && text.contains(CLAUDE_END) => {
            let start = text.find(CLAUDE_START).unwrap();
            let end = text.find(CLAUDE_END).unwrap() + CLAUDE_END.len();
            format!("{}{}{}", &text[..start], section, &text[end..])
        }
        Some(text) => format!("{}\n\n{}\n", text.trim_end(), section),
        None => format!("{section}\n"),
    };
    std::fs::write(&path, updated).with_context(|| format!("writing {}", path.display()))?;
    Ok(message.to_string())
}

/// Add (or refresh) the grove MCP server in `.mcp.json` in explore-mode,
/// registering `["serve", "--explore"]`. Preserves other servers.
fn write_mcp_json_explore(root: &Path) -> Result<String> {
    write_mcp_json_with(
        root,
        &["serve", "--explore"],
        ".mcp.json (explore-mode registration)",
    )
}

/// Write/refresh the grove steering section in `AGENTS.md` via `write_steering_md`;
/// uses harness-neutral framing (AGENTS.md is read by non-Claude harnesses too).
fn write_agents_md(root: &Path, langs: &[String], target: Target) -> Result<String> {
    write_steering_md(
        root,
        "AGENTS.md",
        &agents_section(langs, target),
        "AGENTS.md (explore-mode steering)",
    )
}

/// The AGENTS.md steering block for McpLlm — harness-neutral framing so it is
/// useful in non-Claude harnesses (e.g. Codex, Cline, Cursor) that also read
/// AGENTS.md. Uses the same sentinel markers for idempotent updates.
fn agents_section(langs: &[String], _target: Target) -> String {
    let langs_str = langs.join(", ");
    format!(
        "{CLAUDE_START}
## Code navigation: grove explore-mode

**grove** is running in explore-mode (languages: {langs_str}). The `explore` tool
exposed by the grove MCP server is a code **locator** backed by a local LLM and
tree-sitter tools: it finds WHERE code lives and returns file:line citations.

Ask it ONE narrow, single-focus question per call (where-is / what-defines /
who-calls) instead of built-in file search — but keep each question targeted.
For a broad task (\"how does feature F work\"), make a few focused `explore`
calls to locate the pieces, then read the cited spans and synthesize the answer
yourself; the local model is light and overshoots on one large compound question.

**Recommended flow to understand a feature:** (1) a few narrow `explore` calls to
locate the pieces — iterate broad → symbol-specific, each refining from the last;
(2) read the cited spans — yourself, or via one focused subagent whose prompt
cites those exact file:line locations; (3) synthesize. Do not spawn a grep-based
search subagent before locating with grove — that wastes context on what grove
does locally for free.

**Automatic fallback**: if the grove provider is unreachable, the 7 structural
grove tools (outline, symbols, source, callers, map, definition, check) are
surfaced automatically — use them as you normally would.

For text, configs, and non-code files, use the built-in shell or file-read tools.
{CLAUDE_END}"
    )
}

/// The steering block, tailored to how the agent reaches grove. The MCP target
/// carries the full route-by-task guide inline (there is no skill file to defer
/// to); the skill target is a thin version that defers to SKILL.md as the single
/// source of truth. Both route by task — grove for named symbols / structure, the
/// shell for text / non-code / quick facts, combining freely — rather than banning
/// grep: a cold agent with no block at all defaults to grep / whole-file reads
/// (VISION §6.4.1), and an over-rigid "grove or violation" block overcorrects into
/// costly source fan-outs for things the shell does cheaply.
fn claude_section(langs: &[String], target: Target) -> String {
    let langs = langs.join(", ");
    if target == Target::McpLlm {
        let p = format!("mcp__{MCP_SERVER_KEY}__");
        return format!(
            "{CLAUDE_START}
## Code navigation: grove explore-mode

**grove** is running in explore-mode (languages: {langs}). `{p}explore` is a code
**locator** backed by a local LLM with tree-sitter + text tools — it finds WHERE
relevant code lives and returns file:line citations. It is not a full-analysis
oracle.

**Ask it ONE narrow, single-focus question per call** (\"where is X defined\",
\"which files handle Y\", \"who calls Z\") — not a broad multi-part task. For a
question like \"how does feature F work\", don't delegate the whole thing: make a
few targeted `{p}explore` calls to locate the pieces, then read the cited spans
and **synthesize the explanation yourself**. The local model is light and
overshoots on compound \"find everything and explain how it all works\" prompts.
The explore tool handles tool-selection internally; do **not** call the
individual structural tools (outline, symbols, source, callers, map, definition,
check) directly while the local model is healthy — they are not exposed in
explore-mode.

**Recommended flow to understand a feature:** (1) make a few narrow
`{p}explore` calls to locate the pieces — iterate broad → symbol-specific, each
call refining from the last's results; (2) read the cited spans — yourself, or
via **one** focused subagent whose prompt cites those exact file:line locations;
(3) synthesize. **Do not spawn a grep-based search subagent before locating with
grove** — that spends your metered context on what grove does locally for free.

**Automatic fallback**: if the local model provider is unreachable, grove
automatically surfaces the 7 structural tools. They will appear in your tool
list and work identically to the standard MCP surface.

For text, configs, and non-code files, use the shell (`grep`, `rg`, `read`).
{CLAUDE_END}"
        );
    }
    if target.writes_mcp() {
        // Claude Code exposes the tools as `mcp__<server-key>__<tool>`; steer with
        // those exact names. A future AGENTS.md/.cursorrules adapter passes its own.
        let p = format!("mcp__{MCP_SERVER_KEY}__");
        format!(
            "{CLAUDE_START}
## Code navigation: grove for structure, shell for the rest

**grove** is a tree-sitter engine for *structural* code questions — byte-precise,
token-cheap (languages: {langs}). Its tools are **deferred** MCP tools; load them in
one ToolSearch when a code question lands (don't default to a search agent or grep):
`{p}outline`, `{p}symbols`, `{p}source`, `{p}callers`, `{p}definition`, `{p}map`, `{p}check`.

**Use grove for named symbols and relationships** (every result carries a stable
`symbol-id`, `<lang>:<relpath>#<name>@<row>`, to pass forward; lines 1-based):
- What's in a file (skeleton, not the whole file) → `{p}outline` (`detail:0` if > 500 lines).
- Where a fn / type / struct / macro is defined → `{p}symbols` with `name` → `{p}source` with the id.
- One symbol's exact body → `{p}source`.
- Who calls it → `{p}callers`.
- Go-to-def from a usage (scope-aware, follows imports cross-file) → `{p}definition` with `at` (file:line:col).
- How a directory connects → `{p}map` (one call; prefer over many `{p}source`).
- Syntax after an edit → `{p}check`.

**Use the shell — the right tool, not a fallback — when grove can't see the target:**
- Text, not a symbol (a string, log / error message, config key, a macro's *value*,
  a constant, a flag, a TODO) → `grep -rn` / `rg`. grove finds definitions, not text.
- Non-code files (Makefiles, configs, data, docs) → `grep` / `read`.
- A quick fact (path exists, `ls`, `wc -l`, `find`, read a small file) → shell.

**Combine** (same 1-based lines, same bytes): `grep` a literal's line → `{p}definition`
`at` to resolve its symbol · `{p}outline` → bounded `read` (`offset`/`limit`) for
adjacent symbols · `{p}map` / `{p}symbols` to locate → `grep` a constant inside.

Rule of thumb: want a **symbol** → grove first (don't `grep` / `read` for it). Want
**text or a quick fact** → shell. Combining is fine.
{CLAUDE_END}"
        )
    } else {
        format!(
            "{CLAUDE_START}
## Code navigation: grove for structure, shell for the rest

**grove** (tree-sitter backed; languages: {langs}) answers *structural* code
questions — where a symbol is defined, who calls it, what's in a file, how a
directory connects, syntax-check after edits — byte-precise and token-cheap. For a
**named symbol or a structural relationship**, reach for grove (the **grove skill**,
or the `grove` CLI — same engine) before `grep` or a whole-file `read`.

Use the shell where grove can't see the target — it's the right tool, not a fallback:
**text** (a string, log / error message, config key, constant, flag, TODO) → `grep` / `rg`;
**non-code files** (Makefiles, configs, data, docs) → `grep` / `read`; a **quick fact**
(path exists, `ls`, `wc -l`, `find`) → shell. Combining is fine — grove to navigate,
the shell to pin a literal or read a small region.

**The procedure, recovery rule, and full command surface live in the grove skill —
read it before answering a structural question.** It is the single source of truth;
don't rely on remembered flags.
{CLAUDE_END}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grove_core::config::{GroveConfig, Mode, ModeChoice};

    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("grove_init_test_{}_{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn claude_section_mcp_is_delimited_and_names_langs_and_prefix() {
        let s = claude_section(&["rust".into(), "python".into()], Target::Mcp);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("rust, python"));
        assert!(s.contains("mcp__grove__outline"));
        assert!(s.contains("mcp__grove__callers"));
    }

    #[test]
    fn claude_section_skill_is_thin_and_defers_to_skill() {
        let s = claude_section(&["rust".into()], Target::Skill);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("grove skill"), "skill steering defers to the skill");
        assert!(s.contains("single source of truth"), "defers to SKILL.md, not remembered flags");
        assert!(s.contains("grep"), "routes text search to the shell");
        assert!(!s.contains("mcp__grove__"), "skill steering must not name MCP tools");
    }

    #[test]
    fn claude_section_mcp_routes_grove_and_shell() {
        let s = claude_section(&["rust".into()], Target::Mcp);
        // grove for structure: tools named, with the symbol-first rule of thumb.
        assert!(s.contains("mcp__grove__source"), "names the grove tools");
        assert!(s.contains("symbol"), "routes symbol work to grove");
        // shell is a first-class partner, not a banned fallback.
        assert!(s.contains("grep"), "routes text/quick-fact work to the shell");
        assert!(s.contains("Combine"), "blesses combining grove + shell");
        assert!(!s.contains("steering violation"), "no prohibitive framing");
    }

    #[test]
    fn write_claude_md_creates_then_updates_idempotently() {
        let dir = tmp("claude_idem");
        let path = dir.join("CLAUDE.md");

        write_claude_md(&dir, &["rust".into()], Target::Mcp).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first.matches(CLAUDE_START).count(), 1);
        assert!(first.contains("rust"));

        // Re-run with a different language set: section is replaced in place, not
        // duplicated.
        write_claude_md(&dir, &["python".into()], Target::Mcp).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(second.matches(CLAUDE_START).count(), 1, "exactly one grove section");
        assert_eq!(second.matches(CLAUDE_END).count(), 1);
        assert!(second.contains("python"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_claude_md_appends_below_existing_content() {
        let dir = tmp("claude_append");
        let path = dir.join("CLAUDE.md");
        std::fs::write(&path, "# My project\n\nHand-written notes.\n").unwrap();

        write_claude_md(&dir, &["rust".into()], Target::Mcp).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Hand-written notes."), "existing content preserved");
        assert!(text.contains(CLAUDE_START), "grove section appended");
        assert!(text.find("Hand-written").unwrap() < text.find(CLAUDE_START).unwrap());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_registers_grove_and_preserves_other_servers() {
        let dir = tmp("mcp_preserve");
        let path = dir.join(".mcp.json");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();

        write_mcp_json(&dir).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "existing server kept");
        assert_eq!(doc["mcpServers"]["grove"]["args"], json!(["serve"]), "grove registered");
        assert!(doc["mcpServers"]["grove"]["command"].is_string());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_creates_file_when_absent() {
        let dir = tmp("mcp_fresh");
        write_mcp_json(&dir).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
        assert!(doc["mcpServers"]["grove"].is_object());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_errors_on_invalid_existing_json() {
        let dir = tmp("mcp_bad");
        std::fs::write(dir.join(".mcp.json"), "{not json").unwrap();
        let err = write_mcp_json(&dir).unwrap_err();
        assert!(err.to_string().contains("not valid JSON"), "got: {err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn target_default_is_mcp_and_flags_route_correctly() {
        assert_eq!(Target::default(), Target::Mcp);
        assert!(Target::Mcp.writes_mcp() && !Target::Mcp.is_skill());
        assert!(!Target::Skill.writes_mcp() && Target::Skill.is_skill());
        assert!(Target::Both.writes_mcp() && Target::Both.is_skill());
        // Grammars wires nothing — no mcp, no skill, no steering.
        assert!(!Target::Grammars.writes_mcp() && !Target::Grammars.is_skill());
        assert!(!Target::Grammars.writes_steering());
        assert!(Target::Mcp.writes_steering() && Target::Skill.writes_steering());
    }

    /// `reconcile_harness` is harness-only: grammar provisioning + the
    /// `grove.lock` write moved to `grove_core::init::provision_project`, so
    /// these tests seed a lock first (the source of steering's language list)
    /// and assert only the harness files (`.mcp.json` / `CLAUDE.md` / `AGENTS.md`).
    fn seed_lock(dir: &std::path::Path) {
        std::fs::write(
            dir.join("grove.lock"),
            r#"{"version":1,"grammars":[{"name":"rust","version":"1","wasm":"x"}]}"#,
        )
        .unwrap();
    }

    // ─── strip_steering_block tests ───────────────────────────────────────────

    #[test]
    fn strip_steering_block_removes_block_leaves_host_content() {
        let dir = tmp("strip_leaves_host");
        let path = dir.join("CLAUDE.md");
        // Host content before the block (write_steering_md appends \n\n before the block)
        std::fs::write(
            &path,
            format!("# My project\n\nHand-written notes.\n\n{CLAUDE_START}\ngrove block\n{CLAUDE_END}\n"),
        )
        .unwrap();

        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Hand-written notes."), "host content preserved");
        assert!(!text.contains(CLAUDE_START), "grove block removed");
        assert!(!text.contains(CLAUDE_END), "grove block removed");
        // Should end with a single newline
        assert!(text.ends_with('\n'), "single trailing newline");
        // No double blank lines accreted
        assert!(!text.contains("\n\n\n"), "no triple newlines: {text:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_steering_block_empties_file_when_block_only() {
        let dir = tmp("strip_block_only");
        let path = dir.join("CLAUDE.md");
        std::fs::write(
            &path,
            format!("{CLAUDE_START}\ngrove block\n{CLAUDE_END}\n"),
        )
        .unwrap();

        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.is_empty(), "file written empty when block-only: {text:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_steering_block_noop_when_no_block() {
        let dir = tmp("strip_noop_no_block");
        let path = dir.join("CLAUDE.md");
        let original = "# My project\n\nNo grove block here.\n";
        std::fs::write(&path, original).unwrap();

        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, original, "file unchanged when no grove block");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_steering_block_noop_when_absent() {
        let dir = tmp("strip_noop_absent");
        // File doesn't exist — should not error
        strip_steering_block(&dir, "CLAUDE.md").unwrap();
        assert!(!dir.join("CLAUDE.md").exists(), "file not created");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── strip_grove_entry_from_mcp_json tests ────────────────────────────────

    #[test]
    fn strip_grove_entry_from_mcp_json_removes_grove_key() {
        let dir = tmp("strip_mcp_removes_grove");
        let path = dir.join(".mcp.json");
        std::fs::write(
            &path,
            r#"{"mcpServers":{"grove":{"command":"grove","args":["serve"]},"other":{"command":"x"}}}"#,
        )
        .unwrap();

        strip_grove_entry_from_mcp_json(&dir).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(doc["mcpServers"]["grove"].is_null(), "grove entry removed");
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "other server kept");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_grove_entry_from_mcp_json_noop_when_absent() {
        let dir = tmp("strip_mcp_noop_absent");
        strip_grove_entry_from_mcp_json(&dir).unwrap();
        assert!(!dir.join(".mcp.json").exists(), "file not created");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_grove_entry_from_mcp_json_noop_when_no_grove_key() {
        let dir = tmp("strip_mcp_noop_no_grove");
        let path = dir.join(".mcp.json");
        let original = r#"{"mcpServers":{"other":{"command":"x"}}}"#;
        std::fs::write(&path, original).unwrap();

        strip_grove_entry_from_mcp_json(&dir).unwrap();
        // File should not be modified (same content, but formatting may differ after parse+write).
        // The important thing is that "other" is still there and grove is not.
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "other server intact");
        assert!(doc["mcpServers"]["grove"].is_null(), "grove not added");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── reconcile_harness helper assertions ──────────────────────────────────

    fn assert_mcp_json_consistent(dir: &std::path::Path, mode: Mode) {
        let path = dir.join(".mcp.json");
        match mode {
            Mode::Mcp | Mode::Both => {
                let doc: Value = serde_json::from_str(
                    &std::fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("missing .mcp.json for mode {mode:?}")),
                )
                .unwrap();
                assert_eq!(
                    doc["mcpServers"]["grove"]["args"],
                    json!(["serve"]),
                    "mode {mode:?}: .mcp.json args should be [serve]"
                );
            }
            Mode::McpLlm => {
                let doc: Value = serde_json::from_str(
                    &std::fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("missing .mcp.json for mode {mode:?}")),
                )
                .unwrap();
                assert_eq!(
                    doc["mcpServers"]["grove"]["args"],
                    json!(["serve", "--explore"]),
                    "mode {mode:?}: .mcp.json args should be [serve, --explore]"
                );
            }
            Mode::Skill | Mode::Grammars => {
                // File may not exist, or if it does, grove entry must be absent.
                if path.exists() {
                    let doc: Value = serde_json::from_str(
                        &std::fs::read_to_string(&path).unwrap(),
                    )
                    .unwrap();
                    assert!(
                        doc.get("mcpServers")
                            .and_then(|s| s.get("grove"))
                            .is_none(),
                        "mode {mode:?}: grove entry should be absent in .mcp.json"
                    );
                }
            }
        }
    }

    fn assert_claude_md_consistent(dir: &std::path::Path, mode: Mode) {
        let path = dir.join("CLAUDE.md");
        match mode {
            Mode::Grammars => {
                // File either absent or has no grove block.
                if path.exists() {
                    let text = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !text.contains(CLAUDE_START),
                        "mode Grammars: CLAUDE.md must not contain grove block, got: {text:?}"
                    );
                }
            }
            Mode::McpLlm => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing CLAUDE.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode McpLlm: CLAUDE.md must have grove block"
                );
                assert!(
                    text.contains("mcp__grove__explore"),
                    "mode McpLlm: CLAUDE.md must reference explore tool"
                );
            }
            Mode::Mcp | Mode::Both => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing CLAUDE.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode {mode:?}: CLAUDE.md must have grove block"
                );
                assert!(
                    text.contains("mcp__grove__outline"),
                    "mode {mode:?}: CLAUDE.md must reference outline tool"
                );
            }
            Mode::Skill => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing CLAUDE.md for mode {mode:?}"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode Skill: CLAUDE.md must have grove block"
                );
                assert!(
                    text.contains("grove skill"),
                    "mode Skill: CLAUDE.md must reference grove skill"
                );
            }
        }
    }

    fn assert_agents_md_consistent(dir: &std::path::Path, mode: Mode) {
        let path = dir.join("AGENTS.md");
        match mode {
            Mode::McpLlm => {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("missing AGENTS.md for mode McpLlm"));
                assert!(
                    text.contains(CLAUDE_START),
                    "mode McpLlm: AGENTS.md must have grove block"
                );
            }
            _ => {
                // File absent or grove block absent.
                if path.exists() {
                    let text = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !text.contains(CLAUDE_START),
                        "mode {mode:?}: AGENTS.md must not have grove block, got: {text:?}"
                    );
                }
            }
        }
    }

    // ─── reconcile_harness transition-matrix test (20 ordered A→B pairs) ─────

    #[test]
    fn reconcile_harness_transition_matrix() {
        let modes = [Mode::Mcp, Mode::Skill, Mode::Both, Mode::McpLlm, Mode::Grammars];

        for &old in &modes {
            for &new in &modes {
                if old == new {
                    continue;
                }
                let dir = tmp(&format!("matrix_{old:?}_{new:?}"));
                seed_lock(&dir);

                // Seed the initial harness for mode A
                reconcile_harness(&dir, None, old)
                    .unwrap_or_else(|e| panic!("seed {old:?}: {e}"));

                // Transition to mode B
                reconcile_harness(&dir, Some(old), new)
                    .unwrap_or_else(|e| panic!("transition {old:?}→{new:?}: {e}"));

                // Assert harness is consistent with B
                assert_mcp_json_consistent(&dir, new);
                assert_claude_md_consistent(&dir, new);
                assert_agents_md_consistent(&dir, new);

                std::fs::remove_dir_all(&dir).ok();
            }
        }
    }

    // ─── host-content-preservation test ──────────────────────────────────────

    #[test]
    fn reconcile_harness_preserves_host_content() {
        let dir = tmp("preserves_host");
        seed_lock(&dir);

        // 1. Seed CLAUDE.md with host content + Mcp block.
        write_claude_md(&dir, &["rust".into()], Target::Mcp).unwrap();
        // Prepend host content (simulate a file that had content before grove ran)
        let existing = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        std::fs::write(
            dir.join("CLAUDE.md"),
            format!("# My project\n\nHost notes.\n\n{existing}"),
        )
        .unwrap();

        // Transition Mcp → Grammars: CLAUDE.md block stripped, host content intact.
        reconcile_harness(&dir, Some(Mode::Mcp), Mode::Grammars).unwrap();
        let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("Host notes."), "host content in CLAUDE.md preserved");
        assert!(!claude.contains(CLAUDE_START), "grove block removed from CLAUDE.md");

        // 2. Seed .mcp.json with grove + other server; transition to Skill → grove removed.
        let mcp_path = dir.join(".mcp.json");
        std::fs::write(
            &mcp_path,
            r#"{"mcpServers":{"grove":{"command":"g","args":["serve"]},"other":{"command":"x"}}}"#,
        )
        .unwrap();
        reconcile_harness(&dir, Some(Mode::Grammars), Mode::Skill).unwrap();
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
        assert!(doc["mcpServers"]["grove"].is_null(), "grove removed from .mcp.json");
        assert_eq!(doc["mcpServers"]["other"]["command"], json!("x"), "other server preserved");

        // 3. Seed AGENTS.md with host content + McpLlm block; transition to Mcp → block stripped.
        // First get to McpLlm to seed AGENTS.md
        reconcile_harness(&dir, Some(Mode::Skill), Mode::McpLlm).unwrap();
        let existing_agents = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        std::fs::write(
            dir.join("AGENTS.md"),
            format!("# AGENTS\n\nHost agent notes.\n\n{existing_agents}"),
        )
        .unwrap();
        // Transition McpLlm → Mcp: AGENTS.md block stripped, host content intact.
        reconcile_harness(&dir, Some(Mode::McpLlm), Mode::Mcp).unwrap();
        let agents = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Host agent notes."), "host content in AGENTS.md preserved");
        assert!(!agents.contains(CLAUDE_START), "grove block removed from AGENTS.md");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── config round-trip test ───────────────────────────────────────────────

    #[test]
    fn reconcile_harness_then_save_config_active_mode() {
        let dir = tmp("config_round_trip");
        seed_lock(&dir);

        // Reconcile to Mcp mode, then save config.
        reconcile_harness(&dir, None, Mode::Mcp).unwrap();
        let cfg = GroveConfig { version: 1, mode: Mode::Mcp, explore: None };
        cfg.save(&dir).unwrap();

        // active_mode should report Mcp.
        let loaded = GroveConfig::load(&dir).unwrap();
        assert_eq!(loaded.mode, Mode::Mcp, "config.json round-trips Mcp mode");
        let am = grove_core::config::active_mode(&dir, ModeChoice::None);
        assert_eq!(am, Mode::Mcp, "active_mode returns Mcp");

        // Transition to McpLlm mode, save config.
        reconcile_harness(&dir, Some(Mode::Mcp), Mode::McpLlm).unwrap();
        let cfg2 = GroveConfig { version: 1, mode: Mode::McpLlm, explore: None };
        cfg2.save(&dir).unwrap();

        let loaded2 = GroveConfig::load(&dir).unwrap();
        assert_eq!(loaded2.mode, Mode::McpLlm, "config.json round-trips McpLlm mode");
        let am2 = grove_core::config::active_mode(&dir, ModeChoice::None);
        assert_eq!(am2, Mode::McpLlm, "active_mode returns McpLlm");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── existing McpLlm unit tests ───────────────────────────────────────────

    #[test]
    fn claude_section_mcp_llm_routes_explore_not_individual_tools() {
        let s = claude_section(&["rust".into()], Target::McpLlm);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("mcp__grove__explore"), "names explore tool: {s}");
        assert!(!s.contains("mcp__grove__outline"), "must not name outline directly: {s}");
        assert!(s.contains("fallback"), "mentions fallback behaviour: {s}");
    }

    #[test]
    fn agents_section_mcp_llm_routes_explore_not_individual_tools() {
        let s = agents_section(&["rust".into()], Target::McpLlm);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("explore"), "names explore tool: {s}");
        assert!(!s.contains("mcp__grove__"), "must not use Claude-specific prefix: {s}");
        assert!(s.contains("fallback"), "mentions fallback behaviour: {s}");
    }

    #[test]
    fn reconcile_harness_mcp_llm_writes_mcp_json_explore_and_steering() {
        let dir = tmp("harness_mcp_llm");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::McpLlm).unwrap();

        // .mcp.json must be present and contain --explore
        let mcp_json = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&mcp_json).unwrap();
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!(["serve", "--explore"]),
            "explore-mode args"
        );

        assert!(dir.join("CLAUDE.md").exists(), "CLAUDE.md written");
        assert!(dir.join("AGENTS.md").exists(), "AGENTS.md written");
        assert_eq!(wrote.len(), 3, "3 harness entries: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_agents_md_creates_then_updates_idempotently() {
        let dir = tmp("agents_idem");
        let path = dir.join("AGENTS.md");

        write_agents_md(&dir, &["rust".into()], Target::McpLlm).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first.matches(CLAUDE_START).count(), 1);
        assert!(first.contains("rust"));

        // Re-run: section replaced in place, not duplicated.
        write_agents_md(&dir, &["python".into()], Target::McpLlm).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(second.matches(CLAUDE_START).count(), 1, "exactly one grove section");
        assert_eq!(second.matches(CLAUDE_END).count(), 1);
        assert!(second.contains("python"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_agents_md_appends_below_existing_content() {
        let dir = tmp("agents_append");
        let path = dir.join("AGENTS.md");
        std::fs::write(&path, "# My project\n\nHand-written notes.\n").unwrap();

        write_agents_md(&dir, &["rust".into()], Target::McpLlm).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Hand-written notes."), "existing content preserved");
        assert!(text.contains(CLAUDE_START), "grove section appended");
        assert!(text.find("Hand-written").unwrap() < text.find(CLAUDE_START).unwrap());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_explore_registers_with_explore_flag() {
        let dir = tmp("mcp_explore_fresh");
        write_mcp_json_explore(&dir).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!(["serve", "--explore"]),
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_mcp_json_explore_preserves_other_servers() {
        let dir = tmp("mcp_explore_preserve");
        let path = dir.join(".mcp.json");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();

        write_mcp_json_explore(&dir).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], serde_json::json!("x"), "existing server kept");
        assert_eq!(
            doc["mcpServers"]["grove"]["args"],
            serde_json::json!(["serve", "--explore"]),
            "grove explore registered"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── pre-existing harness tests updated to use reconcile_harness ──────────

    #[test]
    fn grammars_target_writes_no_harness_files() {
        let dir = tmp("harness_grammars");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::Grammars).unwrap();

        assert!(!dir.join("CLAUDE.md").exists() || {
            let t = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
            !t.contains(CLAUDE_START)
        }, "grammars mode writes no steering block");
        assert!(!dir.join(".mcp.json").exists() || {
            let doc: Value = serde_json::from_str(
                &std::fs::read_to_string(dir.join(".mcp.json")).unwrap()
            ).unwrap();
            doc.get("mcpServers").and_then(|s| s.get("grove")).is_none()
        }, "grammars mode writes no .mcp.json grove entry");
        // AGENTS.md must not have a grove block
        assert!(!dir.join("AGENTS.md").exists() || {
            let t = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
            !t.contains(CLAUDE_START)
        }, "grammars mode writes no AGENTS.md block");
        assert!(wrote.is_empty(), "no harness writes reported: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skill_target_writes_steering_but_no_mcp_json() {
        let dir = tmp("harness_skill");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::Skill).unwrap();

        assert!(dir.join("CLAUDE.md").exists(), "skill mode writes steering");
        assert!(!dir.join(".mcp.json").exists(), "skill mode writes no .mcp.json");
        assert_eq!(wrote.len(), 1, "only steering reported: {wrote:?}");

        // steering must point at the skill/CLI, not MCP tools that don't exist here
        let steer = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        assert!(steer.contains("rust"), "steering names the locked language");
        assert!(!steer.contains("mcp__grove__"), "skill steering names no MCP tools");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mcp_target_writes_mcp_json_and_steering() {
        let dir = tmp("harness_mcp");
        seed_lock(&dir);
        let wrote = reconcile_harness(&dir, None, Mode::Mcp).unwrap();

        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        assert_eq!(wrote.len(), 2, ".mcp.json + steering reported: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn both_target_writes_mcp_json_and_steering() {
        let dir = tmp("harness_both");
        seed_lock(&dir);
        reconcile_harness(&dir, None, Mode::Both).unwrap();
        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
