//! `grove fetch` — pull grammars from the hosted registry into the OS cache.
//!
//! Host model: a `grove-registry` GitHub repo served via jsDelivr's GitHub CDN,
//! laid out as `<host>/index.json` (the catalog) and `<host>/<lang>/{grammar.wasm,
//! tags.scm, manifest.json}`. Each wasm's sha256 is verified against the catalog
//! before it lands in the cache. Override the host with `GROVE_REGISTRY_URL`.

use std::{io::Read, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::registry;
use crate::registry::sha256;

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

fn build_agent() -> ureq::Agent {
    crate::proxy::default_config()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .new_agent()
}

pub(crate) fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let agent = build_agent();
    let resp = agent.get(url).call().map_err(|e| anyhow!("GET {url}: {e}"))?;
    let mut buf = Vec::new();
    resp.into_body()
        .into_reader()
        .read_to_end(&mut buf)
        .with_context(|| format!("reading {url}"))?;
    Ok(buf)
}

/// Reject any catalog-supplied name that isn't a single, safe path component.
///
/// The catalog (`index.json`) is fetched over the network and may be hostile or
/// MITM'd. A grammar `name` or filename containing `..`, a path separator, or an
/// absolute prefix would escape the cache directory once joined (path traversal,
/// arbitrary file write). We only ever write `<cache>/<lang>/<file>`, so every
/// such segment must be a plain file name — exactly one `Normal` path component
/// with no separator of either platform.
fn safe_segment(s: &str) -> Result<()> {
    use std::path::{Component, Path};
    let mut comps = Path::new(s).components();
    let single_normal =
        matches!(comps.next(), Some(Component::Normal(_))) && comps.next().is_none();
    if s.is_empty() || s.contains('/') || s.contains('\\') || !single_normal {
        bail!("catalog path segment `{s}` is not a plain file name (possible path traversal)");
    }
    Ok(())
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
        safe_segment(&e.name)?;
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
            safe_segment(fname)?;
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::{build_agent, host, safe_segment, sha256, Catalog, DEFAULT_HOST};
    use crate::proxy::PROXY_ENV_TEST_LOCK;

    #[test]
    fn host_defaults_and_honors_env_override() {
        // All env mutation kept in one test so it can't race a parallel reader.
        std::env::remove_var("GROVE_REGISTRY_URL");
        assert_eq!(host(), DEFAULT_HOST, "defaults to the hosted registry");

        std::env::set_var("GROVE_REGISTRY_URL", "https://mirror.test/grove/");
        assert_eq!(host(), "https://mirror.test/grove", "override + trailing slash trimmed");

        std::env::remove_var("GROVE_REGISTRY_URL");
    }

    #[test]
    fn catalog_parses_schema_2_with_release_assets() {
        let json = r#"{
            "schema": 2,
            "release_base": "https://example.test/releases/v1",
            "grammars": [
                { "name": "rust", "version": "0.24.0", "extensions": ["rs"],
                  "files": {
                      "grammar.wasm": { "sha256": "sha256:aa", "asset": "rust.wasm" },
                      "tags.scm": { "sha256": "sha256:bb" }
                  } }
            ]
        }"#;
        let cat: Catalog = serde_json::from_str(json).unwrap();
        assert_eq!(cat.release_base.as_deref(), Some("https://example.test/releases/v1"));
        assert_eq!(cat.grammars.len(), 1);
        let g = &cat.grammars[0];
        assert_eq!(g.name, "rust");
        assert_eq!(g.extensions, vec!["rs"]);
        assert_eq!(g.files["grammar.wasm"].asset.as_deref(), Some("rust.wasm"));
        assert_eq!(g.files["grammar.wasm"].sha256, "sha256:aa");
        assert!(g.files["tags.scm"].asset.is_none(), "repo-served file has no asset");
    }

    #[test]
    fn catalog_tolerates_missing_optional_fields() {
        let cat: Catalog = serde_json::from_str(r#"{ "grammars": [] }"#).unwrap();
        assert!(cat.release_base.is_none());
        assert!(cat.grammars.is_empty());
    }

    #[test]
    fn fetch_verifies_with_the_shared_helper() {
        // `fetch` no longer has its own digest — it verifies downloads against
        // the exact `registry::sha256` the index/lockfile were built with, so a
        // format change can't drift the producer and verifier apart (#15).
        assert_eq!(
            sha256(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(sha256(b"abc"), crate::registry::sha256(b"abc"));
    }

    #[test]
    fn build_agent_honors_http_proxy_environment() {
        let _guard = PROXY_ENV_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy_addr = addr.to_string();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            // A single `read()` can return a partial request, so drain the
            // stream until the header terminator shows up (mirrors the
            // round-trip test in proxy.rs).
            fn read_headers(stream: &mut std::net::TcpStream) -> String {
                let mut acc = String::new();
                let mut buf = [0u8; 512];
                while !acc.contains("\r\n\r\n") {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0, "peer closed before sending a full request");
                    acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                acc
            }

            loop {
                let raw = read_headers(&mut stream);
                if raw.starts_with("CONNECT") {
                    // ureq tunnels through an HTTP proxy via CONNECT even for a
                    // plain `http://` target; acknowledge the tunnel and keep
                    // reading the real request on the same connection.
                    stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").unwrap();
                    continue;
                }
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .unwrap();
                return;
            }
        });

        std::env::set_var("HTTP_PROXY", format!("http://{proxy_addr}"));
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");
        std::env::remove_var("NO_PROXY");

        let agent = build_agent();
        let mut response = agent.get("http://example.test/ok").call().unwrap();
        let body = response.body_mut().read_to_string().unwrap();
        assert_eq!(body, "ok");

        std::env::remove_var("HTTP_PROXY");
        handle.join().unwrap();
    }

    #[test]
    fn accepts_plain_file_names() {
        for ok in ["rust", "javascript", "c_sharp", "grammar.wasm", "tags.scm", "manifest.json"] {
            assert!(safe_segment(ok).is_ok(), "{ok} should be accepted");
        }
    }

    #[test]
    fn rejects_traversal_and_separators() {
        // Parent-dir escapes, absolute paths, nested paths, separators, and the
        // empty / dot segments must all be refused before they reach a join.
        for bad in [
            "",
            ".",
            "..",
            "../etc",
            "../../.bashrc",
            "/etc/passwd",
            "a/b",
            "a\\b",
            "..\\..\\foo",
            "foo/",
            "./foo",
        ] {
            assert!(safe_segment(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn traversal_segment_does_not_escape_cache() {
        // The concrete property the guard protects: a rejected segment never gets
        // joined onto the cache root, so no write can land outside it.
        let cache = std::path::Path::new("/home/user/.cache/grove/grammars");
        let hostile = "../../../.bashrc";
        assert!(safe_segment(hostile).is_err());
        // Demonstrate why the guard matters: the unchecked join *would* escape.
        let joined = cache.join(hostile);
        assert!(!joined.starts_with(cache) || joined.components().any(|c| {
            matches!(c, std::path::Component::ParentDir)
        }));
    }
}
