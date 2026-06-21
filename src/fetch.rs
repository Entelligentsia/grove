//! `grove fetch` — pull grammars from the hosted registry into the OS cache.
//!
//! Host model: a `grove-registry` GitHub repo served via jsDelivr's GitHub CDN,
//! laid out as `<host>/index.json` (the catalog) and `<host>/<lang>/{grammar.wasm,
//! tags.scm, manifest.json}`. Each wasm's sha256 is verified against the catalog
//! before it lands in the cache. Override the host with `GROVE_REGISTRY_URL`.

use std::io::Read;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::registry;

/// Default host for the catalog + per-language text files (tags.scm, manifest).
/// raw.githubusercontent serves these reliably (jsDelivr's per-file cold-fetch
/// 502s intermittently). The heavy wasm binaries are GitHub Release assets (see
/// `release_base` in the catalog), so this stays light. Override with
/// `GROVE_REGISTRY_URL` to self-host or mirror.
const DEFAULT_HOST: &str = "https://raw.githubusercontent.com/Entelligentsia/grove-registry/v1";

#[derive(Deserialize)]
struct Catalog {
    /// Base URL for release-asset files (GitHub Releases); needed if any file
    /// entry has an `asset`.
    #[serde(default)]
    release_base: Option<String>,
    #[serde(default)]
    grammars: Vec<CatEntry>,
}

#[derive(Deserialize)]
struct CatEntry {
    name: String,
    version: String,
    /// File extensions this grammar handles (schema 2+); used by `grove init`
    /// to detect a project's languages before any grammar is fetched.
    #[serde(default)]
    extensions: Vec<String>,
    /// filename → where/what to fetch + its hash.
    #[serde(default)]
    files: std::collections::HashMap<String, FileRef>,
}

/// A grammar as listed in the hosted catalog: its id and the extensions it
/// covers. `grove init` maps a project's files to languages through this — the
/// full set, not just what's already cached.
pub struct CatalogGrammar {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Fetch the hosted catalog and return every grammar's id + extensions.
pub fn catalog_grammars() -> Result<Vec<CatalogGrammar>> {
    let host = host();
    let catalog: Catalog = serde_json::from_slice(&get_bytes(&format!("{host}/index.json"))?)
        .context("parsing index.json catalog")?;
    Ok(catalog
        .grammars
        .into_iter()
        .map(|g| CatalogGrammar { name: g.name, extensions: g.extensions })
        .collect())
}

#[derive(Deserialize)]
struct FileRef {
    sha256: String,
    /// If set, the file is a release asset of this name under `release_base`;
    /// otherwise it sits in the repo at `<host>/<lang>/<filename>`.
    #[serde(default)]
    asset: Option<String>,
}

fn host() -> String {
    std::env::var("GROVE_REGISTRY_URL")
        .unwrap_or_else(|_| DEFAULT_HOST.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| anyhow!("GET {url}: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .with_context(|| format!("reading {url}"))?;
    Ok(buf)
}

fn sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{:x}", h.finalize())
}

/// Fetch the named languages (or all in the catalog) into the OS cache.
pub fn run(langs: &[String], force: bool) -> Result<()> {
    let host = host();
    println!("registry host: {host}\n");

    let catalog: Catalog = serde_json::from_slice(&get_bytes(&format!("{host}/index.json"))?)
        .context("parsing index.json catalog")?;

    let targets: Vec<&CatEntry> = if langs.is_empty() {
        catalog.grammars.iter().collect()
    } else {
        langs
            .iter()
            .map(|l| {
                catalog
                    .grammars
                    .iter()
                    .find(|g| &g.name == l)
                    .with_context(|| format!("`{l}` is not in the catalog at {host}"))
            })
            .collect::<Result<_>>()?
    };

    let cache = registry::cache_root().context("no OS cache directory available")?;
    let mut fetched = 0;
    for e in targets {
        let dir = cache.join(&e.name);
        if dir.join("grammar.wasm").exists() && !force {
            println!("  {:<12} {} · cached", e.name, e.version);
            continue;
        }
        if e.files.is_empty() {
            bail!("catalog entry for `{}` lists no files", e.name);
        }
        // Download every file and verify its hash *before* writing any (atomic).
        let mut names: Vec<&String> = e.files.keys().collect();
        names.sort();
        let mut blobs = Vec::new();
        for fname in names {
            let fref = &e.files[fname];
            let url = match &fref.asset {
                Some(asset) => {
                    let base = catalog
                        .release_base
                        .as_deref()
                        .context("catalog has release assets but no release_base")?;
                    format!("{base}/{asset}")
                }
                None => format!("{host}/{}/{fname}", e.name),
            };
            let bytes = get_bytes(&url)?;
            let got = sha256(&bytes);
            if got != fref.sha256 {
                bail!("{}/{fname}: hash mismatch — catalog {}, downloaded {got}", e.name, fref.sha256);
            }
            blobs.push((fname.clone(), bytes));
        }

        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        for (fname, bytes) in &blobs {
            std::fs::write(dir.join(fname), bytes)?;
        }
        let kb = blobs
            .iter()
            .find(|(n, _)| n == "grammar.wasm")
            .map_or(0, |(_, b)| b.len() / 1024);
        println!("  {:<12} {} ✓ {} files, {} KB", e.name, e.version, blobs.len(), kb);
        fetched += 1;
    }
    println!("\n{fetched} fetched · cache: {}", cache.display());
    Ok(())
}
