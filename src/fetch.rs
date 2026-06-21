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

/// Default host: the grove registry repo via jsDelivr's GitHub CDN.
const DEFAULT_HOST: &str = "https://cdn.jsdelivr.net/gh/grove-lang/registry@main";

#[derive(Deserialize)]
struct Catalog {
    #[serde(default)]
    grammars: Vec<CatEntry>,
}

#[derive(Deserialize)]
struct CatEntry {
    name: String,
    version: String,
    /// filename → sha256, for every file grove serves for this grammar.
    #[serde(default)]
    files: std::collections::HashMap<String, String>,
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
        let base = format!("{host}/{}", e.name);

        // Download every file and verify its hash *before* writing any (atomic).
        let mut names: Vec<&String> = e.files.keys().collect();
        names.sort();
        let mut blobs = Vec::new();
        for fname in names {
            let want = &e.files[fname];
            let bytes = get_bytes(&format!("{base}/{fname}"))?;
            let got = sha256(&bytes);
            if &got != want {
                bail!("{}/{fname}: hash mismatch — catalog {want}, downloaded {got}", e.name);
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
