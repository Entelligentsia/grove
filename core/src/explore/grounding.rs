//! Citation grounding — a direct port of the reference bench's
//! `agent/utils.py` (`parse_citations` / `format_citations` / `get_final_answer`).
//!
//! The bench's answers are not free prose: the model ends with a
//! `<final_answer>` block of `path:line-range explanation` entries, and the
//! harness **validates every cited path against the real filesystem**, dropping
//! any that don't resolve. That validation is the grounding mechanism the study
//! measured (0.93–0.95) — grove's earlier reimplementation returned raw model
//! text and skipped it entirely, which is why a healthy call could still answer
//! with hallucinated paths.

use std::path::Path;

/// One parsed citation: `path:line_range explanation`.
#[derive(Debug, Clone, PartialEq)]
pub struct Citation {
    pub path: String,
    pub line_range: String,
    pub explanation: String,
}

/// Extract the `<final_answer>…</final_answer>` block and parse its lines into
/// citations. Mirrors `parse_citations`: if there is no block, there are no
/// citations (the raw text is the answer).
fn parse_citations(text: &str) -> Vec<Citation> {
    let inner = match extract_final_answer_block(text) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in inner.lines() {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some(c) = parse_citation_line(entry) {
            out.push(c);
        }
    }
    out
}

/// The `<final_answer>` inner text, if present (DOTALL, first match).
fn extract_final_answer_block(text: &str) -> Option<String> {
    let start_tag = "<final_answer>";
    let end_tag = "</final_answer>";
    let start = text.find(start_tag)? + start_tag.len();
    let rest = &text[start..];
    let end = rest.find(end_tag)?;
    Some(rest[..end].trim().to_string())
}

/// Parse one entry like `path:10-15 (reason)` or `path:10`.
/// Mirrors the Python regex `(.+?):(\d+(?:-\d+)?)\s*(.*)`.
fn parse_citation_line(entry: &str) -> Option<Citation> {
    // Find the LAST `:` followed by a digit run (so Windows drive letters or
    // colons in the path don't misparse) — the Python `.+?` is lazy but anchored
    // on the first `:digit`, which for our paths is equivalent to the last colon
    // before the line range.
    let bytes = entry.as_bytes();
    // Scan for a colon that begins a `\d+(-\d+)?` run.
    let mut idx = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b':' && bytes.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            idx = Some(i);
            break;
        }
    }
    let colon = idx?;
    let path = entry[..colon].trim().to_string();
    let after = &entry[colon + 1..];
    // line range: digits, optional -digits
    let mut end = 0;
    let ab = after.as_bytes();
    while end < ab.len() && ab[end].is_ascii_digit() {
        end += 1;
    }
    if end < ab.len() && ab[end] == b'-' {
        let mut j = end + 1;
        while j < ab.len() && ab[j].is_ascii_digit() {
            j += 1;
        }
        if j > end + 1 {
            end = j;
        }
    }
    if end == 0 {
        return None;
    }
    let line_range = after[..end].to_string();
    let explanation = after[end..].trim().to_string();
    Some(Citation {
        path,
        line_range,
        explanation,
    })
}

/// Validate citations against the filesystem and re-emit a `<final_answer>`
/// block. Mirrors `format_citations(validate=True)`: any citation whose path is
/// not an existing file is dropped. Relative paths are resolved against `root`.
fn format_citations(citations: &[Citation], root: &Path) -> String {
    let mut formatted = Vec::new();
    for c in citations {
        let candidate = if Path::new(&c.path).is_absolute() {
            root.join(c.path.trim_start_matches('/'))
        } else {
            root.join(&c.path)
        };
        // Accept either the path as given (absolute, real) or resolved under root.
        let exists = Path::new(&c.path).is_file() || candidate.is_file();
        if !exists {
            continue;
        }
        if c.explanation.is_empty() {
            formatted.push(format!("{}:{}", c.path, c.line_range));
        } else {
            formatted.push(format!("{}:{} {}", c.path, c.line_range, c.explanation));
        }
    }
    format!("<final_answer>\n{}\n</final_answer>", formatted.join("\n"))
}

/// The grounded final answer: parse the model's citations, validate them against
/// `root`, and return the leading prose (everything before `<final_answer>`)
/// plus a re-emitted, validated `<final_answer>` block. Direct port of
/// `get_final_answer`, extended to keep the model's prose preamble.
pub fn get_final_answer(text: &str, root: &Path) -> String {
    let citations = parse_citations(text);
    let validated = format_citations(&citations, root);
    let preamble = match text.find("<final_answer>") {
        Some(i) => text[..i].trim_end(),
        None => text.trim_end(),
    };
    if preamble.is_empty() {
        validated
    } else {
        format!("{preamble}\n\n{validated}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_path_line_range_and_reason() {
        let t = "prose\n<final_answer>\nsrc/a.rs:10-15 (core logic)\nsrc/b.rs:5\n</final_answer>";
        let c = parse_citations(t);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].path, "src/a.rs");
        assert_eq!(c[0].line_range, "10-15");
        assert_eq!(c[0].explanation, "(core logic)");
        assert_eq!(c[1].path, "src/b.rs");
        assert_eq!(c[1].line_range, "5");
        assert_eq!(c[1].explanation, "");
    }

    #[test]
    fn no_block_means_no_citations() {
        assert!(parse_citations("just prose, no answer block").is_empty());
    }

    #[test]
    fn validation_drops_nonexistent_paths() {
        let dir = std::env::temp_dir().join(format!("grove-ground-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("real.rs"), "fn x() {}\n").unwrap();
        let text = "found it\n<final_answer>\nreal.rs:1-1 (here)\nghost.rs:9-9\n</final_answer>";
        let out = get_final_answer(text, &dir);
        assert!(out.contains("real.rs:1-1 (here)"), "kept real path: {out}");
        assert!(!out.contains("ghost.rs"), "dropped hallucinated path: {out}");
        assert!(out.starts_with("found it"), "kept prose preamble: {out}");
        fs::remove_dir_all(&dir).ok();
    }
}
