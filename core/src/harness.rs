//! Harness shape constants and per-mode query functions.
//!
//! This is the **single source of truth** for every harness constant that
//! both `cli::init` (writing the harness) and `core::doctor` (verifying it)
//! must agree on.  No external crate dependencies other than
//! `core::config::Mode`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

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

// ── Harness adapters ──────────────────────────────────────────────────────
//
// A *harness* is a coding agent grove can wire itself into (Claude Code,
// Cursor, Codex, …). It is a second axis, orthogonal to [`Mode`] (which grove
// *surface* is active): `Mode` says "mcp vs mcp-llm vs skill", a `HarnessId`
// says "which agent's config files to write". Each harness differs in three
// things — where its MCP registration lives, in what format, and how its
// steering names grove's tools — captured by the methods below. This is the
// single source of truth both `cli::init` (writer) and `core::doctor`
// (verifier) consult, mirroring the per-mode functions further down.

/// A coding agent grove can wire into a project. On-disk spellings ([`slug`])
/// are kebab-case and used in `.grove/config.json`'s `harnesses` array.
///
/// [`slug`]: HarnessId::slug
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HarnessId {
    /// Anthropic Claude Code — `.mcp.json` (JSON) + `CLAUDE.md`.
    ClaudeCode,
    /// Cursor — `.cursor/mcp.json` (JSON) + `AGENTS.md`.
    Cursor,
    /// OpenAI Codex CLI — `~/.codex/config.toml` (TOML, **global**) + `AGENTS.md`.
    Codex,
    /// Gemini CLI — `.gemini/settings.json` (JSON) + `AGENTS.md`.
    Gemini,
    /// Windsurf — `.windsurf/mcp.json` (JSON) + `AGENTS.md`.
    Windsurf,
    /// VS Code (Copilot) — `.vscode/mcp.json` (JSON, root key `servers`) + `AGENTS.md`.
    VsCode,
}

/// Where a harness's MCP registration file lives, relative to what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Under the project root (committed with the repo).
    Project,
    /// Under the user's home directory (shared across all the user's projects).
    Global,
}

/// The on-disk shape of a harness's MCP registration file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpFormat {
    /// JSON object keyed by `root_key` (`"mcpServers"` for most, `"servers"`
    /// for VS Code). When `needs_type_stdio` is set, each server entry also
    /// carries `"type": "stdio"` (VS Code requires it).
    Json {
        root_key: &'static str,
        needs_type_stdio: bool,
    },
    /// TOML with a `[<table>.grove]` server table (Codex: `mcp_servers`).
    Toml { table: &'static str },
}

impl HarnessId {
    /// Every known harness, in declaration order. Iterate this to reconcile or
    /// verify all harnesses (writing selected ones, stripping the rest).
    pub const ALL: &'static [HarnessId] = &[
        HarnessId::ClaudeCode,
        HarnessId::Cursor,
        HarnessId::Codex,
        HarnessId::Gemini,
        HarnessId::Windsurf,
        HarnessId::VsCode,
    ];

    /// The kebab-case on-disk spelling (used in `config.json` + `--agents`).
    pub fn slug(self) -> &'static str {
        match self {
            HarnessId::ClaudeCode => "claude-code",
            HarnessId::Cursor => "cursor",
            HarnessId::Codex => "codex",
            HarnessId::Gemini => "gemini",
            HarnessId::Windsurf => "windsurf",
            HarnessId::VsCode => "vscode",
        }
    }

    /// The legal slugs, in declaration order — for error messages and `--agents`.
    pub fn legal_slugs() -> Vec<&'static str> {
        Self::ALL.iter().map(|h| h.slug()).collect()
    }

    /// Parse a harness from its kebab-case slug, with a descriptive error that
    /// lists the legal values (mirrors [`Mode::from_name`]).
    pub fn from_slug(s: &str) -> Result<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|h| h.slug() == s)
            .ok_or(())
            .or_else(|()| {
                bail!(
                    "invalid agent `{s}`: expected one of {}",
                    Self::legal_slugs().join(", ")
                )
            })
    }

    /// Human-facing name for status/summary output.
    pub fn display_name(self) -> &'static str {
        match self {
            HarnessId::ClaudeCode => "Claude Code",
            HarnessId::Cursor => "Cursor",
            HarnessId::Codex => "Codex",
            HarnessId::Gemini => "Gemini CLI",
            HarnessId::Windsurf => "Windsurf",
            HarnessId::VsCode => "VS Code",
        }
    }

    /// Whether this harness's MCP registration is project- or user-global.
    pub fn mcp_scope(self) -> Scope {
        match self {
            HarnessId::Codex => Scope::Global,
            _ => Scope::Project,
        }
    }

    /// The on-disk format of this harness's MCP registration file.
    pub fn mcp_format(self) -> McpFormat {
        match self {
            HarnessId::Codex => McpFormat::Toml {
                table: "mcp_servers",
            },
            HarnessId::VsCode => McpFormat::Json {
                root_key: "servers",
                needs_type_stdio: true,
            },
            // Claude Code, Cursor, Gemini, Windsurf all use `mcpServers`.
            _ => McpFormat::Json {
                root_key: "mcpServers",
                needs_type_stdio: false,
            },
        }
    }

    /// The MCP registration path for this harness. Project-scoped harnesses
    /// resolve under `root`; global harnesses (Codex) resolve under the user's
    /// home directory via [`mcp_config_path_in`]. Errors only when a home dir
    /// is required but cannot be determined.
    ///
    /// [`mcp_config_path_in`]: HarnessId::mcp_config_path_in
    pub fn mcp_config_path(self, root: &Path) -> Result<PathBuf> {
        let home = match self.mcp_scope() {
            Scope::Global => {
                Some(dirs::home_dir().ok_or_else(|| {
                    anyhow::anyhow!("cannot determine home directory for {} config", self.display_name())
                })?)
            }
            Scope::Project => None,
        };
        Ok(self.mcp_config_path_in(root, home.as_deref()))
    }

    /// Resolve the MCP registration path with an explicit home directory —
    /// the test seam behind [`mcp_config_path`]. `home` is consulted only for
    /// global-scope harnesses; project-scope harnesses ignore it and resolve
    /// under `root`.
    ///
    /// [`mcp_config_path`]: HarnessId::mcp_config_path
    pub fn mcp_config_path_in(self, root: &Path, home: Option<&Path>) -> PathBuf {
        match self {
            HarnessId::ClaudeCode => root.join(".mcp.json"),
            HarnessId::Cursor => root.join(".cursor").join("mcp.json"),
            HarnessId::Gemini => root.join(".gemini").join("settings.json"),
            HarnessId::Windsurf => root.join(".windsurf").join("mcp.json"),
            HarnessId::VsCode => root.join(".vscode").join("mcp.json"),
            HarnessId::Codex => home
                .unwrap_or(root)
                .join(".codex")
                .join("config.toml"),
        }
    }

    /// The tool-name prefix this harness's steering must use. Claude Code
    /// namespaces MCP tools as `mcp__grove__<tool>`; every other harness reads
    /// the harness-neutral `AGENTS.md` and refers to grove's tools by bare
    /// name. Empty string means "bare".
    pub fn tool_prefix(self) -> &'static str {
        match self {
            HarnessId::ClaudeCode => "mcp__grove__",
            _ => "",
        }
    }

    /// Executable names to probe on `PATH` when auto-detecting this harness.
    /// Claude Code has no distinctive project marker of its own (grove *is* the
    /// default), so it is detected by binary only.
    pub fn detect_bins(self) -> &'static [&'static str] {
        match self {
            HarnessId::ClaudeCode => &["claude"],
            HarnessId::Cursor => &["cursor"],
            HarnessId::Codex => &["codex"],
            HarnessId::Gemini => &["gemini"],
            HarnessId::Windsurf => &["windsurf"],
            HarnessId::VsCode => &["code"],
        }
    }

    /// A project-local marker whose existence signals this harness is in use
    /// here (e.g. `.cursor/`). `None` for harnesses with no distinctive
    /// project footprint (Claude Code, Codex — the latter is user-global).
    pub fn detect_marker(self, root: &Path) -> Option<PathBuf> {
        match self {
            HarnessId::Cursor => Some(root.join(".cursor")),
            HarnessId::Gemini => Some(root.join(".gemini")),
            HarnessId::Windsurf => Some(root.join(".windsurf")),
            HarnessId::VsCode => Some(root.join(".vscode")),
            HarnessId::ClaudeCode | HarnessId::Codex => None,
        }
    }
}

// Serialize via the kebab-case slug so the on-disk spelling in
// `.grove/config.json` and the `--agents` flag share one source of truth
// ([`HarnessId::slug`] / [`HarnessId::from_slug`]) — a derived `rename_all`
// would spell `VsCode` as `vs-code`, diverging from the `vscode` slug.
impl Serialize for HarnessId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.slug())
    }
}

impl<'de> Deserialize<'de> for HarnessId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        HarnessId::from_slug(&s).map_err(serde::de::Error::custom)
    }
}

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

/// `true` when `mode` requires a grove sentinel block in `AGENTS.md`. Every MCP
/// surface writes one: `McpLlm` (explore-mode framing) plus `Mcp`/`Both`
/// (standard 7-tool framing). `AGENTS.md` is the cross-tool standard read by
/// Codex, Cursor, Windsurf, VS Code and Gemini — and, being harness-neutral,
/// it is written even for a Claude-only project so those agents work if added
/// later. `Skill`/`Grammars` write no `AGENTS.md`.
pub fn agents_md_expected(mode: Mode) -> bool {
    matches!(mode, Mode::Mcp | Mode::Both | Mode::McpLlm)
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
    fn agents_md_expected_for_all_mcp_surfaces() {
        assert!(agents_md_expected(Mode::McpLlm));
        assert!(agents_md_expected(Mode::Mcp));
        assert!(agents_md_expected(Mode::Both));
        assert!(!agents_md_expected(Mode::Skill));
        assert!(!agents_md_expected(Mode::Grammars));
    }

    // ── Harness adapter tests ─────────────────────────────────────────────

    #[test]
    fn slug_round_trips_over_all() {
        for &h in HarnessId::ALL {
            assert_eq!(HarnessId::from_slug(h.slug()).unwrap(), h, "round-trip {h:?}");
        }
        // ALL has no duplicate slugs.
        let mut slugs: Vec<_> = HarnessId::ALL.iter().map(|h| h.slug()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), HarnessId::ALL.len(), "slugs must be unique");
    }

    #[test]
    fn from_slug_bad_names_field_and_legal_values() {
        let err = HarnessId::from_slug("emacs").unwrap_err().to_string();
        assert!(err.contains("emacs"), "names the bad value: {err}");
        for &h in HarnessId::ALL {
            assert!(err.contains(h.slug()), "lists legal value {}: {err}", h.slug());
        }
    }

    #[test]
    fn vscode_uses_servers_root_key_and_type_stdio() {
        match HarnessId::VsCode.mcp_format() {
            McpFormat::Json { root_key, needs_type_stdio } => {
                assert_eq!(root_key, "servers", "VS Code root key is `servers`, not `mcpServers`");
                assert!(needs_type_stdio, "VS Code entries need type:stdio");
            }
            other => panic!("VS Code should be JSON, got {other:?}"),
        }
    }

    #[test]
    fn most_json_harnesses_use_mcp_servers() {
        for &h in &[HarnessId::ClaudeCode, HarnessId::Cursor, HarnessId::Gemini, HarnessId::Windsurf] {
            match h.mcp_format() {
                McpFormat::Json { root_key, needs_type_stdio } => {
                    assert_eq!(root_key, "mcpServers", "{h:?} uses mcpServers");
                    assert!(!needs_type_stdio);
                }
                other => panic!("{h:?} should be JSON, got {other:?}"),
            }
        }
    }

    #[test]
    fn codex_is_global_toml() {
        assert_eq!(HarnessId::Codex.mcp_scope(), Scope::Global);
        match HarnessId::Codex.mcp_format() {
            McpFormat::Toml { table } => assert_eq!(table, "mcp_servers"),
            other => panic!("Codex should be TOML, got {other:?}"),
        }
    }

    #[test]
    fn only_claude_code_has_mcp_tool_prefix() {
        assert_eq!(HarnessId::ClaudeCode.tool_prefix(), "mcp__grove__");
        for &h in HarnessId::ALL {
            if h != HarnessId::ClaudeCode {
                assert_eq!(h.tool_prefix(), "", "{h:?} steering uses bare tool names");
            }
        }
    }

    #[test]
    fn project_paths_resolve_under_root_global_under_home() {
        let root = Path::new("/proj");
        let home = Path::new("/home/u");
        assert_eq!(
            HarnessId::ClaudeCode.mcp_config_path_in(root, Some(home)),
            Path::new("/proj/.mcp.json")
        );
        assert_eq!(
            HarnessId::VsCode.mcp_config_path_in(root, Some(home)),
            Path::new("/proj/.vscode/mcp.json")
        );
        assert_eq!(
            HarnessId::Cursor.mcp_config_path_in(root, Some(home)),
            Path::new("/proj/.cursor/mcp.json")
        );
        // Codex is global: resolves under home, ignoring root.
        assert_eq!(
            HarnessId::Codex.mcp_config_path_in(root, Some(home)),
            Path::new("/home/u/.codex/config.toml")
        );
    }

    #[test]
    fn detect_markers_present_only_for_project_footprint_harnesses() {
        let root = Path::new("/proj");
        assert_eq!(HarnessId::Cursor.detect_marker(root), Some(root.join(".cursor")));
        assert_eq!(HarnessId::VsCode.detect_marker(root), Some(root.join(".vscode")));
        assert!(HarnessId::ClaudeCode.detect_marker(root).is_none());
        assert!(HarnessId::Codex.detect_marker(root).is_none());
    }
}
