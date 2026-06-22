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
    /// Grammars only — the skill itself is distributed via the skills tool
    /// (`npx skills add Entelligentsia/grove`), which steers to MCP-or-CLI.
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

    // 4. Provision grammars (every target) + write the harness glue the target
    //    asks for. The lock is always written; `.mcp.json`/CLAUDE.md only for
    //    the MCP targets — a skill-only init leaves the repo otherwise untouched
    //    (the skill artifact is distributed via the skills tool, not by init).
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
        println!("\n  skill      grammars are ready. Install the cross-harness skill with:");
        println!("             npx skills add Entelligentsia/grove");
        println!("             (it steers your agent to grove's MCP tools when present,");
        println!("              else the grove CLI — same engine either way.)");
    }
    Ok(())
}

/// Write the harness glue for `target` and pin `grove.lock`. The lock is
/// always written; `.mcp.json` + CLAUDE.md only for MCP targets. Returns a
/// human list of what was written, in order.
fn write_harness(root: &Path, target: Target, langs: &[String]) -> Result<Vec<String>> {
    let mut wrote = Vec::new();
    if target.writes_mcp() {
        wrote.push(write_mcp_json(root)?);
        wrote.push(write_claude_md(root, langs)?);
    }
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
fn write_claude_md(root: &Path, langs: &[String]) -> Result<String> {
    let path = root.join("CLAUDE.md");
    // Claude Code exposes the tools as `mcp__<server-key>__<tool>`; steer with
    // those exact names. A future AGENTS.md/.cursorrules adapter passes its own.
    let prefix = format!("mcp__{MCP_SERVER_KEY}__");
    let section = claude_section(langs, &prefix);
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

fn claude_section(langs: &[String], p: &str) -> String {
    format!(
        "{CLAUDE_START}
## Code intelligence: use grove first

This project has the **grove** MCP server — structural code tools backed by
tree-sitter ({langs}). For any question about *where code is, what a file
contains, or how code connects*, prefer grove over grepping or reading whole
files. grove returns byte-precise, token-cheap answers with stable `symbol-id`s
(`<lang>:<relpath>#<name>@<row>`) you can pass between calls.

| You want to… | Use |
|---|---|
| see what's in a file (not read all of it) | `{p}outline` (add `kind=…` / `detail=0` on large files) |
| find where something is defined or used | `{p}symbols`, or `{p}definition` for go-to-def |
| read one function/type body | `{p}source` (by id, or file+name) |
| know what calls a function | `{p}callers` |
| confirm an edit didn't break syntax | `{p}check` (after editing) |

The grove tools are **deferred** MCP tools — when a code question arrives, load
their schemas with ToolSearch up front rather than defaulting to a search agent
or grep. `{p}callers`/`{p}definition` are name-based (not receiver-type resolved).
{CLAUDE_END}",
        langs = langs.join(", "),
    )
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
    fn claude_section_is_delimited_and_names_langs_and_prefix() {
        let s = claude_section(&["rust".into(), "python".into()], "mcp__grove__");
        assert!(s.starts_with(CLAUDE_START));
        assert!(s.trim_end().ends_with(CLAUDE_END));
        assert!(s.contains("rust, python"));
        assert!(s.contains("mcp__grove__outline"));
        assert!(s.contains("mcp__grove__callers"));
    }

    #[test]
    fn write_claude_md_creates_then_updates_idempotently() {
        let dir = tmp("claude_idem");
        let path = dir.join("CLAUDE.md");

        write_claude_md(&dir, &["rust".into()]).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first.matches(CLAUDE_START).count(), 1);
        assert!(first.contains("rust"));

        // Re-run with a different language set: section is replaced in place, not
        // duplicated.
        write_claude_md(&dir, &["python".into()]).unwrap();
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

        write_claude_md(&dir, &["rust".into()]).unwrap();
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
    fn skill_target_writes_only_the_lock() {
        let dir = tmp("harness_skill");
        let wrote = write_harness(&dir, Target::Skill, &["rust".into()]).unwrap();

        assert!(dir.join("grove.lock").exists(), "lock is always written");
        assert!(!dir.join(".mcp.json").exists(), "skill mode writes no .mcp.json");
        assert!(!dir.join("CLAUDE.md").exists(), "skill mode writes no CLAUDE.md");
        assert_eq!(wrote.len(), 1, "only the lock reported: {wrote:?}");

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
