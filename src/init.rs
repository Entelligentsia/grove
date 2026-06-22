//! `grove init` — make a project one where the agent actually uses grove.
//!
//! Two halves, per VISION §6.4.1: registration (`.mcp.json`) makes the tools
//! *available*; a steering directive (`CLAUDE.md`) makes them *adopted*. Plus a
//! `grove.lock` pinning the grammars the project needs. Idempotent: re-running
//! updates the grove pieces without clobbering anything else.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;
use ignore::WalkBuilder;
use serde_json::{json, Value};

use crate::{fetch, registry};

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
}

impl Target {
    fn writes_mcp(self) -> bool {
        matches!(self, Target::Mcp | Target::Both)
    }
    fn is_skill(self) -> bool {
        matches!(self, Target::Skill | Target::Both)
    }
}

/// The key grove registers itself under in `.mcp.json`. Claude Code namespaces
/// an MCP server's tools as `mcp__<key>__<tool>`, so this also determines the
/// tool prefix the steering directive must use to name the real tools.
const MCP_SERVER_KEY: &str = "grove";

pub fn run(root: &Path, target: Target, dry_run: bool) -> Result<()> {
    println!("grove init  scanning {} (as {:?})\n", root.display(), target);

    // 1. Build an extension→language map. Prefer the hosted catalog so we detect
    //    languages whose grammar isn't fetched yet (otherwise a project's main
    //    language is silently skipped); fall back to cached grammars offline.
    let (ext_map, online) = extension_map();

    // 2. Count project files per language.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in WalkBuilder::new(root).build().flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if let Some(lang) = ext_map.get(ext) {
                    *counts.entry(lang.clone()).or_default() += 1;
                }
            }
        }
    }

    if counts.is_empty() {
        println!("  no files matched a known grammar.");
        println!("  nothing to do.");
        return Ok(());
    }
    for (lang, n) in &counts {
        println!("  detected   {:<10} {} files", lang, n);
    }
    let langs: Vec<String> = counts.keys().cloned().collect();

    if dry_run {
        println!("\n  (dry run — no files written)");
        return Ok(());
    }

    // 3. Auto-fetch any detected grammar the cache doesn't have yet, so the
    //    tools work the moment init finishes. (Checked against the filesystem,
    //    not the in-memory registry index, which we must not build before the
    //    fetch lands — it is cached on first access.)
    let missing: Vec<String> = langs.iter().filter(|l| !is_cached(l)).cloned().collect();
    if !missing.is_empty() {
        if online {
            println!("\n  fetching   {} grammar(s): {}\n", missing.len(), missing.join(", "));
            fetch::run(&missing, false).context("auto-fetching detected grammars")?;
        } else {
            println!(
                "\n  note       {} not cached: {}\n             offline — run `grove fetch {}` to enable them.",
                if missing.len() == 1 { "language" } else { "languages" },
                missing.join(", "),
                missing.join(" "),
            );
        }
    }

    // 4. Provision grammars (every target) + write the harness glue. The lock
    //    and a CLAUDE.md steering block are written for every target; `.mcp.json`
    //    only for MCP targets. Steering is not optional: without it a cold agent
    //    ignores grove and falls back to grep/whole-file reads (VISION §6.4.1).
    //    The skill artifact itself still comes from the skills tool, not init.
    let langs: Vec<String> = langs.into_iter().filter(|l| is_cached(l)).collect();
    if langs.is_empty() {
        println!("\n  no grammars available — nothing written.");
        return Ok(());
    }
    let wrote = write_harness(root, target, &langs)?;

    println!("\n  wrote");
    for w in &wrote {
        println!("             ✓ {w}");
    }
    if target.writes_mcp() {
        println!("\n  ready      your agent now has grove's tools across its loop:");
        println!("             outline · symbols · source · callers · definition · check");
        println!("\n  try it     start a fresh Claude Code session here and ask a");
        println!("             \"where/what/who-calls\" question — it routes through grove.");
    }
    if target.is_skill() {
        println!("\n  skill      grammars + steering are ready. Install the cross-harness");
        println!("             skill with:  npx skills add Entelligentsia/grove");
        println!("             (it steers your agent to grove's MCP tools when present,");
        println!("              else the grove CLI — same engine either way.)");
    }
    Ok(())
}

/// Write the harness glue for `target` and pin `grove.lock`. The lock and a
/// CLAUDE.md steering block are written for every target; `.mcp.json` only for
/// MCP targets. Returns a human list of what was written, in order.
fn write_harness(root: &Path, target: Target, langs: &[String]) -> Result<Vec<String>> {
    let mut wrote = Vec::new();
    if target.writes_mcp() {
        wrote.push(write_mcp_json(root)?);
    }
    // Steering for every target — availability isn't adoption (VISION §6.4.1).
    wrote.push(write_claude_md(root, langs, target)?);
    let n = registry::write_lock_for(langs, &root.join("grove.lock"))?;
    wrote.push(format!("grove.lock ({n} grammars)"));
    Ok(wrote)
}

/// Extension→language map and whether it came from the hosted catalog (`true`,
/// covering all languages) or — when offline — the local cache (`false`).
fn extension_map() -> (HashMap<String, String>, bool) {
    match fetch::catalog_grammars() {
        Ok(grammars) => {
            let mut m = HashMap::new();
            for g in &grammars {
                for ext in &g.extensions {
                    m.insert(ext.clone(), g.name.clone());
                }
            }
            (m, true)
        }
        Err(e) => {
            eprintln!("  note: catalog unavailable ({e}); detecting from cached grammars only.");
            let mut m = HashMap::new();
            for man in registry::manifests() {
                for ext in &man.extensions {
                    m.insert(ext.clone(), man.name.clone());
                }
            }
            (m, false)
        }
    }
}

/// True if `lang`'s grammar is already in the OS cache. Checks the filesystem
/// directly so it never triggers the (once-initialised) in-memory registry index.
fn is_cached(lang: &str) -> bool {
    registry::cache_root().is_some_and(|c| c.join(lang).join("grammar.wasm").exists())
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
/// carries the full imperative procedure inline (there is no skill file to defer
/// to); the skill target is a thin invariant + trigger + ban list that defers to
/// SKILL.md as the single source of truth. Both are imperative and lexically
/// triggered — a hortative "prefer grove" loses to the model's grep prior, and a
/// cold agent without any block defaults to grep/whole-file reads (VISION §6.4.1).
fn claude_section(langs: &[String], target: Target) -> String {
    let langs = langs.join(", ");
    if target.writes_mcp() {
        // Claude Code exposes the tools as `mcp__<server-key>__<tool>`; steer with
        // those exact names. A future AGENTS.md/.cursorrules adapter passes its own.
        let p = format!("mcp__{MCP_SERVER_KEY}__");
        format!(
            "{CLAUDE_START}
## INVARIANT — code navigation goes through grove

Every where-is / what's-in / who-calls action in this project goes through the
**grove** MCP server (tree-sitter backed; languages: {langs}). This is not a
preference. `grep`, `rg`, `read`, `cat`, and `sed` on a source file are
FALLBACKS, allowed only after grove has been tried and returned insufficient
content. Running `grep -rn '<symbol>'`, or reading a whole source file, as your
first action on a code question is a steering violation.

The grove tools are **deferred** MCP tools — the moment a code question arrives,
load their schemas with ToolSearch (do not default to a search agent or grep):
`{p}outline`, `{p}symbols`, `{p}source`, `{p}callers`, `{p}definition`, `{p}check`.

**Trigger — check before every tool call.** If the prompt contains any of — a
file path, a function / type / struct / macro name, or the words \"where is\",
\"what does X define\", \"who calls\", \"show me\", \"find\", \"list\",
\"outline\" — your FIRST tool call MUST be a grove tool. Otherwise grove is optional.

**Procedure.**
1. File but no symbol → `{p}outline` (pass `detail:0` on files > 500 lines).
2. Symbol but no file → `{p}symbols` with `name`.
3. Take the `symbol-id` (`<lang>:<relpath>#<name>@<row>`) from the result.
4. `{p}source` with that **id** → exactly that symbol's body.
5. \"who calls\" → `{p}callers`; \"where defined\" → `{p}definition`.
6. After an edit → `{p}check`.

**Cross-file.** `{p}symbols` over the root (definitions tree-wide) → `{p}callers`
(use sites) → `{p}source` per definition. Do NOT `grep -rn '<type>' .` instead —
grep returns string matches, grove returns semantic definitions.

**Recovery (partial/truncated output).** Re-run `{p}source` with the `symbol-id`
form to force body extraction; still partial → `read` with `offset`/`limit` from
the outline, never the whole file. A single grove miss does NOT justify switching
to grep for later questions — re-run with the id form and continue.

`read` on a 1700-line file floods context with ~50 KB you don't need; `grep`
misses struct/function boundaries. `{p}callers`/`{p}definition` are name-based
(not receiver-type resolved).
{CLAUDE_END}"
        )
    } else {
        format!(
            "{CLAUDE_START}
## INVARIANT — code navigation goes through grove

Every where-is / what's-in / who-calls action in this project goes through
**grove** (tree-sitter backed; languages: {langs}). This is not a preference.
`grep`, `rg`, `read`, `cat`, and `sed` on a source file are FALLBACKS, allowed
only after grove has been tried. Running `grep -rn '<symbol>'`, or reading a
whole source file, as your first action on a code question is a steering violation.

**Trigger — check before every tool call.** If the prompt contains any of — a
file path, a function / type / struct / macro name, or the words \"where is\",
\"what does X define\", \"who calls\", \"show me\", \"find\", \"list\",
\"outline\" — your FIRST action MUST be a grove command, via the **grove skill**
(or the `grove` CLI — same engine). Otherwise grove is optional.

**The procedure, recovery rule, cross-file recipe, and full command surface live
in the grove skill — invoke/read it before answering any triggered question.** It
is the single source of truth; do not rely on remembered flags.

`read` on a large file floods context with bytes you don't need; `grep` misses
struct/function boundaries. grove returns one symbol's exact bytes with a stable
id you pass forward.
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
    fn claude_section_skill_is_thin_imperative_and_defers_to_skill() {
        let s = claude_section(&["rust".into()], Target::Skill);
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("grove skill"), "skill steering defers to the skill");
        assert!(s.contains("MUST"), "steering is imperative, not hortative");
        assert!(s.contains("FALLBACK"), "steering carries the grep/read ban");
        assert!(!s.contains("mcp__grove__"), "skill steering must not name MCP tools");
    }

    #[test]
    fn claude_section_mcp_is_imperative_with_trigger_and_ban() {
        let s = claude_section(&["rust".into()], Target::Mcp);
        assert!(s.contains("MUST"), "imperative trigger");
        assert!(s.contains("FALLBACK"), "grep/read ban list");
        assert!(s.contains("steering violation"), "names the forbidden first move");
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
    }

    #[test]
    fn skill_target_writes_lock_and_steering_but_no_mcp_json() {
        let dir = tmp("harness_skill");
        let wrote = write_harness(&dir, Target::Skill, &["rust".into()]).unwrap();

        assert!(dir.join("grove.lock").exists(), "lock is always written");
        assert!(dir.join("CLAUDE.md").exists(), "skill mode writes steering");
        assert!(!dir.join(".mcp.json").exists(), "skill mode writes no .mcp.json");
        assert_eq!(wrote.len(), 2, "steering + lock reported: {wrote:?}");

        // steering must point at the skill/CLI, not MCP tools that don't exist here
        let steer = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
        assert!(!steer.contains("mcp__grove__"), "skill steering names no MCP tools");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mcp_target_writes_all_three() {
        let dir = tmp("harness_mcp");
        let wrote = write_harness(&dir, Target::Mcp, &["rust".into()]).unwrap();

        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        assert!(dir.join("grove.lock").exists());
        assert_eq!(wrote.len(), 3);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn both_target_writes_all_three() {
        let dir = tmp("harness_both");
        write_harness(&dir, Target::Both, &["rust".into()]).unwrap();
        assert!(dir.join(".mcp.json").exists());
        assert!(dir.join("CLAUDE.md").exists());
        assert!(dir.join("grove.lock").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
