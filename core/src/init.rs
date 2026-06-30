//! Grammar provisioning for `grove init` — the clap-free half.
//!
//! `provision_project` owns everything that touches only grammars and the
//! lockfile: it scans the project's extensions, counts files per language,
//! auto-fetches any detected grammar the cache is missing, and writes/refreshes
//! `grove.lock`. The agent harness (`.mcp.json` + `CLAUDE.md`) lives CLI-side;
//! this module is a library API consumable by any embedding host, with no
//! dependency on `clap` or the CLI's `Target` enum.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use ignore::WalkBuilder;

use crate::{fetch, registry};

/// Provision the grammars `root` needs and pin them in `grove.lock`.
///
/// Scans extensions, counts project files per language, auto-fetches missing
/// grammars into the OS cache, filters to cached languages, and writes the lock.
/// Prints the provisioning narration (`detected …`, `fetching …`, the offline
/// `note …`, the dry-run note, and the `no files` / `no grammars` terminals).
///
/// Returns the provisioning wrote-actions — on the happy path
/// `["grove.lock (N grammars)"]`; on any short-circuit (dry-run, no files
/// matched, no cached grammars) an **empty `Vec`**, which is the contract telling
/// the caller "nothing was provisioned, stop." Writes no `.mcp.json` / `CLAUDE.md`.
pub fn provision_project(root: &Path, dry_run: bool) -> Result<Vec<String>> {
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
        return Ok(Vec::new());
    }
    for (lang, n) in &counts {
        println!("  detected   {:<10} {} files", lang, n);
    }
    let langs: Vec<String> = counts.keys().cloned().collect();

    if dry_run {
        println!("\n  (dry run — no files written)");
        return Ok(Vec::new());
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

    // 4. Provision grammars (every target). Filter to what is actually cached
    //    and pin it in `grove.lock`. The harness glue (`.mcp.json` / `CLAUDE.md`)
    //    is written CLI-side; this returns the provisioning wrote-action.
    let langs: Vec<String> = langs.into_iter().filter(|l| is_cached(l)).collect();
    if langs.is_empty() {
        println!("\n  no grammars available — nothing written.");
        return Ok(Vec::new());
    }
    let n = registry::write_lock_for(&langs, &root.join("grove.lock"))?;
    Ok(vec![format!("grove.lock ({n} grammars)")])
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("grove_provision_test_{}_{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn provision_empty_project_writes_nothing_and_returns_empty() {
        let dir = tmp("empty");
        // A dir with no source files matches no grammar → short-circuit, empty Vec.
        let wrote = provision_project(&dir, false).unwrap();
        assert!(wrote.is_empty(), "no files matched → no provisioning: {wrote:?}");
        assert!(!dir.join("grove.lock").exists(), "no lock written on empty short-circuit");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn provision_dry_run_writes_no_lock_and_returns_empty() {
        let dir = tmp("dry_run");
        // A Rust file so detection finds a language, but dry-run must short-circuit
        // before any fetch/lock write and return an empty Vec.
        std::fs::write(dir.join("main.rs"), "fn main() {}\n").unwrap();
        let wrote = provision_project(&dir, true).unwrap();
        assert!(wrote.is_empty(), "dry-run provisions nothing: {wrote:?}");
        assert!(!dir.join("grove.lock").exists(), "dry-run writes no lock");
        std::fs::remove_dir_all(&dir).ok();
    }
}
