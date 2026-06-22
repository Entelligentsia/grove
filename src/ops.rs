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

/// Walk every registered-source file under `dir`, yielding `(grammar, relpath, source)`.
fn for_each_source(dir: &Path, mut f: impl FnMut(&Grammar, &str, &[u8]) -> Result<()>) -> Result<()> {
    for entry in WalkBuilder::new(dir).build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() || !registry::is_source(path) {
            continue;
        }
        let grammar = registry::for_path(path)?;
        let src = read(path)?;
        f(&grammar, &rel(path), &src)?;
    }
    Ok(())
}

/// `outline` — the definitions in one file, optionally filtered by kind.
pub fn outline(file: &Path, kind: Option<&str>) -> Result<Vec<Symbol>> {
    let grammar = registry::for_path(file)?;
    let src = read(file)?;
    let mut syms = engine::extract(&grammar, &rel(file), &src)?;
    syms.retain(|s| s.is_definition && kind.is_none_or(|k| s.kind == k));
    Ok(syms)
}

/// Project symbols to a JSON array at a detail level, to keep payloads bounded:
/// 0 = terse (kind/name/parent/row), 1 = default (adds id/col/signature, drops
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
            m.insert("row".into(), s.row.into());
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
            if kind.is_some_and(|k| s.kind != k) {
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

/// `source` — full code of a symbol, by id (`<lang>:<path>#<name>@<row>`) or
/// by file + name.
pub fn source(id_or_file: &str, name: Option<&str>) -> Result<SourceResult> {
    let (file, want, want_row): (PathBuf, String, Option<usize>) = match name {
        Some(n) => (PathBuf::from(id_or_file), n.to_string(), None),
        None => {
            let rest = id_or_file.split_once(':').map_or(id_or_file, |(_, r)| r);
            let (path, after) = rest
                .split_once('#')
                .context("symbol id must look like <lang>:<path>#<name>@<row>")?;
            // The `@<row>` suffix disambiguates duplicate-named definitions; keep
            // it so the requested symbol is the one returned.
            let (name, row) = match after.split_once('@') {
                Some((n, r)) => (n.to_string(), r.parse::<usize>().ok()),
                None => (after.to_string(), None),
            };
            (PathBuf::from(path), name, row)
        }
    };

    let grammar = registry::for_path(&file)?;
    let src = read(&file)?;
    let syms = engine::extract(&grammar, &rel(&file), &src)?;
    let matches: Vec<&Symbol> = syms
        .iter()
        .filter(|s| s.is_definition && s.name == want)
        .collect();

    // Prefer the exact-row match when the id carried a row; otherwise (name mode,
    // rowless id, or no row matched) fall back to the first definition.
    let chosen = match want_row.and_then(|r| matches.iter().find(|s| s.row == r)) {
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
    pub row: usize,
    pub col: usize,
    /// The function/method that contains this call (`Type::method` when known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_function: Option<String>,
    /// The trimmed source line of the call.
    pub line: String,
}

/// `callers` — every call site of `name` across `dir`, with enclosing function.
///
/// Name-based: matches calls to *any* function/method with this name (the slice
/// does not resolve receiver types). Honest over-match, documented for the agent.
pub fn callers(dir: &Path, name: &str) -> Result<Vec<CallSite>> {
    let mut out = Vec::new();
    for_each_source(dir, |grammar, relpath, src| {
        let syms = engine::extract(grammar, relpath, src)?;
        let calls: Vec<&Symbol> = syms
            .iter()
            .filter(|s| !s.is_definition && grammar.profile.is_call_kind(&s.kind) && s.name == name)
            .collect();
        if calls.is_empty() {
            return Ok(());
        }
        engine::with_tree(grammar, src, |root, profile| {
            for s in &calls {
                out.push(CallSite {
                    in_function: engine::enclosing_function_at(root, s.start_byte, src, profile),
                    file: s.file.clone(),
                    row: s.row,
                    col: s.col,
                    line: s.signature.clone(),
                });
            }
        })?;
        Ok(())
    })?;
    Ok(out)
}

/// Parse a `file:row:col` position string (row/col 0-based).
pub fn parse_pos(s: &str) -> Result<(PathBuf, usize, usize)> {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    match parts.as_slice() {
        [col, row, file] => Ok((
            PathBuf::from(file),
            row.parse().map_err(|_| anyhow::anyhow!("bad row in `{s}`"))?,
            col.parse().map_err(|_| anyhow::anyhow!("bad col in `{s}`"))?,
        )),
        _ => anyhow::bail!("expected file:row:col, got `{s}`"),
    }
}

/// `definition` — exact-name definitions of `name` across `dir` (go-to-def).
pub fn definition(dir: &Path, name: &str) -> Result<Vec<Symbol>> {
    let mut defs = symbols(dir, None, Some(name), false)?;
    defs.retain(|s| s.name == name);
    Ok(defs)
}

/// `definition --at` — resolve the identifier at `file:row:col`, then find its
/// definition(s). Returns the resolved name alongside the matches.
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
    fn id_row_selects_that_definition() {
        let path = write_temp("dup_row", DUP);

        let res = source(&format!("rust:{}#run@4", path.display()), None).unwrap();
        assert!(res.source.contains("_second"), "row 4 must pick the 2nd run, got: {}", res.source);
        assert!(res.id.ends_with("@4"), "chosen id should be the row-4 def, got {}", res.id);

        let res0 = source(&format!("rust:{}#run@0", path.display()), None).unwrap();
        assert!(res0.source.contains("_first"), "row 0 must pick the 1st run, got: {}", res0.source);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unmatched_row_falls_back_to_first() {
        let path = write_temp("dup_fallback", DUP);
        let res = source(&format!("rust:{}#run@99", path.display()), None).unwrap();
        assert!(res.source.contains("_first"), "unknown row falls back to the first def");
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
}
