//! `grove ingest` — build registry artifacts from curated source specs.
//!
//! For each grammar in the sources file, pull its **official tree-sitter release
//! wasm** (native `dylink.0`) + the repo's `tags.scm`, attach grove's curated
//! profile/extensions, and lay out `<out>/<lang>/{grammar.wasm, tags.scm,
//! manifest.json}`, then regenerate `index.json`. This is the registry's build
//! step (CI / maintainer), not an end-user command.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::{fetch, registry};

#[derive(Deserialize)]
struct Sources {
    #[serde(default)]
    grammars: Vec<Spec>,
}

#[derive(Deserialize)]
struct Spec {
    name: String,
    /// GitHub repo, e.g. `tree-sitter/tree-sitter-python`.
    repo: String,
    /// Release tag, e.g. `v0.25.0`.
    rev: String,
    /// Release asset filename, e.g. `tree-sitter-python.wasm`.
    wasm_asset: String,
    /// Path to the tags query in the repo at `rev`.
    #[serde(default = "default_tags")]
    tags_path: String,
    extensions: Vec<String>,
    profile: Value,
}

fn default_tags() -> String {
    "queries/tags.scm".to_string()
}

pub fn run(sources: &Path, out: &Path, only: &[String]) -> Result<()> {
    let specs: Sources = serde_json::from_str(
        &std::fs::read_to_string(sources)
            .with_context(|| format!("reading {}", sources.display()))?,
    )
    .context("parsing sources spec")?;

    let targets: Vec<&Spec> = if only.is_empty() {
        specs.grammars.iter().collect()
    } else {
        only.iter()
            .map(|n| {
                specs
                    .grammars
                    .iter()
                    .find(|s| &s.name == n)
                    .with_context(|| format!("`{n}` is not in {}", sources.display()))
            })
            .collect::<Result<_>>()?
    };

    for s in targets {
        let version = s.rev.trim_start_matches('v').to_string();
        println!("  {} {} ← {}@{}", s.name, version, s.repo, s.rev);

        // Official release wasm (native dylink.0 module).
        let wasm_url = format!(
            "https://github.com/{}/releases/download/{}/{}",
            s.repo, s.rev, s.wasm_asset
        );
        let wasm = fetch::get_bytes(&wasm_url)
            .with_context(|| format!("downloading {} release wasm", s.name))?;
        if !wasm.windows(8).any(|w| w == b"dylink.0") {
            bail!(
                "{}: {} is not a native (dylink.0) module — won't load in grove",
                s.name,
                s.wasm_asset
            );
        }

        // tags.scm from the repo at the pinned rev.
        let tags_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            s.repo, s.rev, s.tags_path
        );
        let tags = fetch::get_bytes(&tags_url)
            .with_context(|| format!("downloading {} tags.scm", s.name))?;

        // manifest = identity + provenance + grove's curated profile/extensions.
        let manifest = serde_json::json!({
            "name": s.name,
            "version": version,
            "extensions": s.extensions,
            "source": { "repo": s.repo, "rev": s.rev },
            "profile": s.profile,
        });

        let dir = out.join(&s.name);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        std::fs::write(dir.join("grammar.wasm"), &wasm)?;
        std::fs::write(dir.join("tags.scm"), &tags)?;
        std::fs::write(
            dir.join("manifest.json"),
            format!("{}\n", serde_json::to_string_pretty(&manifest)?),
        )?;
        println!("    ✓ {} KB wasm + {}-byte tags", wasm.len() / 1024, tags.len());
    }

    let catalog = registry::build_index(out)?;
    std::fs::write(
        out.join("index.json"),
        format!("{}\n", serde_json::to_string_pretty(&catalog)?),
    )?;
    println!("\nwrote {}/index.json", out.display());
    Ok(())
}
