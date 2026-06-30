//! `grove init` — make a project one where the agent actually uses grove.
//!
//! Two halves, per VISION §6.4.1: registration (`.mcp.json`) makes the tools
//! *available*; a steering directive (`CLAUDE.md`) makes them *adopted*. Plus a
//! `grove.lock` pinning the grammars the project needs. Idempotent: re-running
//! updates the grove pieces without clobbering anything else.

use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde_json::{json, Value};

use grove_core::init::provision_project;
use grove_core::registry;

const CLAUDE_START: &str = "<!-- grove:start -->";
const CLAUDE_END: &str = "<!-- grove:end -->";

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
}

/// The key grove registers itself under in `.mcp.json`. Claude Code namespaces
/// an MCP server's tools as `mcp__<key>__<tool>`, so this also determines the
/// tool prefix the steering directive must use to name the real tools.
const MCP_SERVER_KEY: &str = "grove";

pub fn run(root: &Path, target: Target, dry_run: bool) -> Result<()> {
    println!("grove init  scanning {} (as {:?})\n", root.display(), target);

    // Provision grammars + the lock (clap-free core). An empty return is the
    // contract for "nothing provisioned" — a dry-run / no-files / no-cached-
    // grammars short-circuit; core already printed its terminal line, so stop.
    let provisioned = provision_project(root, dry_run)?;
    if provisioned.is_empty() {
        return Ok(());
    }

    // Write the harness glue for the chosen target, then report harness writes
    // first and the provisioning actions after — preserving today's `wrote`
    // order: `.mcp.json`, `CLAUDE.md`, `grove.lock`. Steering is not optional:
    // without it a cold agent ignores grove and falls back to grep/whole-file
    // reads (VISION §6.4.1). The skill artifact itself still comes from the
    // skills tool, not init.
    let mut wrote = write_harness(root, target)?;
    wrote.extend(provisioned);

    println!("\n  wrote");
    for w in &wrote {
        println!("             ✓ {w}");
    }
    if target.writes_mcp() {
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

/// Write the harness glue for `target`: `.mcp.json` (MCP targets) and a
/// `CLAUDE.md` steering block (every target but `Grammars`). Grammar
/// provisioning + the `grove.lock` write live in `grove_core::init`. The
/// `langs` for steering come from the lock that core just wrote. Returns a
/// human list of what was written, in order.
fn write_harness(root: &Path, target: Target) -> Result<Vec<String>> {
    let mut wrote = Vec::new();
    if target.writes_mcp() {
        wrote.push(write_mcp_json(root)?);
    }
    // Steering for every target except `Grammars` — availability isn't adoption
    // (VISION §6.4.1), but embedding hosts supply their own steering and want
    // project files left untouched.
    if target.writes_steering() {
        let langs = registry::locked_langs(&root.join("grove.lock"))?;
        wrote.push(write_claude_md(root, &langs, target)?);
    }
    Ok(wrote)
}

/// Add (or refresh) the grove MCP server in `.mcp.json`, preserving other servers.
fn write_mcp_json(root: &Path) -> Result<String> {
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
        "args": ["serve"],
    });
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(".mcp.json (registration — the tools exist)".to_string())
}

/// Write/refresh the grove steering section in `CLAUDE.md`, between markers, so
/// re-running is idempotent and never disturbs the rest of the file.
fn write_claude_md(root: &Path, langs: &[String], target: Target) -> Result<String> {
    let path = root.join("CLAUDE.md");
    let section = claude_section(langs, target);
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
    Ok("CLAUDE.md (steering — the tools get used)".to_string())
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

    /// `write_harness` is harness-only now: grammar provisioning + the
    /// `grove.lock` write moved to `grove_core::init::provision_project`, so
    /// these tests seed a lock first (the source of steering's language list)
    /// and assert only the harness files (`.mcp.json` / `CLAUDE.md`).
    fn seed_lock(dir: &Path) {
        std::fs::write(
            dir.join("grove.lock"),
            r#"{"version":1,"grammars":[{"name":"rust","version":"1","wasm":"x"}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn grammars_target_writes_no_harness_files() {
        let dir = tmp("harness_grammars");
        seed_lock(&dir);
        let wrote = write_harness(&dir, Target::Grammars).unwrap();

        assert!(!dir.join("CLAUDE.md").exists(), "grammars mode writes no steering");
        assert!(!dir.join(".mcp.json").exists(), "grammars mode writes no .mcp.json");
        assert!(wrote.is_empty(), "no harness writes reported: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skill_target_writes_steering_but_no_mcp_json() {
        let dir = tmp("harness_skill");
        seed_lock(&dir);
        let wrote = write_harness(&dir, Target::Skill).unwrap();

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
        let wrote = write_harness(&dir, Target::Mcp).unwrap();

        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        assert_eq!(wrote.len(), 2, ".mcp.json + steering reported: {wrote:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn both_target_writes_mcp_json_and_steering() {
        let dir = tmp("harness_both");
        seed_lock(&dir);
        write_harness(&dir, Target::Both).unwrap();
        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
