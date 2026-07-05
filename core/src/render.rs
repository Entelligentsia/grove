//! Human text rendering for the read-only structural verbs, shared by the CLI
//! (`cli/src/main.rs`) and the explore inner toolset (`core::explore::toolset`)
//! so both faces emit identical output. See ADR 0003.
//!
//! Each function reproduces the CLI's stdout `println!` block byte-for-byte
//! (one line + `\n` per row). The CLI keeps its own `--json` branch and its
//! `eprintln!` summaries; those stay CLI-only and are not part of this surface.

use crate::engine::Symbol;
use crate::ops::{CallSite, CallSource, FileMap, SourceResult};

/// `grove outline` — one line per definition (kind, name, owner, line:col, sig).
pub fn outline(syms: &[Symbol]) -> String {
    let mut out = String::new();
    for s in syms {
        let owner = s.parent.clone().unwrap_or_default();
        out.push_str(&format!(
            "{:<10} {:<26} {:<18} {}:{:<4} {}\n",
            s.kind, s.name, owner, s.line, s.col, s.signature
        ));
    }
    out
}

/// `grove symbols` — a def/ref mark, kind, name, and the stable symbol id.
pub fn symbols(syms: &[Symbol]) -> String {
    let mut out = String::new();
    for s in syms {
        let mark = if s.is_definition { "def" } else { "ref" };
        out.push_str(&format!("{:<3} {:<10} {:<28} {}\n", mark, s.kind, s.name, s.id));
    }
    out
}

/// `grove source` — the symbol's source body. The CLI's "also matched" hint goes
/// to stderr, so it is not part of captured stdout and not rendered here.
pub fn source(res: &SourceResult) -> String {
    format!("{}\n", res.source)
}

/// `grove callers` — `file:line:col`, enclosing function, provenance tag
/// (`S`=structural / `T`=textual), and the call-site text.
pub fn callers(sites: &[CallSite]) -> String {
    let mut out = String::new();
    for s in sites {
        let inf = s.in_function.as_deref().unwrap_or("<top-level>");
        let tag = if s.source == CallSource::Structural { "S" } else { "T" };
        out.push_str(&format!(
            "{}:{}:{}   {:<28} [{}] {}\n",
            s.file, s.line, s.col, inf, tag, s.text
        ));
    }
    out
}

/// `grove definition` — leads with `file:line:col` so directory-wide hits that
/// span files are locatable without a follow-up `symbols`.
pub fn definition(defs: &[Symbol]) -> String {
    let mut out = String::new();
    for s in defs {
        let owner = s.parent.clone().unwrap_or_default();
        let loc = format!("{}:{}:{}", s.file, s.line, s.col);
        out.push_str(&format!(
            "{:<10} {:<26} {:<18} {:<28} {}\n",
            s.kind, s.name, owner, loc, s.signature
        ));
    }
    out
}

/// `grove map` — each file's definitions with their outgoing references.
pub fn map(maps: &[FileMap]) -> String {
    let mut out = String::new();
    for fm in maps {
        out.push_str(&format!("{}\n", fm.file));
        for e in &fm.entries {
            let parent = e.parent.as_deref().unwrap_or("");
            if e.references.is_empty() {
                out.push_str(&format!(
                    "  {:<10} {:<26} {:<18} {:<4} {}\n",
                    e.kind, e.name, parent, e.row, e.signature
                ));
            } else {
                out.push_str(&format!(
                    "  {:<10} {:<26} {:<18} {:<4} {}  → {}\n",
                    e.kind, e.name, parent, e.row, e.signature, e.references.join(", ")
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::MapEntry;

    fn sym(kind: &str, name: &str, parent: Option<&str>, is_def: bool) -> Symbol {
        Symbol {
            id: format!("rust:x.rs#{name}@1"),
            name: name.into(),
            kind: kind.into(),
            is_definition: is_def,
            file: "x.rs".into(),
            line: 1,
            col: 0,
            start_byte: 0,
            end_byte: 1,
            signature: format!("fn {name}()"),
            parent: parent.map(String::from),
        }
    }

    #[test]
    fn outline_and_symbols_have_one_line_per_symbol() {
        let syms = vec![sym("function", "foo", None, true), sym("method", "bar", Some("Baz"), true)];
        let o = outline(&syms);
        assert_eq!(o.lines().count(), 2);
        assert!(o.contains("function") && o.contains("foo") && o.contains("1:0"));
        assert!(o.contains("Baz"), "owner rendered");

        let s = symbols(&syms);
        assert!(s.starts_with("def "), "definition mark: {s}");
        assert!(s.contains("rust:x.rs#foo@1"), "stable id: {s}");
    }

    #[test]
    fn map_appends_references_arrow_only_when_present() {
        let maps = vec![FileMap {
            file: "x.rs".into(),
            entries: vec![
                MapEntry { id: "i1".into(), kind: "function".into(), name: "a".into(),
                    parent: None, row: 1, signature: "fn a()".into(), references: vec![] },
                MapEntry { id: "i2".into(), kind: "function".into(), name: "b".into(),
                    parent: None, row: 2, signature: "fn b()".into(), references: vec!["a".into()] },
            ],
        }];
        let m = map(&maps);
        assert!(m.starts_with("x.rs\n"), "file header first: {m}");
        assert!(!m.lines().nth(1).unwrap().contains('→'), "no arrow without refs");
        assert!(m.lines().nth(2).unwrap().contains("→ a"), "arrow with refs");
    }
}
