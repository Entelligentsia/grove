//! Answer grounding for the bare-location-line output contract — a port of the
//! reference harness's `run_eval.py::_extract_final` / `strip_leak_lines` /
//! `neutralize_xml`, extended (per the sprint decision) to keep grove's
//! **filesystem validation**: a location line whose path does not resolve under
//! the workspace is dropped.
//!
//! The `base-q4-v2-hf` answer is **not** a `<final_answer>` block of
//! `path:line-range explanation` citations. It is location lines —
//! `lang:relpath#symbol@line`, or `relpath:line` when a point has no enclosing
//! symbol — one per line, most relevant first, nothing else. The `<final_answer>`
//! wrapper is legacy and optional: a tool-less final message *is* the answer.
//!
//! Grounding does three things, mirroring the reference then adding validation:
//! 1. **Unwrap** a `<final_answer>` block if present (legacy path); else take the
//!    raw content.
//! 2. **Strip leak lines** — drop any line carrying a leaked tool-call / control
//!    tag (`<tool_call>`, `</parameter></function></tool_call>`, …), salvaging the
//!    real location lines rather than nullifying the whole message (H3).
//! 3. **Validate** — drop any location line whose path does not resolve to a file
//!    under `root` (grove's value-add over the reference's serve-time grounding).

use std::path::Path;

/// XML-ish tags that a provider's tool-call template parses as markup: if a tool
/// observation (or the model's echoed reasoning) contains them, llama.cpp / ollama
/// can 500 with an XML parse error. Mirrors `run_eval.py::_XML_BREAK`.
const XML_BREAK: &[&str] = &["tool_call", "tool_response", "function", "parameter"];

/// Insert a zero-width space between `<`/`</` and a breaking tag name so the
/// server-side template no longer parses it as a tag, while the text the model
/// reads is unchanged. Port of `run_eval.py::neutralize_xml`.
pub fn neutralize_xml(s: &str) -> String {
    let mut out = s.to_string();
    for t in XML_BREAK {
        out = out
            .replace(&format!("<{t}"), &format!("<\u{200b}{t}"))
            .replace(&format!("</{t}"), &format!("</\u{200b}{t}"));
    }
    out
}

/// True when a single line carries a leaked tool-call / control tag. Port of
/// `run_eval.py::LEAK_RE` (`</?tool_call | </?function[=>\s/] | </?parameter |
/// </?tool_response`), matching **both** the opening `<tool_call>` / `<function=`
/// forms and the closing `</parameter></function></tool_call>` trailer the XML
/// template emits.
fn line_has_leak(line: &str) -> bool {
    let hit = |needle: &str| line.contains(needle);
    if hit("<tool_call") || hit("</tool_call") || hit("<tool_response") || hit("</tool_response")
    {
        return true;
    }
    if hit("<parameter") || hit("</parameter") {
        return true;
    }
    // `<function` / `</function` only when followed by `=`, `>`, whitespace, or `/`
    // so it can't fire on a prose word like "function".
    for pat in ["<function", "</function"] {
        if let Some(idx) = line.find(pat) {
            match line[idx + pat.len()..].chars().next() {
                None | Some('=') | Some('>') | Some('/') => return true,
                Some(c) if c.is_whitespace() => return true,
                _ => {}
            }
        }
    }
    false
}

/// True when any line of `text` carries a leaked control tag — the retry trigger.
pub fn has_leak(text: &str) -> bool {
    text.lines().any(line_has_leak)
}

/// Drop lines carrying a leaked tool-call / control tag; keep the real answer
/// lines. Port of `run_eval.py::strip_leak_lines`.
fn strip_leak_lines(text: &str) -> String {
    text.lines()
        .filter(|l| !line_has_leak(l))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// The final answer content before validation: unwrap a `<final_answer>` block if
/// present (legacy), else strip leak lines from the raw content. Port of
/// `run_eval.py::_extract_final`. Returns `None` when nothing survives (a pure
/// leak, not an answer).
pub fn extract_final(content: &str) -> Option<String> {
    if let Some(inner) = extract_final_answer_block(content) {
        return Some(inner);
    }
    let c = strip_leak_lines(content);
    if c.is_empty() {
        None
    } else {
        Some(c)
    }
}

/// The inner text of a `<final_answer>…</final_answer>` block, if present.
fn extract_final_answer_block(text: &str) -> Option<String> {
    let start_tag = "<final_answer>";
    let end_tag = "</final_answer>";
    let start = text.find(start_tag)? + start_tag.len();
    let rest = &text[start..];
    let end = rest.find(end_tag)?;
    Some(rest[..end].trim().to_string())
}

/// Extract the repo-relative path from a location line, if it is one:
/// - `lang:relpath#symbol@line` → the text between the first `:` and the `#`.
/// - `relpath:line` (no `#`) → the text before the trailing `:<digits>`.
///
/// Returns `None` for a line that is not a location line (kept as-is by the
/// caller — it costs nothing and the reference never validated prose).
fn location_path(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some(hash) = line.find('#') {
        // Form A: lang:path#symbol@line — path is between the first ':' and '#'.
        let before = &line[..hash];
        let colon = before.find(':')?;
        let path = before[colon + 1..].trim();
        return (!path.is_empty()).then_some(path);
    }
    // Form B: path:line — trailing ':' + all-digit suffix.
    let colon = line.rfind(':')?;
    let (path, num) = (line[..colon].trim(), &line[colon + 1..]);
    if !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()) && !path.is_empty() {
        return Some(path);
    }
    None
}

/// Does `path` resolve to a file, as-given or under `root`? Absolute paths are
/// also tried relative to `root` (the model sometimes echoes a rooted path).
fn path_resolves(path: &str, root: &Path) -> bool {
    if Path::new(path).is_file() {
        return true;
    }
    let joined = if Path::new(path).is_absolute() {
        root.join(path.trim_start_matches('/'))
    } else {
        root.join(path)
    };
    joined.is_file()
}

/// The grounded final answer for the bare-location-line contract: extract the
/// answer (unwrap `<final_answer>` or strip leaks), then drop any location line
/// whose path does not resolve under `root`. Non-location lines (rare) are kept.
/// Returns an empty string when nothing survives.
pub fn get_final_answer(text: &str, root: &Path) -> String {
    let inner = match extract_final(text) {
        Some(s) => s,
        None => return String::new(),
    };
    let kept: Vec<&str> = inner
        .lines()
        .filter(|line| match location_path(line) {
            Some(p) => path_resolves(p, root),
            None => !line.trim().is_empty(), // keep non-location, non-empty lines
        })
        .collect();
    kept.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn neutralize_breaks_tags_but_preserves_reading() {
        let n = neutralize_xml("a <tool_call> and </parameter> b");
        assert!(!n.contains("<tool_call>"));
        assert!(n.contains('\u{200b}'));
        // The visible characters (minus the zero-width space) are unchanged.
        assert_eq!(n.replace('\u{200b}', ""), "a <tool_call> and </parameter> b");
    }

    #[test]
    fn leak_lines_are_detected_both_ways() {
        assert!(line_has_leak("<tool_call>{...}"));
        assert!(line_has_leak("#watch@143</parameter></function></tool_call>"));
        assert!(line_has_leak("<function=foo>"));
        assert!(!line_has_leak("go:hugolib/gitinfo.go#forPage@57"));
        assert!(!line_has_leak("the function lives here")); // prose word, not a tag
    }

    #[test]
    fn salvages_answer_lines_from_trailing_tag_junk() {
        let dir = std::env::temp_dir().join(format!("grove-ground-salv-{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/sort.c"), "int main(){}\n").unwrap();
        // A valid location line followed by a leaked closing trailer (hugo:0044 shape).
        let text = "c:src/sort.c#sortCommand@1\n</parameter></function></tool_call>";
        let out = get_final_answer(text, &dir);
        assert_eq!(out, "c:src/sort.c#sortCommand@1");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn drops_location_lines_whose_path_is_hallucinated() {
        let dir = std::env::temp_dir().join(format!("grove-ground-val-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("real.rs"), "fn x(){}\n").unwrap();
        let text = "rust:real.rs#x@1\nrust:ghost.rs#y@9\nreal.rs:1\nghost.rs:9";
        let out = get_final_answer(text, &dir);
        assert!(out.contains("rust:real.rs#x@1"), "kept resolved id: {out}");
        assert!(out.contains("real.rs:1"), "kept resolved path:line: {out}");
        assert!(!out.contains("ghost.rs"), "dropped hallucinated paths: {out}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn optional_final_answer_wrapper_is_unwrapped() {
        let dir = std::env::temp_dir().join(format!("grove-ground-wrap-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.rs"), "fn a(){}\n").unwrap();
        let text = "prose\n<final_answer>\nrust:a.rs#a@1\n</final_answer>";
        let out = get_final_answer(text, &dir);
        assert_eq!(out, "rust:a.rs#a@1");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pure_leak_grounds_to_empty() {
        assert_eq!(get_final_answer("<tool_call>{}", Path::new(".")), "");
    }
}
