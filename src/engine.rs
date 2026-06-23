//! The structural engine: parse + tag extraction + syntax check, over grammars
//! loaded from the registry as wasm.
//!
//! Tags are extracted by running the grammar's `tags.scm` through the Query
//! engine (interpreting `@definition.*` / `@reference.*` / `@name` captures),
//! because `tree-sitter-tags` cannot drive a wasm-loaded language. The same path
//! serves every language, static or wasm.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, WasmStore};

use crate::registry::{Grammar, Profile};

/// A definition or reference extracted from a file.
#[derive(Debug, Serialize)]
pub struct Symbol {
    /// Stable handle: `<lang>:<relpath>#<name>@<line>` (line is 1-based). Survives across turns.
    pub id: String,
    pub name: String,
    /// e.g. `function`, `method`, `class`, `call` — from the grammar's tag query.
    pub kind: String,
    pub is_definition: bool,
    pub file: String,
    /// 1-based line and column of the name — the editor / `grep -n` convention,
    /// so a citation printed as "line N" lands on the right line. (tree-sitter's
    /// own `Point` is 0-based; we normalize here. The byte range below, not these,
    /// is what `source` slices.)
    pub line: usize,
    pub col: usize,
    /// Byte range of the whole symbol (what `source` slices).
    pub start_byte: usize,
    pub end_byte: usize,
    /// The trimmed source line containing the name — a compact signature.
    pub signature: String,
    /// The owning container — the `impl` type, trait, class, or module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// A syntactic defect found by `check`.
#[derive(Debug, Serialize)]
pub struct Defect {
    pub kind: &'static str,
    /// 1-based line and column — same editor / `grep -n` convention as `Symbol`.
    pub line: usize,
    pub col: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub text: String,
}

// The per-language node-kind profile is data now — it comes from the grammar's
// manifest (`registry::Profile`), not a table compiled in here.

// ---- loaded-grammar cache (load each wasm grammar once per process) ----

fn engine() -> &'static tree_sitter::wasmtime::Engine {
    static E: OnceLock<tree_sitter::wasmtime::Engine> = OnceLock::new();
    E.get_or_init(tree_sitter::wasmtime::Engine::default)
}

struct Loaded {
    parser: Parser,
    /// Held to anchor the wasm-loaded language's lifetime alongside its store.
    #[allow(dead_code)]
    language: Language,
    tags_query: Query,
    capture_names: Vec<String>,
}

impl Loaded {
    fn load(g: &Grammar) -> Result<Loaded> {
        let mut store = WasmStore::new(engine()).map_err(|e| anyhow!("wasm store: {e:?}"))?;
        let language = store
            .load_language(&g.name, &g.wasm)
            .map_err(|e| anyhow!("loading `{}` grammar from wasm: {e:?}", g.name))?;
        let tags_query =
            Query::new(&language, &g.tags_query).context("compiling tags query")?;
        let capture_names = tags_query
            .capture_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut parser = Parser::new();
        parser
            .set_wasm_store(store)
            .map_err(|e| anyhow!("attaching wasm store: {e}"))?;
        parser
            .set_language(&language)
            .map_err(|e| anyhow!("setting language: {e}"))?;
        Ok(Loaded { parser, language, tags_query, capture_names })
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Loaded>> = RefCell::new(HashMap::new());
}

fn with_loaded<R>(g: &Grammar, f: impl FnOnce(&mut Loaded) -> Result<R>) -> Result<R> {
    CACHE.with(|c| {
        let mut map = c.borrow_mut();
        if !map.contains_key(&g.name) {
            let loaded = Loaded::load(g)?;
            map.insert(g.name.clone(), loaded);
        }
        f(map.get_mut(&g.name).unwrap())
    })
}

// ---- extraction ----

/// Parse `source` with `parser` — the single choke point for parsing, so cost
/// (and, in tests, the parse count) lives in one place.
fn parse_source(parser: &mut Parser, source: &[u8]) -> Result<tree_sitter::Tree> {
    #[cfg(test)]
    parse_counter::bump();
    parser.parse(source, None).context("parse produced no tree")
}

/// Test-only parse counter, used to prove `callers` parses each file once.
/// Thread-local so it counts only the parses on the calling test's thread —
/// immune to other tests parsing in parallel.
#[cfg(test)]
pub mod parse_counter {
    use std::cell::Cell;
    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }
    pub(super) fn bump() {
        COUNT.with(|c| c.set(c.get() + 1));
    }
    pub fn reset() {
        COUNT.with(|c| c.set(0));
    }
    pub fn get() -> usize {
        COUNT.with(Cell::get)
    }
}

fn symbol_id(lang: &str, rel: &str, name: &str, line: usize) -> String {
    format!("{lang}:{rel}#{name}@{line}")
}

/// The trimmed source line containing `byte`.
fn line_text(source: &[u8], byte: usize) -> String {
    let start = source[..byte.min(source.len())]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let end = source[byte.min(source.len())..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(source.len(), |i| byte + i);
    String::from_utf8_lossy(&source[start..end]).trim().to_string()
}

/// Extract all tags (definitions + references) from one file's source.
pub fn extract(grammar: &Grammar, rel: &str, source: &[u8]) -> Result<Vec<Symbol>> {
    extract_with_tree(grammar, rel, source).map(|(syms, _)| syms)
}

/// Like [`extract`], but also returns the parsed tree so a caller that needs a
/// second pass (e.g. `callers`' enclosing-function lookup) can reuse it instead
/// of re-parsing the identical bytes. Parsing dominates tree-sitter cost.
pub fn extract_with_tree(
    grammar: &Grammar,
    rel: &str,
    source: &[u8],
) -> Result<(Vec<Symbol>, tree_sitter::Tree)> {
    with_loaded(grammar, |lg| {
        let tree = parse_source(&mut lg.parser, source)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&lg.tags_query, tree.root_node(), source);

        let mut out = Vec::new();
        while let Some(m) = matches.next() {
            let mut anchor: Option<(Node, String, bool)> = None;
            let mut name_node: Option<Node> = None;
            for cap in m.captures {
                let cn = &lg.capture_names[cap.index as usize];
                if let Some(kind) = cn.strip_prefix("definition.") {
                    anchor = Some((cap.node, kind.to_string(), true));
                } else if let Some(kind) = cn.strip_prefix("reference.") {
                    anchor = Some((cap.node, kind.to_string(), false));
                } else if cn == "name" {
                    name_node = Some(cap.node);
                }
            }
            let Some((node, kind, is_definition)) = anchor else {
                continue;
            };
            let nn = name_node.unwrap_or(node);
            let name = nn.utf8_text(source).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let pos = nn.start_position();
            // Some upstream tags queries anchor `@definition.function` on a
            // declarator (e.g. C's `function_declarator`) that spans only the
            // signature, not the body. Expand to the enclosing full-function
            // node so `source` returns the complete body. The name position and
            // signature line stay anchored on the name itself.
            let span = definition_span(node, &kind, &grammar.profile);
            // tree-sitter `Point` is 0-based; surface 1-based line/col so the
            // agent-facing handle and output read as human line numbers (#31).
            let line = pos.row + 1;
            out.push(Symbol {
                id: symbol_id(&grammar.name, rel, &name, line),
                name,
                kind,
                is_definition,
                file: rel.to_string(),
                line,
                col: pos.column + 1,
                start_byte: span.start_byte(),
                end_byte: span.end_byte(),
                signature: line_text(source, nn.start_byte()),
                parent: None,
            });
        }

        // Overlapping tag patterns can match the same node twice (e.g. a method
        // matches both @definition.function and @definition.method). Keep the
        // first match per (range, is_definition) — query order puts the more
        // specific pattern first.
        let mut seen = std::collections::HashSet::new();
        out.retain(|s| seen.insert((s.start_byte, s.end_byte, s.is_definition)));

        // Second pass: fill parents from the same tree. Search starts at the
        // def node's *parent* so a container (e.g. a class) is never its own parent.
        let root = tree.root_node();
        for s in &mut out {
            s.parent = root
                .descendant_for_byte_range(s.start_byte, s.end_byte)
                .and_then(|def| def.parent())
                .and_then(|p| nearest_container(p, source, &grammar.profile));
        }
        Ok((out, tree))
    })
}

/// The full source of a symbol, given its byte range.
pub fn slice<'a>(source: &'a [u8], sym: &Symbol) -> &'a str {
    std::str::from_utf8(&source[sym.start_byte..sym.end_byte]).unwrap_or("<non-utf8>")
}

/// Parse a file and report every ERROR / MISSING node.
pub fn check(grammar: &Grammar, source: &[u8]) -> Result<Vec<Defect>> {
    with_loaded(grammar, |lg| {
        let tree = parse_source(&mut lg.parser, source)?;
        let mut defects = Vec::new();
        collect_defects(tree.root_node(), source, &mut defects);
        Ok(defects)
    })
}

fn collect_defects(node: Node, source: &[u8], out: &mut Vec<Defect>) {
    if node.is_error() || node.is_missing() {
        let start = node.start_position();
        out.push(Defect {
            kind: if node.is_missing() { "missing" } else { "error" },
            line: start.row + 1,
            col: start.column + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            text: String::from_utf8_lossy(&source[node.byte_range()])
                .chars()
                .take(60)
                .collect(),
        });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_defects(child, source, out);
    }
}

/// The node whose byte range best represents the *whole* function definition.
///
/// Upstream tags queries vary in what they anchor `@definition.function` on:
/// Rust/Python capture the full function node (body included), but C's query
/// anchors on `function_declarator`, which spans only the signature. When the
/// captured node is such a declarator nested inside a full-function node (per
/// the profile's `function_kinds`), climb to that node so the symbol's byte
/// range covers the body. A bare prototype (no enclosing function node) keeps
/// the declarator range — it has no body to miss.
fn definition_span<'a>(node: Node<'a>, kind: &str, profile: &Profile) -> Node<'a> {
    if kind != "function" && kind != "method" {
        return node;
    }
    let is_fn_kind = |n: &Node| profile.function_kinds.iter().any(|k| k.as_str() == n.kind());
    if is_fn_kind(&node) {
        return node;
    }
    let mut cur = node.parent();
    while let Some(n) = cur {
        if is_fn_kind(&n) {
            return n;
        }
        cur = n.parent();
    }
    node
}

// ---- position resolution (parent / enclosing-fn / go-to-def) ----

/// Name of the nearest container (impl type / trait / class / module) at or
/// above `node`, per the language profile. Pass the def node's parent to exclude
/// the node itself.
fn nearest_container(node: Node, source: &[u8], profile: &Profile) -> Option<String> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        for (kind, field) in &profile.containers {
            if kind.as_str() == n.kind() {
                if let Some(c) = n.child_by_field_name(field) {
                    let text = c.utf8_text(source).ok()?;
                    return Some(text.split('<').next().unwrap_or(text).trim().to_string());
                }
            }
        }
        cur = n.parent();
    }
    None
}

/// Run a closure with the parsed tree of `source` under `grammar`. The closure
/// receives the root node and the grammar's profile.
pub fn with_tree<R>(
    grammar: &Grammar,
    source: &[u8],
    f: impl FnOnce(Node, &Profile) -> R,
) -> Result<R> {
    with_loaded(grammar, |lg| {
        let tree = parse_source(&mut lg.parser, source)?;
        Ok(f(tree.root_node(), &grammar.profile))
    })
}

/// Name of a function node. Prefers a direct `name` field (Rust/Python/JS);
/// falls back to descending the `declarator` chain to the identifier (C, where
/// `function_definition` has no `name` field — the name sits under
/// `function_declarator`, possibly through `pointer_declarator`).
fn function_name(node: Node, source: &[u8], profile: &Profile) -> Option<String> {
    if let Some(n) = node.child_by_field_name("name") {
        return n.utf8_text(source).ok().map(str::to_string);
    }
    let mut cur = node.child_by_field_name("declarator")?;
    loop {
        if profile.identifier_kinds.iter().any(|k| k.as_str() == cur.kind()) {
            return cur.utf8_text(source).ok().map(str::to_string);
        }
        cur = cur.child_by_field_name("declarator")?;
    }
}

/// Name of the function/method enclosing `byte`, qualified by container.
pub fn enclosing_function_at(
    root: Node,
    byte: usize,
    source: &[u8],
    profile: &Profile,
) -> Option<String> {
    let mut node = root.descendant_for_byte_range(byte, byte)?;
    loop {
        if profile.function_kinds.iter().any(|k| k.as_str() == node.kind()) {
            let fname = function_name(node, source, profile)?;
            let container = node
                .parent()
                .and_then(|p| nearest_container(p, source, profile));
            return Some(match container {
                Some(c) => format!("{c}::{fname}"),
                None => fname,
            });
        }
        node = node.parent()?;
    }
}

/// The identifier text at a (row, col) position — for go-to-def.
pub fn identifier_at(
    root: Node,
    row: usize,
    col: usize,
    source: &[u8],
    profile: &Profile,
) -> Option<String> {
    let point = tree_sitter::Point { row, column: col };
    let node = root.descendant_for_point_range(point, point)?;
    if profile.identifier_kinds.iter().any(|k| k.as_str() == node.kind()) {
        node.utf8_text(source).ok().map(str::to_string)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    fn rust() -> Grammar {
        registry::resolve("rust").expect("rust grammar (dev stub or cache)")
    }

    /// Regression guard for #31: the reported `line` must equal the real
    /// `grep -n` line of the definition (1-based), per grammar. The off-by-one
    /// clustered by language, so this asserts across every dev-stub grammar with
    /// the def deliberately placed below line 1 — where a 0-vs-1 slip would show.
    #[test]
    fn reported_line_matches_grep_n_per_grammar() {
        // (lang, source, def name). `target` sits on the 3rd line in each — the
        // line `grep -n target` would report.
        let cases: &[(&str, &str, &str)] = &[
            ("rust", "// header\n\nfn target() {}\n", "target"),
            ("python", "# header\n\ndef target():\n    pass\n", "target"),
            ("javascript", "// header\n\nfunction target() {}\n", "target"),
        ];
        for (lang, src, name) in cases {
            let Ok(g) = registry::resolve(lang) else {
                eprintln!("skipping {lang}: grammar not resolvable in this environment");
                continue;
            };
            let want_line = src
                .lines()
                .position(|l| l.contains(&format!(" {name}")) || l.contains(&format!("{name}(")))
                .map(|i| i + 1)
                .expect("fixture contains the def");
            let syms = extract(&g, &format!("demo.{lang}"), src.as_bytes()).unwrap();
            let def = syms
                .iter()
                .find(|s| s.name == *name && s.is_definition)
                .unwrap_or_else(|| panic!("{lang}: no def named {name}"));
            assert_eq!(
                def.line, want_line,
                "{lang}: reported line {} != grep -n line {want_line}",
                def.line
            );
            // The id's `@<line>` must carry the same 1-based line.
            assert!(
                def.id.ends_with(&format!("@{want_line}")),
                "{lang}: id {} should end with @{want_line}",
                def.id
            );
        }
    }

    #[test]
    fn check_passes_clean_source() {
        let defects = check(&rust(), b"fn main() {}\n").unwrap();
        assert!(defects.is_empty(), "valid rust has no defects, got {defects:?}");
    }

    #[test]
    fn check_reports_defects_on_broken_source() {
        // Unbalanced delimiters force ERROR / MISSING nodes.
        let defects = check(&rust(), b"fn main( {\n").unwrap();
        assert!(!defects.is_empty(), "broken rust must report a defect");
        assert!(defects.iter().all(|d| d.kind == "error" || d.kind == "missing"));
        assert!(defects.iter().all(|d| d.end_byte >= d.start_byte));
    }

    #[test]
    fn extract_finds_definitions_with_container_parent() {
        let src = b"struct S;\nimpl S {\n    fn method(&self) {}\n}\n";
        let syms = extract(&rust(), "lib.rs", src).unwrap();
        let m = syms
            .iter()
            .find(|s| s.name == "method" && s.is_definition)
            .expect("method definition");
        assert_eq!(m.parent.as_deref(), Some("S"), "method's container is impl S");
        assert!(m.id.starts_with("rust:lib.rs#method@"), "stable id, got {}", m.id);
    }

    #[test]
    fn rust_definition_span_covers_the_whole_body() {
        // Rust anchors @definition.function on the full function node, so the
        // captured range already spans the body. Guards against regressing the
        // common case while exercising the slice/range path.
        let src = b"fn f() {\n    let x = 1;\n    x + 1\n}\n";
        let syms = extract(&rust(), "lib.rs", src).unwrap();
        let f = syms.iter().find(|s| s.name == "f" && s.is_definition).unwrap();
        let body = slice(src, f);
        assert!(body.starts_with("fn f()"), "starts at signature: {body:?}");
        assert!(body.trim_end().ends_with('}'), "includes closing brace: {body:?}");
    }

    #[test]
    fn c_function_definition_span_includes_the_body() {
        // C's upstream tags query anchors @definition.function on
        // `function_declarator` (signature only). The engine must expand the
        // range to the enclosing `function_definition` so `source` returns the
        // full body. Skip where the C grammar isn't installed (the dev stub
        // ships rust/python/js only).
        let Ok(c) = registry::resolve("c") else {
            eprintln!("skipping: C grammar not resolvable in this environment");
            return;
        };
        let src = b"static int *get_thing(const char *s,\n                      int n)\n{\n\tint total = 0;\n\treturn &total;\n}\n";
        let syms = extract(&c, "demo.c", src).unwrap();
        let f = syms
            .iter()
            .find(|s| s.name == "get_thing" && s.is_definition)
            .expect("get_thing definition");
        let body = slice(src, f);
        assert!(body.contains("int total = 0"), "body included: {body:?}");
        assert!(body.contains("return &total"), "body included: {body:?}");
        assert!(body.trim_end().ends_with('}'), "closing brace included: {body:?}");
        // The pointer return type sits above the declarator — expansion must
        // reach the whole `function_definition`, not just the declarator.
        assert!(body.starts_with("static int *"), "return type included: {body:?}");
        // Name position still anchors on the identifier, not the expanded span.
        assert_eq!(f.line, 1, "name on the first line (1-based)");
    }

    #[test]
    fn c_callers_capture_calls_with_enclosing_function() {
        // Two-part regression for #27: C's curated tags must capture
        // `@reference.call`, and `enclosing_function_at` must resolve a C
        // function's name through the declarator chain (C `function_definition`
        // has no `name` field). Skip where the C grammar isn't installed.
        let Ok(c) = registry::resolve("c") else {
            eprintln!("skipping: C grammar not resolvable in this environment");
            return;
        };
        let src = b"static int helper(int x) { return x + 1; }\nstatic int caller_one(void) { return helper(5); }\n";
        let (syms, tree) = extract_with_tree(&c, "demo.c", src).unwrap();
        let call = syms
            .iter()
            .find(|s| s.name == "helper" && !s.is_definition)
            .expect("helper call captured as a reference");
        assert_eq!(call.kind, "call", "call reference kind");
        let enc = enclosing_function_at(tree.root_node(), call.start_byte, src, &c.profile);
        assert_eq!(enc.as_deref(), Some("caller_one"), "enclosing fn resolved for C");
    }

    #[test]
    fn extract_with_tree_returns_a_reusable_tree() {
        let src = b"fn helper() {}\nfn caller() {\n    helper();\n}\n";
        let (syms, tree) = extract_with_tree(&rust(), "lib.rs", src).unwrap();
        assert!(syms.iter().any(|s| s.name == "helper" && s.is_definition));
        // The returned tree is usable for the enclosing-function pass.
        let call = syms.iter().find(|s| s.name == "helper" && !s.is_definition).unwrap();
        let enc = enclosing_function_at(tree.root_node(), call.start_byte, src, &rust().profile);
        assert_eq!(enc.as_deref(), Some("caller"));
    }

    #[test]
    fn slice_returns_the_symbols_bytes() {
        let src = b"fn only() { let x = 1; }\n";
        let syms = extract(&rust(), "lib.rs", src).unwrap();
        let f = syms.iter().find(|s| s.name == "only").unwrap();
        let body = slice(src, f);
        assert!(body.starts_with("fn only"));
        assert!(body.contains("let x = 1"));
    }

    #[test]
    fn identifier_at_resolves_the_name_under_the_cursor() {
        let src = b"fn helper() {}\nfn caller() {\n    helper();\n}\n";
        let g = rust();
        let name = with_tree(&g, src, |root, profile| {
            // row 2 (0-based), col 4 — start of `helper` in the call.
            identifier_at(root, 2, 4, src, profile)
        })
        .unwrap();
        assert_eq!(name.as_deref(), Some("helper"));
    }

    #[test]
    fn enclosing_function_at_qualifies_method_with_its_type() {
        let src = b"struct S;\nimpl S {\n    fn m(&self) {\n        let _ = 1;\n    }\n}\n";
        let g = rust();
        // A byte inside the method body.
        let needle = src.windows(9).position(|w| w == b"let _ = 1").unwrap();
        let enc = with_tree(&g, src, |root, profile| {
            enclosing_function_at(root, needle, src, profile)
        })
        .unwrap();
        assert_eq!(enc.as_deref(), Some("S::m"), "method qualified by container type");
    }

    #[test]
    fn extract_dedups_overlapping_matches() {
        // No symbol range appears twice with the same is_definition flag.
        let src = b"struct S;\nimpl S {\n    fn a(&self) {}\n    fn b(&self) {}\n}\n";
        let syms = extract(&rust(), "lib.rs", src).unwrap();
        let mut seen = std::collections::HashSet::new();
        for s in &syms {
            assert!(seen.insert((s.start_byte, s.end_byte, s.is_definition)), "duplicate: {s:?}");
        }
    }
}
