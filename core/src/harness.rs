//! Harness shape constants and per-mode query functions.
//!
//! This is the **single source of truth** for every harness constant that
//! both `cli::init` (writing the harness) and `core::doctor` (verifying it)
//! must agree on.  No external crate dependencies other than
//! `core::config::Mode`.

use crate::config::Mode;

// ── Sentinel markers ──────────────────────────────────────────────────────

/// Opening sentinel that brackets the grove steering block in `CLAUDE.md`
/// and `AGENTS.md`.
pub const GROVE_START: &str = "<!-- grove:start -->";

/// Closing sentinel for the grove steering block.
pub const GROVE_END: &str = "<!-- grove:end -->";

// ── MCP server registry key ───────────────────────────────────────────────

/// The key grove registers itself under in `.mcp.json`.  Claude Code
/// namespaces MCP tools as `mcp__<key>__<tool>`, so this string also
/// determines the tool-name prefix the steering directive uses.
pub const MCP_SERVER_KEY: &str = "grove";

// ── Per-mode expected harness content ─────────────────────────────────────

/// The `.mcp.json` `args` array that grove should register under
/// [`MCP_SERVER_KEY`] for `mode`.  Returns `None` when the mode must have
/// **no** grove MCP entry.
pub fn expected_mcp_args(mode: Mode) -> Option<&'static [&'static str]> {
    match mode {
        Mode::Mcp | Mode::Both => Some(&["serve"]),
        Mode::McpLlm => Some(&["serve", "--explore"]),
        Mode::Skill | Mode::Grammars => None,
    }
}

/// The substring that **must** appear inside the grove block in `CLAUDE.md`
/// for `mode`.  Returns `None` for `Grammars` — in that mode the sentinel
/// block must be **absent**.
pub fn expected_claude_marker(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::McpLlm => Some("mcp__grove__explore"),
        Mode::Mcp | Mode::Both => Some("mcp__grove__outline"),
        Mode::Skill => Some("grove skill"),
        Mode::Grammars => None,
    }
}

/// `true` when `mode` requires a grove sentinel block in `AGENTS.md`.
pub fn agents_md_expected(mode: Mode) -> bool {
    mode == Mode::McpLlm
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Mode;

    #[test]
    fn expected_mcp_args_coverage() {
        assert_eq!(expected_mcp_args(Mode::Mcp), Some(["serve"].as_slice()));
        assert_eq!(expected_mcp_args(Mode::Both), Some(["serve"].as_slice()));
        assert_eq!(
            expected_mcp_args(Mode::McpLlm),
            Some(["serve", "--explore"].as_slice())
        );
        assert_eq!(expected_mcp_args(Mode::Skill), None);
        assert_eq!(expected_mcp_args(Mode::Grammars), None);
    }

    #[test]
    fn expected_claude_marker_coverage() {
        assert_eq!(
            expected_claude_marker(Mode::McpLlm),
            Some("mcp__grove__explore")
        );
        assert_eq!(
            expected_claude_marker(Mode::Mcp),
            Some("mcp__grove__outline")
        );
        assert_eq!(
            expected_claude_marker(Mode::Both),
            Some("mcp__grove__outline")
        );
        assert_eq!(expected_claude_marker(Mode::Skill), Some("grove skill"));
        assert_eq!(expected_claude_marker(Mode::Grammars), None);
    }

    #[test]
    fn agents_md_expected_only_mcp_llm() {
        assert!(agents_md_expected(Mode::McpLlm));
        assert!(!agents_md_expected(Mode::Mcp));
        assert!(!agents_md_expected(Mode::Both));
        assert!(!agents_md_expected(Mode::Skill));
        assert!(!agents_md_expected(Mode::Grammars));
    }
}
