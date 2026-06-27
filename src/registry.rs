//! The grammar registry — Phase 2 spine.
//!
//! Resolves a language id (or a file path) to its wasm grammar + tags query,
//! loaded at runtime. The local-directory stub here (`registry/<lang>/` with
//! `grammar.wasm`, `tags.scm`, `manifest.json`) stands in for the future hosted
//! WASM registry; nothing above this module knows grammars aren't compiled in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Language-specific node kinds that drive the structural niceties (parent
/// grouping, enclosing-function for callers, go-to-def). Data, not code — so a
/// new language is fully supported by dropping a manifest, with no recompile.
#[derive(Deserialize, Clone, Default)]
pub struct Profile {
    /// Node kinds that are function/method definitions.
    #[serde(default)]
    pub function_kinds: Vec<String>,
    /// Container node kinds paired with the field that names them, e.g.
    /// `["impl_item", "type"]` or `["class_definition", "name"]`.
    #[serde(default)]
    pub containers: Vec<(String, String)>,
    /// Node kinds whose text is a usable identifier (for go-to-def).
    #[serde(default)]
    pub identifier_kinds: Vec<String>,
    /// `@reference.*` capture suffixes that denote a call site, e.g. `["call"]`
    /// for Rust/JS or `["send", "call"]` for Ruby. Empty means the default
    /// (`"call"`), which keeps every existing manifest working unchanged.
    /// Kept for manifest backward compatibility; callers now includes all reference
    /// kinds (issue #33), but the field is preserved for future precision modes.
    #[serde(default)]
    #[allow(dead_code)]
    pub call_kinds: Vec<String>,
}

impl Profile {
    /// Is `kind` (a `@reference.*` capture suffix) a call site? Honors the
    /// manifest's `call_kinds`, falling back to the literal `"call"` so a
    /// grammar without the field behaves exactly as before.
    ///
    /// Kept for manifest backward compatibility; callers now includes all reference
    /// kinds (issue #33), but the method is preserved for future precision modes.
    #[allow(dead_code)]
    pub fn is_call_kind(&self, kind: &str) -> bool {
        if self.call_kinds.is_empty() {
            kind == "call"
        } else {
            self.call_kinds.iter().any(|k| k == kind)
        }
    }
}

/// Where a hosted artifact was ingested from — recorded for auditability.
/// grove serves the bytes; this attributes the source.
#[derive(Deserialize, Clone)]
pub struct Source {
    pub repo: String,
    #[serde(default)]
    pub rev: String,
}

#[derive(Deserialize, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub profile: Profile,
}

/// A resolved grammar artifact — enough to load and extract. Cheap to clone
/// (the heavy wasm bytes and query text are shared via `Arc`).
#[derive(Clone)]
pub struct Grammar {
    pub name: String,
    pub version: String,
    pub wasm: Arc<Vec<u8>>,
    pub tags_query: Arc<String>,
    /// Optional `locals.scm` (tree-sitter's standard `@local.scope` /
    /// `@local.definition` / `@local.reference` query). Drives scope-aware
    /// go-to-def. `None` when the registry dir ships no `locals.scm` — those
    /// languages keep the directory-wide name lookup.
    pub locals_query: Option<Arc<String>>,
    pub profile: Arc<Profile>,
}

impl Grammar {
    /// sha256 of the wasm bytes — the lockfile's integrity field.
    pub fn wasm_sha256(&self) -> String {
        sha256(self.wasm.as_slice())
    }
}

/// The OS-native global cache location for grammars:
/// `~/.cache/grove/grammars` (Linux), `~/Library/Caches/grove/grammars` (macOS),
/// `%LOCALAPPDATA%\grove\grammars` (Windows). Grammars are a cache — reconstructible
/// from the hosted registry and content-addressed by `grove.lock`.
pub fn cache_root() -> Option<PathBuf> {
    dirs::cache_dir().map(|c| c.join("grove").join("grammars"))
}

/// Dev fallback: the registry shipped in the source tree (only exists in a checkout).
fn dev_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/registry"))
}

/// A candidate registry root with where it came from, for diagnostics.
pub struct RootCandidate {
    pub source: &'static str,
    pub path: PathBuf,
    pub exists: bool,
}

/// The ordered search path, first existing wins. Surfaced by `grove registry`.
pub fn search_path() -> Vec<RootCandidate> {
    let mut out = Vec::new();
    let mut add = |source, path: PathBuf| {
        let exists = path.is_dir();
        out.push(RootCandidate { source, path, exists });
    };
    if let Ok(p) = std::env::var("GROVE_REGISTRY") {
        add("GROVE_REGISTRY", PathBuf::from(p));
    }
    if let Ok(cwd) = std::env::current_dir() {
        for dir in cwd.ancestors() {
            let cand = dir.join(".grove").join("grammars");
            if cand.is_dir() {
                add("project (.grove/grammars)", cand);
                break;
            }
        }
    }
    if let Some(c) = cache_root() {
        add("user cache", c);
    }
    add("dev (source tree)", dev_root());
    out
}

/// Resolve the registry root by precedence: an explicit `GROVE_REGISTRY` always
/// wins; otherwise the first existing of project-vendored → user cache → dev tree;
/// otherwise the canonical user-cache path (so errors point at the right home).
fn registry_root() -> PathBuf {
    if let Ok(p) = std::env::var("GROVE_REGISTRY") {
        return PathBuf::from(p);
    }
    for cand in search_path() {
        if cand.exists {
            return cand.path;
        }
    }
    cache_root().unwrap_or_else(dev_root)
}

/// Index of available languages, read once from the registry manifests.
struct Index {
    root: PathBuf,
    by_name: HashMap<String, Manifest>,
    by_ext: HashMap<String, String>, // extension -> language name
}

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        let root = registry_root();
        let mut by_name = HashMap::new();
        let mut by_ext = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten() {
                let mpath = e.path().join("manifest.json");
                let Ok(text) = std::fs::read_to_string(&mpath) else {
                    continue;
                };
                let Ok(m) = serde_json::from_str::<Manifest>(&text) else {
                    continue;
                };
                for ext in &m.extensions {
                    by_ext.insert(ext.clone(), m.name.clone());
                }
                by_name.insert(m.name.clone(), m);
            }
        }
        Index { root, by_name, by_ext }
    })
}

fn cache() -> &'static Mutex<HashMap<String, Grammar>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Grammar>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve a grammar by language id, reading (and caching) its artifacts.
pub fn resolve(lang: &str) -> Result<Grammar> {
    if let Some(g) = cache().lock().unwrap().get(lang) {
        return Ok(g.clone());
    }
    let idx = index();
    let manifest = idx.by_name.get(lang).with_context(|| {
        format!(
            "language `{lang}` is not in the registry ({}). Available: {}",
            idx.root.display(),
            available().join(", ")
        )
    })?;
    let dir = idx.root.join(lang);
    let wasm = std::fs::read(dir.join("grammar.wasm"))
        .with_context(|| format!("reading grammar.wasm for `{lang}`"))?;
    let tags = std::fs::read_to_string(dir.join("tags.scm"))
        .with_context(|| format!("reading tags.scm for `{lang}`"))?;
    // `locals.scm` is optional: present it only if the registry dir ships one.
    let locals = match std::fs::read_to_string(dir.join("locals.scm")) {
        Ok(s) => Some(Arc::new(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e).with_context(|| format!("reading locals.scm for `{lang}`")),
    };
    let grammar = Grammar {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        wasm: Arc::new(wasm),
        tags_query: Arc::new(tags),
        locals_query: locals,
        profile: Arc::new(manifest.profile.clone()),
    };
    cache()
        .lock()
        .unwrap()
        .insert(lang.to_string(), grammar.clone());
    Ok(grammar)
}

/// The language id for a file path, by extension.
pub fn lang_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    index().by_ext.get(ext).map(String::as_str)
}

/// Resolve the grammar for a file path.
pub fn for_path(path: &Path) -> Result<Grammar> {
    let lang = lang_for_path(path).with_context(|| {
        format!(
            "no registered grammar for `{}` (extensions: {})",
            path.display(),
            extensions().join(", ")
        )
    })?;
    resolve(lang)
}

/// True if the path has a registered extension.
pub fn is_source(path: &Path) -> bool {
    lang_for_path(path).is_some()
}

/// The registry root actually in use this run.
pub fn root() -> &'static Path {
    &index().root
}

/// All available language names.
pub fn available() -> Vec<String> {
    let mut v: Vec<String> = index().by_name.keys().cloned().collect();
    v.sort();
    v
}

/// All registered extensions.
pub fn extensions() -> Vec<String> {
    let mut v: Vec<String> = index().by_ext.keys().cloned().collect();
    v.sort();
    v
}

/// The manifests, for `grove languages`.
pub fn manifests() -> Vec<Manifest> {
    let mut v: Vec<Manifest> = index().by_name.values().cloned().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// The canonical content hash for grove artifacts: `sha256:<hex>`. The single
/// source of truth — the lockfile, the index, and `grove fetch`'s verification
/// all go through here, so the format can never drift between producer and
/// verifier.
pub fn sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{:x}", h.finalize())
}

/// Build the hosted catalog (`index.json`) for a registry directory: per
/// language, its version, provenance, and a content hash of every served file.
/// This is what registry CI runs to publish; `grove fetch` consumes it.
///
/// When `release_base` is set, `grammar.wasm` is recorded as a release **asset**
/// (`<lang>.wasm`) served from that base, so the heavy binaries live in GitHub
/// Releases and the repo stays small. `tags.scm`/`manifest.json` are always
/// served from the repo alongside the catalog.
pub fn build_index(root: &Path, release_base: Option<&str>) -> Result<serde_json::Value> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .with_context(|| format!("reading registry {}", root.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut grammars = Vec::new();
    for dir in dirs {
        let mpath = dir.join("manifest.json");
        if !mpath.exists() {
            continue;
        }
        let m: Manifest = serde_json::from_str(&std::fs::read_to_string(&mpath)?)
            .with_context(|| format!("parsing {}", mpath.display()))?;
        let mut files = serde_json::Map::new();
        for fname in ["grammar.wasm", "tags.scm", "manifest.json"] {
            let bytes = std::fs::read(dir.join(fname))
                .with_context(|| format!("hashing {}/{fname}", m.name))?;
            let mut fref = serde_json::json!({ "sha256": sha256(&bytes) });
            if fname == "grammar.wasm" && release_base.is_some() {
                fref["asset"] = serde_json::json!(format!("{}.wasm", m.name));
            }
            files.insert(fname.into(), fref);
        }
        // `locals.scm` is optional (scope-aware resolution, ADR 0001): record it
        // only when the grammar dir ships one, so `fetch` pulls it for languages
        // that have it without breaking those that don't.
        let locals = dir.join("locals.scm");
        if locals.exists() {
            let bytes = std::fs::read(&locals)
                .with_context(|| format!("hashing {}/locals.scm", m.name))?;
            files.insert("locals.scm".into(), serde_json::json!({ "sha256": sha256(&bytes) }));
        }
        let mut entry = serde_json::json!({
            "name": m.name,
            "version": m.version,
            "extensions": m.extensions,
            "files": files,
        });
        if let Some(src) = &m.source {
            entry["source"] = serde_json::json!({ "repo": src.repo, "rev": src.rev });
        }
        grammars.push(entry);
    }
    let mut catalog = serde_json::json!({ "schema": 2, "grammars": grammars });
    if let Some(base) = release_base {
        catalog["release_base"] = serde_json::json!(base);
    }
    Ok(catalog)
}

/// Build the registry catalog and write it as JSON. `dir` defaults to the
/// resolved registry root; `output` defaults to `<dir>/index.json`. Returns the
/// path written and the grammar count, for the caller to report. Keeps path
/// resolution and file I/O out of `main`, alongside its sibling verbs.
pub fn write_index(
    dir: Option<PathBuf>,
    output: Option<PathBuf>,
    release_base: Option<&str>,
) -> Result<(PathBuf, usize)> {
    let dir = dir.unwrap_or_else(|| root().to_path_buf());
    let out = output.unwrap_or_else(|| dir.join("index.json"));
    let catalog = build_index(&dir, release_base)?;
    std::fs::write(&out, format!("{}\n", serde_json::to_string_pretty(&catalog)?))
        .with_context(|| format!("writing {}", out.display()))?;
    let n = catalog["grammars"].as_array().map_or(0, |a| a.len());
    Ok((out, n))
}

/// Write a lockfile pinning every registry grammar's version + wasm hash.
pub fn write_lock(path: &Path) -> Result<usize> {
    write_lock_for(&available(), path)
}

/// Write a lockfile pinning the given languages' version + wasm hash.
/// Deterministic (sorted) so it is diff-friendly and commit-able.
pub fn write_lock_for(langs: &[String], path: &Path) -> Result<usize> {
    let mut names: Vec<String> = langs.to_vec();
    names.sort();
    names.dedup();
    let mut grammars = Vec::new();
    for name in &names {
        let g = resolve(name)?;
        grammars.push(serde_json::json!({
            "name": g.name,
            "version": g.version,
            "wasm": g.wasm_sha256(),
        }));
    }
    let doc = serde_json::json!({ "version": 1, "grammars": grammars });
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&doc)?))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(grammars.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_call_kinds_defaults_to_literal_call() {
        // A manifest with no `call_kinds` (every grammar shipped before #10)
        // must keep treating `@reference.call` as the call site.
        let p = Profile::default();
        assert!(p.is_call_kind("call"));
        assert!(!p.is_call_kind("send"));
        assert!(!p.is_call_kind("invocation"));
    }

    #[test]
    fn call_kinds_drives_the_filter() {
        // A Ruby/Elixir-style grammar declares its own call suffixes; the
        // literal "call" is no longer special once the field is set.
        let p = Profile { call_kinds: vec!["send".into(), "invocation".into()], ..Default::default() };
        assert!(p.is_call_kind("send"));
        assert!(p.is_call_kind("invocation"));
        assert!(!p.is_call_kind("call"));
    }

    /// A minimal one-grammar registry dir that `build_index` can hash.
    fn toy_registry(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("grove_index_test_{}_{tag}", std::process::id()));
        let lang = dir.join("toy");
        std::fs::create_dir_all(&lang).unwrap();
        std::fs::write(lang.join("grammar.wasm"), b"\0asm-toy-bytes").unwrap();
        std::fs::write(lang.join("tags.scm"), "; tags").unwrap();
        std::fs::write(lang.join("manifest.json"), r#"{"name":"toy","version":"1.2.3","extensions":["toy"]}"#).unwrap();
        dir
    }

    #[test]
    fn write_index_writes_catalog_and_returns_count() {
        let dir = toy_registry("explicit");
        let out = dir.join("custom.json");

        let (written, n) = write_index(Some(dir.clone()), Some(out.clone()), None).unwrap();
        assert_eq!(written, out, "returns the path it wrote");
        assert_eq!(n, 1, "one grammar in the toy registry");

        let catalog: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(catalog["schema"], serde_json::json!(2));
        assert_eq!(catalog["grammars"][0]["name"], serde_json::json!("toy"));
        assert!(catalog["grammars"][0]["files"]["grammar.wasm"]["sha256"]
            .as_str().unwrap().starts_with("sha256:"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_index_records_locals_scm_only_when_present() {
        // Absent by default: the toy registry ships no locals.scm.
        let dir = toy_registry("no_locals");
        let catalog = build_index(&dir, None).unwrap();
        assert!(
            catalog["grammars"][0]["files"].get("locals.scm").is_none(),
            "no locals.scm in the dir → none in the catalog"
        );
        std::fs::remove_dir_all(&dir).ok();

        // Present: dropping a locals.scm in the dir surfaces it (hashed) in the
        // catalog, so `fetch` will pull it (it's repo-served, no asset).
        let dir = toy_registry("with_locals");
        std::fs::write(dir.join("toy").join("locals.scm"), "(identifier) @local.reference\n").unwrap();
        let catalog = build_index(&dir, None).unwrap();
        let entry = &catalog["grammars"][0]["files"]["locals.scm"];
        assert!(
            entry["sha256"].as_str().unwrap().starts_with("sha256:"),
            "locals.scm hashed into the catalog: {entry}"
        );
        assert!(entry.get("asset").is_none(), "locals.scm is repo-served, not a release asset");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sha256_has_canonical_format() {
        // Known-answer vector for the empty input, pinning the `sha256:` prefix
        // and lowercase hex — the format every producer and verifier shares.
        assert_eq!(
            sha256(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn wasm_sha256_delegates_to_sha256() {
        let bytes = b"\0asm-some-grammar".to_vec();
        let g = Grammar {
            name: "toy".into(),
            version: "0.0.0".into(),
            wasm: Arc::new(bytes.clone()),
            tags_query: Arc::new(String::new()),
            locals_query: None,
            profile: Arc::new(Profile::default()),
        };
        // The lockfile field and the index/fetch helper must agree byte-for-byte.
        assert_eq!(g.wasm_sha256(), sha256(&bytes));
    }

    #[test]
    fn write_index_defaults_output_to_dir_index_json() {
        let dir = toy_registry("default");
        let (written, _) = write_index(Some(dir.clone()), None, None).unwrap();
        assert_eq!(written, dir.join("index.json"), "default output is <dir>/index.json");
        assert!(written.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_index_records_release_asset_when_release_base_set() {
        let dir = toy_registry("asset");
        let catalog = build_index(&dir, Some("https://example.test/releases/v1")).unwrap();
        assert_eq!(catalog["release_base"], serde_json::json!("https://example.test/releases/v1"));
        let wasm = &catalog["grammars"][0]["files"]["grammar.wasm"];
        assert_eq!(wasm["asset"], serde_json::json!("toy.wasm"), "wasm routed to a release asset");
        // tags.scm / manifest.json stay in the repo — no asset field.
        assert!(catalog["grammars"][0]["files"]["tags.scm"].get("asset").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_index_errors_on_missing_root() {
        let missing = std::env::temp_dir().join(format!("grove_no_such_registry_{}", std::process::id()));
        let err = build_index(&missing, None).unwrap_err();
        assert!(err.to_string().contains("reading registry"), "got: {err}");
    }

    // ---- path/extension resolution (against whichever registry root wins) ----

    #[test]
    fn lang_for_path_maps_known_extensions() {
        assert_eq!(lang_for_path(Path::new("a/b/foo.rs")), Some("rust"));
        assert_eq!(lang_for_path(Path::new("foo.py")), Some("python"));
        assert_eq!(lang_for_path(Path::new("foo.js")), Some("javascript"));
        assert_eq!(lang_for_path(Path::new("foo.unknownext")), None);
        assert_eq!(lang_for_path(Path::new("no_extension")), None);
    }

    #[test]
    fn is_source_follows_extension() {
        assert!(is_source(Path::new("lib.rs")));
        assert!(!is_source(Path::new("README.unknownext")));
        assert!(!is_source(Path::new("Makefile")));
    }

    #[test]
    fn for_path_errors_on_unregistered_extension() {
        let err = for_path(Path::new("notes.unknownext")).err().expect("should error");
        assert!(err.to_string().contains("no registered grammar"), "got: {err}");
    }

    #[test]
    fn resolve_errors_on_unknown_language() {
        let err = resolve("definitely-not-a-language").err().expect("should error");
        assert!(err.to_string().contains("not in the registry"), "got: {err}");
    }

    #[test]
    fn resolve_caches_and_loads_a_real_grammar() {
        let a = resolve("rust").unwrap();
        let b = resolve("rust").unwrap();
        assert_eq!(a.name, "rust");
        assert!(!a.wasm.is_empty());
        // Second resolve returns the cached Arc — same allocation.
        assert!(Arc::ptr_eq(&a.wasm, &b.wasm));
    }

    #[test]
    fn available_and_extensions_are_sorted_and_include_the_dev_stub() {
        let langs = available();
        assert!(langs.contains(&"rust".to_string()));
        assert!(langs.contains(&"python".to_string()));
        assert!(langs.windows(2).all(|w| w[0] <= w[1]), "available() must be sorted");
        let exts = extensions();
        assert!(exts.contains(&"rs".to_string()));
        assert!(exts.windows(2).all(|w| w[0] <= w[1]), "extensions() must be sorted");
    }

    #[test]
    fn manifests_are_sorted_and_carry_versions() {
        let ms = manifests();
        assert!(!ms.is_empty());
        assert!(ms.windows(2).all(|w| w[0].name <= w[1].name), "manifests sorted by name");
        let rust = ms.iter().find(|m| m.name == "rust").expect("rust manifest");
        assert!(!rust.version.is_empty());
        assert!(rust.extensions.contains(&"rs".to_string()));
    }

    #[test]
    fn search_path_is_ordered_and_root_exists() {
        let path = search_path();
        assert!(!path.is_empty());
        assert!(path.iter().any(|c| c.source == "dev (source tree)"), "dev candidate always listed");
        assert!(root().is_dir(), "the resolved root must exist");
    }

    #[test]
    fn write_lock_for_pins_versions_and_hashes() {
        let out = std::env::temp_dir().join(format!("grove_lock_test_{}.lock", std::process::id()));
        let n = write_lock_for(&["rust".into(), "rust".into()], &out).unwrap();
        assert_eq!(n, 1, "duplicate langs are deduped");
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(doc["version"], serde_json::json!(1));
        assert_eq!(doc["grammars"][0]["name"], serde_json::json!("rust"));
        assert!(doc["grammars"][0]["wasm"].as_str().unwrap().starts_with("sha256:"));
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn manifest_deserializes_call_kinds_from_profile() {
        // End-to-end of #10: a manifest's profile.call_kinds reaches the Profile.
        let json = r#"{
            "name": "ruby", "version": "1.0.0", "extensions": ["rb"],
            "profile": { "function_kinds": ["method"], "call_kinds": ["call", "send"] }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.profile.call_kinds, vec!["call", "send"]);
        assert!(m.profile.is_call_kind("send"));
    }
}
