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
    /// Stable handle: `<lang>:<relpath>#<name>@<row>`. Survives across turns.
    pub id: String,
    pub name: String,
    /// e.g. `function`, `method`, `class`, `call` — from the grammar's tag query.
    pub kind: String,
    pub is_definition: bool,
    pub file: String,
    /// 0-based start row/col of the name.
    pub row: usize,
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
    pub row: usize,
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

fn symbol_id(lang: &str, rel: &str, name: &str, row: usize) -> String {
    format!("{lang}:{rel}#{name}@{row}")
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
            out.push(Symbol {
                id: symbol_id(&grammar.name, rel, &name, pos.row),
                name,
                kind,
                is_definition,
                file: rel.to_string(),
                row: pos.row,
                col: pos.column,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
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
            row: start.row,
            col: start.column,
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
            let fname = node.child_by_field_name("name")?.utf8_text(source).ok()?;
            let container = node
                .parent()
                .and_then(|p| nearest_container(p, source, profile));
            return Some(match container {
                Some(c) => format!("{c}::{fname}"),
                None => fname.to_string(),
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
