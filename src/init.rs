//! `grove init` — make a project one where the agent actually uses grove.
//!
//! Two halves, per VISION §6.4.1: registration (`.mcp.json`) makes the tools
//! *available*; a steering directive (`CLAUDE.md`) makes them *adopted*. Plus a
//! `grove.lock` pinning the grammars the project needs. Idempotent: re-running
//! updates the grove pieces without clobbering anything else.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde_json::{json, Value};

use crate::{fetch, registry};

const CLAUDE_START: &str = "<!-- grove:start -->";
const CLAUDE_END: &str = "<!-- grove:end -->";

pub fn run(root: &Path, dry_run: bool) -> Result<()> {
    println!("grove init  scanning {}\n", root.display());

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

    // 4. Write the three artifacts (only for languages now available).
    let langs: Vec<String> = langs.into_iter().filter(|l| is_cached(l)).collect();
    if langs.is_empty() {
        println!("\n  no grammars available — nothing written.");
        return Ok(());
    }
    let mut wrote = Vec::new();
    wrote.push(write_mcp_json(root)?);
    wrote.push(write_claude_md(root, &langs)?);
    let n = registry::write_lock_for(&langs, &root.join("grove.lock"))?;
    wrote.push(format!("grove.lock ({n} grammars)"));

    println!("\n  agent      Claude Code");
    for w in &wrote {
        println!("             ✓ {w}");
    }
    println!("\n  ready      your agent now has grove's tools across its loop:");
    println!("             outline · symbols · source · callers · definition · check");
    println!("\n  try it     start a fresh Claude Code session here and ask a");
    println!("             \"where/what/who-calls\" question — it routes through grove.");
    Ok(())
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
    doc["mcpServers"]["grove"] = json!({
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
    let section = claude_section(langs);
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

fn claude_section(langs: &[String]) -> String {
    format!(
        "{CLAUDE_START}
## Code intelligence: use grove first

This project has the **grove** MCP server — structural code tools backed by
tree-sitter ({langs}). For any question about *where code is, what a file
contains, or how code connects*, prefer grove over grepping or reading whole
files. grove returns byte-precise, token-cheap answers with stable `symbol-id`s
you can pass between calls.

| You want to… | Use |
|---|---|
| see what's in a file (not read all of it) | `outline` (add `kind=…` / `detail=0` on large files) |
| find where something is defined or used | `symbols`, or `definition` for go-to-def |
| read one function/type body | `source` (by id, or file+name) |
| know what calls a function | `callers` |
| confirm an edit didn't break syntax | `check` (after editing) |

The grove tools are **deferred** MCP tools — when a code question arrives, load
their schemas with ToolSearch up front rather than defaulting to a search agent
or grep. `callers`/`definition` are name-based (not receiver-type resolved).
{CLAUDE_END}",
        langs = langs.join(", ")
    )
}
