//! Standalone library smoke test for `grove-core` (GROVE-S01-T04, AC3/AC4).
//!
//! This crate lives *outside* the parent workspace (its `Cargo.toml` carries an
//! empty `[workspace]` table) and depends on `grove-core` purely by path. It
//! exercises two public entry points — `ops::symbols` over a Rust fixture and
//! `init::provision_project` in dry-run mode — proving the curated public API is
//! usable from a foreign crate without reaching for the `grove` CLI.

use std::path::{Path, PathBuf};

use grove_core::{init, ops};

/// Resolve the repo-root `registry/` directory relative to this crate, so the
/// grammar resolution `ops::symbols` performs is reproducible and independent of
/// machine-specific `~/.cache/grove` state. `CARGO_MANIFEST_DIR` is
/// `<repo>/engineering/sprints/GROVE-S01/GROVE-S01-T04/smoke`; its 5th ancestor
/// is the repo root.
fn repo_registry() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .ancestors()
        .nth(5)
        .expect("repo root is the 5th ancestor of the smoke crate manifest dir");
    repo_root.join("registry")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Point grammar resolution at the repo's checked-in registry. This is the
    // explicit `GROVE_REGISTRY` precedence step that short-circuits the user-cache
    // and (non-existent) dev-root fallbacks.
    let registry = repo_registry();
    std::env::set_var("GROVE_REGISTRY", &registry);
    println!("GROVE_REGISTRY = {}", registry.display());

    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("sample");
    println!("sample tree   = {}\n", sample.display());

    // --- AC3 part 1: ops::symbols over the Rust fixture ---
    println!("== ops::symbols(sample, defs only) ==");
    let syms = ops::symbols(&sample, None, None, false, false)?;
    for s in &syms {
        println!("  {:<10} {:<12} {}:{}", s.kind, s.name, s.file, s.line);
    }
    println!("  ({} definitions)\n", syms.len());
    assert!(
        !syms.is_empty(),
        "expected non-empty symbols from the Rust fixture"
    );

    // --- AC3 part 2: init::provision_project (dry-run) ---
    println!("== init::provision_project(sample, dry_run=true) ==");
    let provisioned = init::provision_project(&sample, true)?;
    println!("  returned {} wrote-actions (dry-run ⇒ empty)\n", provisioned.len());

    println!("smoke OK");
    Ok(())
}
