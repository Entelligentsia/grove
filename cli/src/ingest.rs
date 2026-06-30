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
    /// Path to the locals query in the repo at `rev` (scope-aware resolution,
    /// ADR 0001). Optional — many grammars ship one here; if it's absent upstream
    /// the language simply keeps directory-wide name lookup.
    #[serde(default = "default_locals")]
    locals_path: String,
    /// Curated query patterns appended after the upstream `tags.scm`. Lets grove
    /// fill gaps in an upstream query (e.g. C ships no rule for file-scope
    /// variable/array definitions) without forking the whole file. Patterns are
    /// appended verbatim, so they must use the same `@definition.*`/`@name`
    /// captures the engine interprets.
    #[serde(default)]
    extra_tags: String,
    extensions: Vec<String>,
    profile: Value,
}

fn default_tags() -> String {
    "queries/tags.scm".to_string()
}

fn default_locals() -> String {
    "queries/locals.scm".to_string()
}

/// Ingest one grammar; returns (wasm KB, has_tags). Errors are per-language.
fn ingest_one(s: &Spec, out: &Path) -> Result<(usize, bool)> {
    let version = s.rev.trim_start_matches('v').to_string();

    let wasm_url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        s.repo, s.rev, s.wasm_asset
    );
    let wasm = fetch::get_bytes(&wasm_url).with_context(|| format!("wasm {wasm_url}"))?;
    if !wasm.windows(8).any(|w| w == b"dylink.0") {
        bail!("{} is not a native (dylink.0) module", s.wasm_asset);
    }

    // tags.scm is optional: not every grammar ships one (css/html/json have no
    // definitions to extract). Without it the grammar still parses and `check`s;
    // the symbol tools are graceful no-ops.
    let tags_url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}",
        s.repo, s.rev, s.tags_path
    );
    let (mut tags, has_tags) = match fetch::get_bytes(&tags_url) {
        Ok(t) => (t, true),
        Err(_) => (b"; no upstream tags query for this grammar\n".to_vec(), false),
    };
    if !s.extra_tags.is_empty() {
        if !tags.ends_with(b"\n") {
            tags.push(b'\n');
        }
        tags.extend_from_slice(b"\n; --- grove curated additions ---\n");
        tags.extend_from_slice(s.extra_tags.as_bytes());
        if !s.extra_tags.ends_with('\n') {
            tags.push(b'\n');
        }
    }

    // locals.scm is optional too: fetch it if upstream ships one at `locals_path`,
    // otherwise the language keeps directory-wide name lookup (no scope-aware
    // go-to-def). A 404 is not an error.
    let locals_url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}",
        s.repo, s.rev, s.locals_path
    );
    let locals = fetch::get_bytes(&locals_url).ok();

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
    if let Some(l) = &locals {
        std::fs::write(dir.join("locals.scm"), l)?;
    }
    std::fs::write(
        dir.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok((wasm.len() / 1024, has_tags))
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

    // Continue past a failing language; report a summary at the end.
    let mut ok = 0;
    let mut failed = Vec::new();
    for s in targets {
        match ingest_one(s, out) {
            Ok((kb, has_tags)) => {
                let tags = if has_tags { "" } else { "  (no tags query)" };
                println!("  ✓ {:<16} {}  ({kb} KB){tags}", s.name, s.rev);
                ok += 1;
            }
            Err(e) => {
                eprintln!("  ✗ {:<16} {:#}", s.name, e);
                failed.push(s.name.clone());
            }
        }
    }
    println!("\n{ok} ingested, {} failed{}", failed.len(),
        if failed.is_empty() { String::new() } else { format!(": {}", failed.join(", ")) });

    let catalog = registry::build_index(out, None)?;
    std::fs::write(
        out.join("index.json"),
        format!("{}\n", serde_json::to_string_pretty(&catalog)?),
    )?;
    println!("\nwrote {}/index.json", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_sources(tag: &str, body: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("grove_ingest_src_{}_{tag}.json", std::process::id()));
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn default_tags_path() {
        assert_eq!(default_tags(), "queries/tags.scm");
    }

    #[test]
    fn sources_spec_deserializes_with_defaults() {
        let json = r#"{ "grammars": [
            { "name": "python", "repo": "tree-sitter/tree-sitter-python", "rev": "v0.25.0",
              "wasm_asset": "tree-sitter-python.wasm", "extensions": ["py"], "profile": {} }
        ] }"#;
        let s: Sources = serde_json::from_str(json).unwrap();
        assert_eq!(s.grammars.len(), 1);
        let g = &s.grammars[0];
        assert_eq!(g.name, "python");
        assert_eq!(g.tags_path, "queries/tags.scm", "tags_path defaults when omitted");
        assert!(g.extra_tags.is_empty(), "extra_tags defaults to empty when omitted");
    }

    #[test]
    fn extra_tags_deserializes_when_present() {
        let json = r#"{ "grammars": [
            { "name": "c", "repo": "tree-sitter/tree-sitter-c", "rev": "v0.24.2",
              "wasm_asset": "tree-sitter-c.wasm", "extensions": ["c"], "profile": {},
              "extra_tags": "(translation_unit (declaration declarator: (identifier) @name) @definition.variable)\n" }
        ] }"#;
        let s: Sources = serde_json::from_str(json).unwrap();
        assert!(s.grammars[0].extra_tags.contains("@definition.variable"));
    }

    #[test]
    fn run_errors_when_only_names_an_unknown_grammar() {
        // Filtering happens after parsing but before any network fetch.
        let src = write_sources("unknown_only", r#"{ "grammars": [
            { "name": "python", "repo": "r", "rev": "v1", "wasm_asset": "p.wasm",
              "extensions": ["py"], "profile": {} } ] }"#);
        let out = std::env::temp_dir().join(format!("grove_ingest_out_{}", std::process::id()));
        let err = run(&src, &out, &["nope".to_string()]).unwrap_err();
        assert!(err.to_string().contains("`nope` is not in"), "got: {err}");
        std::fs::remove_file(&src).ok();
    }

    #[test]
    fn run_errors_on_missing_sources_file() {
        let missing = std::env::temp_dir().join(format!("grove_ingest_absent_{}.json", std::process::id()));
        let out = std::env::temp_dir().join("grove_ingest_out_x");
        let err = run(&missing, &out, &[]).unwrap_err();
        assert!(err.to_string().contains("reading"), "got: {err}");
    }

    #[test]
    fn run_errors_on_malformed_sources() {
        let src = write_sources("malformed", "{ not valid json");
        let out = std::env::temp_dir().join("grove_ingest_out_y");
        let err = run(&src, &out, &[]).unwrap_err();
        assert!(err.to_string().contains("parsing sources spec"), "got: {err}");
        std::fs::remove_file(&src).ok();
    }
}
