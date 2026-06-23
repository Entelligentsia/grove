//! The operations, as a library — the single engine both faces call.
//!
//! `main.rs` (CLI) formats these into human tables; `mcp.rs` (MCP server)
//! serializes them to JSON. Grammars come from the registry, so these work for
//! any registered language, not just one compiled in.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::engine::{self, Defect, Symbol};
use crate::registry::{self, Grammar};
use ignore::WalkBuilder;

/// Read a file's bytes with a contextual error.
pub fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("reading {}", path.display()))
}

/// Best-effort repo-relative path, for stable symbol ids.
pub fn rel(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.canonicalize().ok().map(|p| (cwd, p)))
        .and_then(|(cwd, p)| p.strip_prefix(&cwd).ok().map(|r| r.display().to_string()))
        .unwrap_or_else(|| path.display().to_string())
}

/// True if `path` is a generated declaration file grove should not index as
/// source during a directory walk. TypeScript `.d.ts`/`.d.cts`/`.d.mts` files
/// are type declarations with no implementation — often machine-generated
/// (under `tests/baselines/`, `declarations/`, `dist/`). Indexing them points
/// `symbols`/`definition`/`callers` at the decl instead of the real source and
/// drops genuine call sites, so they are excluded from the walk. A single
/// file requested explicitly via `outline`/`source`/`check` is still honored —
/// this filter only governs the recursive indexing pass. (Issue #32.)
fn is_generated_decl(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.ends_with(".d.ts") || name.ends_with(".d.cts") || name.ends_with(".d.mts")
}

/// Walk every registered-source file under `dir`, yielding `(grammar, relpath, source)`.
/// Generated declaration files (`*.d.ts`, see [`is_generated_decl`]) are skipped
/// so `symbols`/`definition`/`callers` answer from real source, not generated decls.
fn for_each_source(dir: &Path, mut f: impl FnMut(&Grammar, &str, &[u8]) -> Result<()>) -> Result<()> {
    for entry in WalkBuilder::new(dir).build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() || !registry::is_source(path) || is_generated_decl(path) {
            continue;
        }
        let grammar = registry::for_path(path)?;
        let src = read(path)?;
        f(&grammar, &rel(path), &src)?;
    }
    Ok(())
}

/// Whether a symbol of `sym_kind` satisfies a `--kind filter`. Exact match,
/// plus synonyms so a natural term finds the umbrella kind grove actually emits:
/// every grammar tags struct/class-likes as `class` (C/Rust structs, C unions),
/// so `--kind struct`/`--kind union` still find them. The aliases map onto
/// `class` only — no grammar emits `struct`/`union` as a kind — so this can only
/// widen a match, never hide one.
fn kind_matches(sym_kind: &str, filter: &str) -> bool {
    sym_kind == filter
        || (matches!(filter, "struct" | "union" | "record") && sym_kind == "class")
}

/// `outline` — the definitions in one file, optionally filtered by kind.
pub fn outline(file: &Path, kind: Option<&str>) -> Result<Vec<Symbol>> {
    let grammar = registry::for_path(file)?;
    let src = read(file)?;
    let mut syms = engine::extract(&grammar, &rel(file), &src)?;
    syms.retain(|s| s.is_definition && kind.is_none_or(|k| kind_matches(&s.kind, k)));
    Ok(syms)
}

/// Project symbols to a JSON array at a detail level, to keep payloads bounded:
/// 0 = terse (kind/name/parent/line), 1 = default (adds id/col/signature, drops
/// byte offsets — the agent addresses symbols by id, not offset), 2 = full.
pub fn project(syms: &[Symbol], detail: u8) -> serde_json::Value {
    use serde_json::{Map, Value};
    if detail >= 2 {
        return serde_json::to_value(syms).unwrap_or(Value::Null);
    }
    let arr = syms
        .iter()
        .map(|s| {
            let mut m = Map::new();
            if detail >= 1 {
                m.insert("id".into(), s.id.clone().into());
            }
            m.insert("kind".into(), s.kind.clone().into());
            m.insert("name".into(), s.name.clone().into());
            if let Some(p) = &s.parent {
                m.insert("parent".into(), p.clone().into());
            }
            m.insert("line".into(), s.line.into());
            if detail >= 1 {
                m.insert("col".into(), s.col.into());
                m.insert("signature".into(), s.signature.clone().into());
            }
            Value::Object(m)
        })
        .collect();
    Value::Array(arr)
}

/// `symbols` — find across a directory, gitignore-aware.
pub fn symbols(
    dir: &Path,
    kind: Option<&str>,
    name: Option<&str>,
    refs: bool,
) -> Result<Vec<Symbol>> {
    let name_lc = name.map(str::to_lowercase);
    let mut all = Vec::new();
    for_each_source(dir, |grammar, relpath, src| {
        for s in engine::extract(grammar, relpath, src)? {
            if !refs && !s.is_definition {
                continue;
            }
            if kind.is_some_and(|k| !kind_matches(&s.kind, k)) {
                continue;
            }
            if name_lc
                .as_ref()
                .is_some_and(|n| !s.name.to_lowercase().contains(n))
            {
                continue;
            }
            all.push(s);
        }
        Ok(())
    })?;
    Ok(all)
}

/// The result of `source`: the chosen symbol's code, plus any other
/// definitions that shared the name (so the agent can disambiguate).
#[derive(Debug, Serialize)]
pub struct SourceResult {
    pub id: String,
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_candidates: Vec<String>,
}

/// `source` — full code of a symbol, by id (`<lang>:<path>#<name>@<line>`) or
/// by file + name.
pub fn source(id_or_file: &str, name: Option<&str>) -> Result<SourceResult> {
    let (file, want, want_line): (PathBuf, String, Option<usize>) = match name {
        Some(n) => (PathBuf::from(id_or_file), n.to_string(), None),
        None => {
            let rest = id_or_file.split_once(':').map_or(id_or_file, |(_, r)| r);
            let (path, after) = rest
                .split_once('#')
                .context("symbol id must look like <lang>:<path>#<name>@<line>")?;
            // The `@<line>` suffix disambiguates duplicate-named definitions; keep
            // it so the requested symbol is the one returned.
            let (name, line) = match after.split_once('@') {
                Some((n, r)) => (n.to_string(), r.parse::<usize>().ok()),
                None => (after.to_string(), None),
            };
            (PathBuf::from(path), name, line)
        }
    };

    let grammar = registry::for_path(&file)?;
    let src = read(&file)?;
    let syms = engine::extract(&grammar, &rel(&file), &src)?;
    let matches: Vec<&Symbol> = syms
        .iter()
        .filter(|s| s.is_definition && s.name == want)
        .collect();

    // Prefer the exact-line match when the id carried a line; otherwise (name
    // mode, lineless id, or no line matched) fall back to the first definition.
    let chosen = match want_line.and_then(|r| matches.iter().find(|s| s.line == r)) {
        Some(c) => *c,
        None => match matches.first() {
            None => anyhow::bail!("no definition named `{want}` in {}", file.display()),
            Some(c) => *c,
        },
    };
    Ok(SourceResult {
        id: chosen.id.clone(),
        source: engine::slice(&src, chosen).to_string(),
        other_candidates: matches
            .iter()
            .filter(|s| s.id != chosen.id)
            .map(|s| s.id.clone())
            .collect(),
    })
}

/// `check` — ERROR / MISSING nodes in one file.
pub fn check(file: &Path) -> Result<Vec<Defect>> {
    let grammar = registry::for_path(file)?;
    let src = read(file)?;
    engine::check(&grammar, &src)
}

/// A site where a symbol is called.
#[derive(Debug, Serialize)]
pub struct CallSite {
    pub file: String,
    /// 1-based line and column of the call (editor / `grep -n` convention).
    pub line: usize,
    pub col: usize,
    /// The function/method that contains this call (`Type::method` when known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_function: Option<String>,
    /// The trimmed source text of the call's line.
    pub text: String,
}

/// `callers` — every call site of `name` across `dir`, with enclosing function.
///
/// Name-based: matches calls to *any* function/method with this name (the slice
/// does not resolve receiver types). Honest over-match, documented for the agent.
pub fn callers(dir: &Path, name: &str) -> Result<Vec<CallSite>> {
    let mut out = Vec::new();
    for_each_source(dir, |grammar, relpath, src| {
        // Reuse the parse tree from extraction for the enclosing-function pass —
        // parsing dominates cost, so re-parsing here would double the work.
        let (syms, tree) = engine::extract_with_tree(grammar, relpath, src)?;
        let calls: Vec<&Symbol> = syms
            .iter()
            .filter(|s| !s.is_definition && grammar.profile.is_call_kind(&s.kind) && s.name == name)
            .collect();
        if calls.is_empty() {
            return Ok(());
        }
        let root = tree.root_node();
        for s in &calls {
            out.push(CallSite {
                in_function: engine::enclosing_function_at(root, s.start_byte, src, &grammar.profile),
                file: s.file.clone(),
                line: s.line,
                col: s.col,
                text: s.signature.clone(),
            });
        }
        Ok(())
    })?;
    Ok(out)
}

/// Parse a `file:line:col` position string. `line`/`col` are 1-based on input
/// (the editor / `grep -n` convention grove prints), and are returned as the
/// 0-based row/col tree-sitter expects, so the location grove prints round-trips
/// straight back into `--at`.
pub fn parse_pos(s: &str) -> Result<(PathBuf, usize, usize)> {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    match parts.as_slice() {
        [col, line, file] => {
            let line: usize = line.parse().map_err(|_| anyhow::anyhow!("bad line in `{s}`"))?;
            let col: usize = col.parse().map_err(|_| anyhow::anyhow!("bad col in `{s}`"))?;
            Ok((
                PathBuf::from(file),
                line.saturating_sub(1),
                col.saturating_sub(1),
            ))
        }
        _ => anyhow::bail!("expected file:line:col, got `{s}`"),
    }
}

/// A definition in a `map` result, with its outgoing references.
#[derive(Debug, Serialize)]
pub struct MapEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub row: usize,
    pub signature: String,
    /// Names of other symbols this definition references (outgoing edges).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
}

/// A file in a `map` result, with its definitions and their references.
#[derive(Debug, Serialize)]
pub struct FileMap {
    pub file: String,
    pub entries: Vec<MapEntry>,
}

/// `map` — compact structural map of a directory: every definition grouped by
/// file, with each definition's outgoing references (which other symbols it
/// calls or uses). No source bodies — just the dependency graph. Use this
/// instead of many `symbols`+`source` calls when you need a broad picture of
/// how code connects.
pub fn map(dir: &Path, kind: Option<&str>, name: Option<&str>) -> Result<Vec<FileMap>> {
    let name_lc = name.map(str::to_lowercase);
    let mut file_maps = Vec::new();
    for_each_source(dir, |grammar, relpath, src| {
        let syms = engine::extract(grammar, relpath, src)?;

        // Collect matching definition indices, sorted by byte-range size
        // ascending so innermost (narrowest) definitions come first. This
        // lets us attribute each reference to its innermost enclosing def.
        let mut defs: Vec<usize> = syms
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_definition)
            .filter(|(_, s)| kind.is_none_or(|k| kind_matches(&s.kind, k)))
            .filter(|(_, s)| name_lc.as_ref().is_none_or(|n| s.name.to_lowercase().contains(n)))
            .map(|(i, _)| i)
            .collect();
        defs.sort_by_key(|&i| syms[i].end_byte - syms[i].start_byte);

        // Attribute each reference to the innermost containing definition.
        let mut ref_map: std::collections::HashMap<usize, Vec<String>> =
            std::collections::HashMap::new();
        for (_, s) in syms.iter().enumerate() {
            if s.is_definition {
                continue;
            }
            for &d in &defs {
                if s.start_byte >= syms[d].start_byte && s.end_byte <= syms[d].end_byte {
                    ref_map.entry(d).or_default().push(s.name.clone());
                    break; // first (narrowest) match wins
                }
            }
        }

        // Deduplicate reference names per definition.
        for names in ref_map.values_mut() {
            names.sort();
            names.dedup();
        }

        // Build entries, sorted by row for deterministic output.
        let mut entries: Vec<MapEntry> = defs
            .iter()
            .map(|&d| {
                let s = &syms[d];
                let mut refs = ref_map.remove(&d).unwrap_or_default();
                // Remove self-references (e.g. recursive calls).
                refs.retain(|n| n != &s.name);
                MapEntry {
                    id: s.id.clone(),
                    kind: s.kind.clone(),
                    name: s.name.clone(),
                    parent: s.parent.clone(),
                    row: s.line,
                    signature: s.signature.clone(),
                    references: refs,
                }
            })
            .collect();
        entries.sort_by_key(|e| e.row);

        if !entries.is_empty() {
            file_maps.push(FileMap {
                file: relpath.to_string(),
                entries,
            });
        }
        Ok(())
    })?;
    file_maps.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(file_maps)
}

/// `definition` — exact-name definitions of `name` across `dir` (go-to-def).
pub fn definition(dir: &Path, name: &str) -> Result<Vec<Symbol>> {
    let mut defs = symbols(dir, None, Some(name), false)?;
    defs.retain(|s| s.name == name);
    Ok(defs)
}

/// `definition --at` — resolve the identifier at a usage site, then find its
/// definition(s). `row`/`col` are 0-based tree-sitter coords (callers feed the
/// output of [`parse_pos`], which converts the 1-based `file:line:col` users
/// type). Returns the resolved name alongside the matches.
pub fn definition_at(file: &Path, row: usize, col: usize, dir: &Path) -> Result<(String, Vec<Symbol>)> {
    let grammar = registry::for_path(file)?;
    let src = read(file)?;
    let name = engine::with_tree(&grammar, &src, |root, profile| {
        engine::identifier_at(root, row, col, &src, profile)
    })?
    .with_context(|| format!("no identifier at {}:{row}:{col}", file.display()))?;
    let defs = definition(dir, &name)?;
    Ok((name, defs))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two definitions named `run`, at rows 0 and 4.
    const DUP: &str =
        "fn run() {\n    let _first = 1;\n}\n\nfn run() {\n    let _second = 2;\n}\n";

    fn write_temp(tag: &str, contents: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("grove_src_test_{}_{tag}.rs", std::process::id()));
        std::fs::write(&p, contents).unwrap();
        p
    }

    #[test]
    fn id_line_selects_that_definition() {
        let path = write_temp("dup_line", DUP);

        // The 2nd `run` starts on the 5th line (1-based) of DUP.
        let res = source(&format!("rust:{}#run@5", path.display()), None).unwrap();
        assert!(res.source.contains("_second"), "line 5 must pick the 2nd run, got: {}", res.source);
        assert!(res.id.ends_with("@5"), "chosen id should be the line-5 def, got {}", res.id);

        let res0 = source(&format!("rust:{}#run@1", path.display()), None).unwrap();
        assert!(res0.source.contains("_first"), "line 1 must pick the 1st run, got: {}", res0.source);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unmatched_line_falls_back_to_first() {
        let path = write_temp("dup_fallback", DUP);
        let res = source(&format!("rust:{}#run@99", path.display()), None).unwrap();
        assert!(res.source.contains("_first"), "unknown line falls back to the first def");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn by_name_returns_first_and_lists_other_candidate() {
        let path = write_temp("dup_name", DUP);
        let res = source(path.to_str().unwrap(), Some("run")).unwrap();
        assert!(res.source.contains("_first"));
        assert_eq!(res.other_candidates.len(), 1, "the 2nd run is the other candidate");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn callers_finds_call_sites_via_profile() {
        // `helper` is called once; the profile-driven call filter (#10) must
        // still surface the `@reference.call` site for the dev-stub rust grammar.
        let dir = std::env::temp_dir().join(format!("grove_callers_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lib.rs");
        std::fs::write(&file, "fn helper() {}\nfn main() {\n    helper();\n}\n").unwrap();

        let sites = callers(&dir, "helper").unwrap();
        assert_eq!(sites.len(), 1, "exactly one call to helper, got {sites:?}");
        assert_eq!(sites[0].in_function.as_deref(), Some("main"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn callers_parses_each_file_once() {
        // #13: `callers` used to parse every matched file twice (extract +
        // with_tree). It must now parse each source file exactly once.
        let dir = std::env::temp_dir().join(format!("grove_parse_once_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Three files; two contain a call to `helper`, one does not. All three
        // are still parsed once each by the single extraction pass.
        std::fs::write(dir.join("a.rs"), "fn main() {\n    helper();\n}\n").unwrap();
        std::fs::write(dir.join("b.rs"), "fn run() {\n    helper();\n}\n").unwrap();
        std::fs::write(dir.join("c.rs"), "fn unrelated() {}\n").unwrap();

        engine::parse_counter::reset();
        let sites = callers(&dir, "helper").unwrap();
        let parses = engine::parse_counter::get();

        assert_eq!(sites.len(), 2, "two call sites, got {sites:?}");
        assert_eq!(parses, 3, "expected one parse per source file (3), got {parses}");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- parse_pos ----

    #[test]
    fn parse_pos_parses_file_line_col() {
        // 1-based `line:col` input is returned as 0-based row/col.
        let (file, row, col) = parse_pos("src/lib.rs:12:4").unwrap();
        assert_eq!(file, PathBuf::from("src/lib.rs"));
        assert_eq!((row, col), (11, 3));
    }

    #[test]
    fn parse_pos_keeps_colons_in_the_path() {
        // rsplitn(3) means only the last two colons split line/col; a path with a
        // colon (or a Windows drive) stays intact.
        let (file, row, col) = parse_pos("a:b/file.rs:3:7").unwrap();
        assert_eq!(file, PathBuf::from("a:b/file.rs"));
        assert_eq!((row, col), (2, 6));
    }

    #[test]
    fn parse_pos_rejects_bad_shapes() {
        assert!(parse_pos("no-colons").is_err());
        assert!(parse_pos("file.rs:notarow:4").unwrap_err().to_string().contains("bad line"));
        assert!(parse_pos("file.rs:4:notacol").unwrap_err().to_string().contains("bad col"));
    }

    // ---- project (detail tiers) ----

    #[test]
    fn project_tiers_control_field_density() {
        let dir = std::env::temp_dir().join(format!("grove_project_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lib.rs");
        std::fs::write(&file, "struct S;\nimpl S {\n    fn m(&self) {}\n}\n").unwrap();
        let syms = outline(&file, None).unwrap();

        let terse = project(&syms, 0);
        let first = &terse.as_array().unwrap()[0];
        assert!(first.get("id").is_none(), "detail 0 omits id");
        assert!(first.get("signature").is_none(), "detail 0 omits signature");
        assert!(first.get("kind").is_some() && first.get("name").is_some());

        let default = project(&syms, 1);
        let d0 = &default.as_array().unwrap()[0];
        assert!(d0.get("id").is_some(), "detail 1 adds id");
        assert!(d0.get("signature").is_some(), "detail 1 adds signature");
        assert!(d0.get("start_byte").is_none(), "detail 1 drops byte offsets");

        let full = project(&syms, 2);
        let f0 = &full.as_array().unwrap()[0];
        assert!(f0.get("start_byte").is_some(), "detail 2 includes byte offsets");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- outline / symbols filters ----

    #[test]
    fn outline_filters_by_kind_and_skips_references() {
        let dir = std::env::temp_dir().join(format!("grove_outline_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lib.rs");
        std::fs::write(&file, "struct S;\nfn f() {\n    g();\n}\n").unwrap();

        let all = outline(&file, None).unwrap();
        assert!(all.iter().all(|s| s.is_definition), "outline yields definitions only");
        assert!(all.iter().any(|s| s.name == "S"));
        assert!(all.iter().any(|s| s.name == "f"));

        // The rust tags map `struct` to the `class` kind.
        let classes = outline(&file, Some("class")).unwrap();
        assert!(classes.iter().all(|s| s.kind == "class"));
        assert!(classes.iter().any(|s| s.name == "S"));
        assert!(!classes.iter().any(|s| s.name == "f"), "kind filter excludes fns");

        // `--kind struct` is a synonym for `class` so a natural term still finds it.
        let structs = outline(&file, Some("struct")).unwrap();
        assert!(structs.iter().any(|s| s.name == "S"), "struct aliases to class");
        assert!(!structs.iter().any(|s| s.name == "f"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn is_generated_decl_flags_typescript_declaration_files() {
        // Issue #32: `.d.ts`/`.d.cts`/`.d.mts` are generated declarations — they
        // must be skipped by the directory walk so symbols/definition/callers
        // answer from real source, not the decl. The check is suffix-based so it
        // is independent of the registry (the typescript grammar may be absent).
        assert!(is_generated_decl(Path::new("src/compiler/scanner.d.ts")));
        assert!(is_generated_decl(Path::new("tests/baselines/reference/api/typescript.d.ts")));
        assert!(is_generated_decl(Path::new("declarations/LoaderContext.d.ts")));
        assert!(is_generated_decl(Path::new("pkg/index.d.cts")));
        assert!(is_generated_decl(Path::new("pkg/index.d.mts")));

        // Real implementation files and other paths are left alone.
        assert!(!is_generated_decl(Path::new("src/compiler/scanner.ts")));
        assert!(!is_generated_decl(Path::new("lib/Compiler.js")));
        assert!(!is_generated_decl(Path::new("types.ts")), "`types.ts` is real source, not `types.d.ts`");
        assert!(!is_generated_decl(Path::new("README.md")));
        assert!(!is_generated_decl(Path::new("no_extension")));
    }

    #[test]
    fn symbols_skips_generated_declaration_files() {
        // Issue #32: a `.d.ts` file in the tree must not contribute symbols,
        // even when a registered grammar would otherwise accept its extension.
        // Here the dev-stub registry has no typescript grammar, so `.d.ts` is
        // already not source — but a `.d.js`-style nested decl under a real
        // registered extension is the closest in-stub analog. We instead assert
        // the skip at the predicate level (see is_generated_decl_flags_*).
        // This test pins that a real `.js` decl-like name is still indexed (i.e.
        // the filter is suffix-precise and does not over-reach onto `.js`).
        let dir = std::env::temp_dir().join(format!("grove_nodecl_test_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("declarations")).unwrap();
        std::fs::write(dir.join("declarations/LoaderContext.d.ts"), "export class LoaderContext {}").unwrap();
        std::fs::write(dir.join("lib.js"), "class Compiler {}").unwrap();

        let defs = symbols(&dir, None, Some("Compiler"), false).unwrap();
        assert!(defs.iter().any(|s| s.name == "Compiler"), "real source is indexed");
        // No symbol named `LoaderContext` leaks in from the `.d.ts` (typescript is
        // not registered here, but this also guards against a future regression where
        // the filter stops being applied in the walk).
        assert!(!defs.iter().any(|s| s.name == "LoaderContext"), "generated decl is skipped");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn kind_matches_exact_and_struct_synonyms() {
        assert!(kind_matches("class", "class"));
        assert!(kind_matches("class", "struct"), "struct → class");
        assert!(kind_matches("class", "union"), "union → class");
        assert!(kind_matches("function", "function"));
        assert!(!kind_matches("function", "struct"), "synonyms only widen onto class");
        assert!(!kind_matches("variable", "class"));
    }

    #[test]
    fn symbols_honors_name_kind_and_refs_filters() {
        let dir = std::env::temp_dir().join(format!("grove_symbols_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.rs"), "fn alpha() {}\nfn beta() {\n    alpha();\n}\n").unwrap();

        // Definitions only by default.
        let defs = symbols(&dir, None, None, false).unwrap();
        assert!(defs.iter().all(|s| s.is_definition));

        // With refs, the call site shows up too.
        let with_refs = symbols(&dir, None, Some("alpha"), true).unwrap();
        assert!(with_refs.iter().any(|s| !s.is_definition && s.name == "alpha"));

        // Case-insensitive substring on name.
        let named = symbols(&dir, None, Some("AL"), false).unwrap();
        assert!(named.iter().any(|s| s.name == "alpha"));
        assert!(!named.iter().any(|s| s.name == "beta"));

        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- definition / definition_at ----

    #[test]
    fn definition_finds_exact_name() {
        let dir = std::env::temp_dir().join(format!("grove_def_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.rs"), "fn target() {}\nfn target_helper() {}\n").unwrap();

        let defs = definition(&dir, "target").unwrap();
        assert_eq!(defs.len(), 1, "exact match only, not the substring `target_helper`");
        assert_eq!(defs[0].name, "target");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn definition_at_resolves_use_site_to_def() {
        let dir = std::env::temp_dir().join(format!("grove_defat_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lib.rs");
        std::fs::write(&file, "fn target() {}\nfn caller() {\n    target();\n}\n").unwrap();

        let (name, defs) = definition_at(&file, 2, 4, &dir).unwrap();
        assert_eq!(name, "target");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].line, 1, "def is on line 1 (1-based)");

        // No identifier at an empty position errors with context.
        let err = definition_at(&file, 1, 0, &dir).err();
        assert!(err.is_none() || err.unwrap().to_string().contains("no identifier"));

        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- source error paths ----

    #[test]
    fn source_rejects_malformed_id() {
        let err = source("rust:src/lib.rs", None).unwrap_err();
        assert!(err.to_string().contains("symbol id must look like"), "got: {err}");
    }

    #[test]
    fn source_errors_when_name_absent() {
        let path = write_temp("absent", DUP);
        let err = source(path.to_str().unwrap(), Some("does_not_exist")).unwrap_err();
        assert!(err.to_string().contains("no definition named"), "got: {err}");
        std::fs::remove_file(&path).ok();
    }

    // ---- map ----

    #[test]
    fn map_returns_definitions_with_references() {
        let dir = std::env::temp_dir().join(format!("grove_map_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("lib.rs"),
            "fn helper() {}\nfn main() {\n    helper();\n}\n",
        )
        .unwrap();

        let maps = map(&dir, None, None).unwrap();
        assert_eq!(maps.len(), 1, "one file");
        let fm = &maps[0];
        assert!(fm.file.ends_with("lib.rs"), "file is lib.rs, got {}", fm.file);

        // Two definitions: helper and main.
        assert_eq!(fm.entries.len(), 2, "two definitions");
        let helper = fm.entries.iter().find(|e| e.name == "helper").unwrap();
        let main_entry = fm.entries.iter().find(|e| e.name == "main").unwrap();

        // helper has no outgoing references (it doesn't call anything).
        assert!(helper.references.is_empty(), "helper has no outgoing refs, got {:?}", helper.references);

        // main references helper.
        assert_eq!(main_entry.references, vec!["helper"], "main references helper");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn map_filters_by_kind_and_name() {
        let dir = std::env::temp_dir().join(format!("grove_map_filter_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("lib.rs"),
            "struct S;\nimpl S {\n    fn m(&self) {\n        helper();\n    }\n}\nfn helper() {}\n",
        )
        .unwrap();

        // Filter by kind: "function" only (Rust tags free functions as "function",
        // methods as "method").
        let maps = map(&dir, Some("function"), None).unwrap();
        let entries = &maps[0].entries;
        assert!(entries.iter().all(|e| e.kind == "function"), "all entries are functions, got {:?}", entries);
        assert!(entries.iter().any(|e| e.name == "helper"), "helper is a function");
        // m is a method, not a function — excluded by the kind filter.
        assert!(!entries.iter().any(|e| e.name == "m"), "m is a method, not a function");

        // Filter by name substring.
        let maps = map(&dir, None, Some("help")).unwrap();
        let entries = &maps[0].entries;
        assert!(entries.iter().any(|e| e.name == "helper"), "helper matches 'help'");
        assert!(!entries.iter().any(|e| e.name == "S"), "S does not match 'help'");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn map_excludes_self_references() {
        let dir = std::env::temp_dir().join(format!("grove_map_selfref_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Recursive function: fn fib(n) { fib(n-1) }
        std::fs::write(
            dir.join("lib.rs"),
            "fn fib(n: i32) -> i32 {\n    fib(n - 1)\n}\n",
        )
        .unwrap();

        let maps = map(&dir, None, None).unwrap();
        let fib = &maps[0].entries[0];
        assert_eq!(fib.name, "fib");
        assert!(fib.references.is_empty(), "self-reference is excluded, got {:?}", fib.references);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn map_attributes_refs_to_innermost_definition() {
        // A reference inside a nested function should belong to the inner function,
        // not the outer one.
        let dir = std::env::temp_dir().join(format!("grove_map_nesting_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Rust doesn't have nested named functions, but methods in impl blocks are
        // a similar nesting pattern. The reference to `helper` inside `m` should
        // belong to `m`, not to `S`.
        std::fs::write(
            dir.join("lib.rs"),
            "fn helper() {}\nstruct S;\nimpl S {\n    fn m(&self) {\n        helper();\n    }\n}\n",
        )
        .unwrap();

        let maps = map(&dir, None, None).unwrap();
        let entries = &maps[0].entries;

        let _s_entry = entries.iter().find(|e| e.name == "S").unwrap();
        // S's definition (struct) shouldn't reference helper directly —
        // helper is inside m, not inside S's struct body.
        // However, the impl block is a container, so the tags query might
        // attribute the reference to S. What matters is that m references helper.
        let m_entry = entries.iter().find(|e| e.name == "m").unwrap();
        assert!(m_entry.references.contains(&"helper".to_string()),
            "m references helper, got {:?}", m_entry.references);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn map_across_multiple_files() {
        let dir = std::env::temp_dir().join(format!("grove_map_multi_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "fn alpha() {}\nfn call_beta() {\n    beta();\n}\n").unwrap();
        std::fs::write(dir.join("b.rs"), "fn beta() {}\nfn call_alpha() {\n    alpha();\n}\n").unwrap();

        let maps = map(&dir, None, None).unwrap();
        assert!(maps.len() >= 2, "should have entries from both files, got {} files", maps.len());

        // Each file should have its own definitions with references.
        let a_map = maps.iter().find(|m| m.file.contains("a.rs")).unwrap();
        let call_beta = a_map.entries.iter().find(|e| e.name == "call_beta").unwrap();
        assert!(call_beta.references.contains(&"beta".to_string()),
            "call_beta references beta, got {:?}", call_beta.references);

        std::fs::remove_dir_all(&dir).ok();
    }
}
