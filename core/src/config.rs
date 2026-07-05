//! Grove project configuration — `.grove/config.json`.
//!
//! [`GroveConfig`] is the top-level project configuration record. It specifies
//! the integration [`Mode`] (which grove surface to activate) and optionally
//! carries an [`ExploreConfig`] section for the mcp-llm explorer subsystem.
//!
//! Persistence is atomic (temp-file + rename) and fail-fast:
//! - [`GroveConfig::load`] yields an actionable error when the file is absent.
//! - [`GroveConfig::save`] creates `.grove/` on first write.
//! - [`GroveConfig::validate`] rejects unknown versions and illegal mode values.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::explore::{ExploreConfig, Provider, Steering};
use crate::harness::HarnessId;

/// The default harness set for a project with no explicit `harnesses` array:
/// Claude Code only. Keeps pre-multi-harness `config.json` files (which lack
/// the field) behaving exactly as before.
pub fn default_harnesses() -> Vec<HarnessId> {
    vec![HarnessId::ClaudeCode]
}

/// Deprecation warning emitted once when `.grove/explore.json` is migrated to
/// `.grove/config.json`. Kept as a named constant so tests can assert on its
/// content without capturing stderr.
pub(crate) const DEPRECATION_WARNING: &str = "\
warning: .grove/explore.json is deprecated and will be removed in a future \
version of grove. Your configuration has been automatically migrated to \
.grove/config.json. Please commit the new file and remove \
.grove/explore.json from your repository.";

/// The integration mode — which grove surface is active for a project.
///
/// On-disk spellings use kebab-case (e.g. `"mcp-llm"`). Use [`Mode::LEGAL`]
/// and [`Mode::from_name`] for parse / enumeration. Serialized with
/// `serde(rename_all = "kebab-case")`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Standard MCP structural surface (outline, symbols, source, …).
    Mcp,
    /// Skill-server surface.
    Skill,
    /// Both MCP and skill surfaces active.
    Both,
    /// MCP + inner LLM explorer (the mcp-llm surface).
    McpLlm,
    /// Grammar-registry management surface.
    Grammars,
}

impl Mode {
    /// The legal on-disk spellings, in declaration order.
    pub const LEGAL: &'static [&'static str] = &["mcp", "skill", "both", "mcp-llm", "grammars"];

    /// Parse a mode from its on-disk spelling, yielding a descriptive error on
    /// failure that names the field and lists legal values.
    pub fn from_name(s: &str) -> Result<Self> {
        match s {
            "mcp" => Ok(Mode::Mcp),
            "skill" => Ok(Mode::Skill),
            "both" => Ok(Mode::Both),
            "mcp-llm" => Ok(Mode::McpLlm),
            "grammars" => Ok(Mode::Grammars),
            other => bail!(
                "invalid `mode` value `{other}`: expected one of {}",
                Self::LEGAL.join(", ")
            ),
        }
    }
}

/// The grove project configuration, persisted to `.grove/config.json`.
///
/// Construct with [`GroveConfig::default`] (mode = `mcp`, no explore section),
/// then persist with [`GroveConfig::save`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroveConfig {
    /// Wire version. The only valid value today is `1`; [`validate`] rejects
    /// other values to ensure future migrations are explicit.
    pub version: u32,
    /// Which grove integration surface is active.
    pub mode: Mode,
    /// Optional mcp-llm explorer configuration. Omitted from the serialized
    /// form when `None` (see `skip_serializing_if`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explore: Option<ExploreConfig>,
    /// The coding agents `grove init` has wired into this project. Defaults to
    /// `[claude-code]` when absent, so pre-multi-harness configs are unchanged.
    #[serde(default = "default_harnesses")]
    pub harnesses: Vec<HarnessId>,
}

impl Default for GroveConfig {
    fn default() -> Self {
        GroveConfig {
            version: 1,
            mode: Mode::Mcp,
            explore: None,
            harnesses: default_harnesses(),
        }
    }
}

/// Raw wire shape: `mode` kept as `String` so parse failures can name the
/// offending field and enumerate legal values — serde's stock unknown-variant
/// error does neither.
#[derive(Deserialize)]
struct RawGroveConfig {
    version: u32,
    mode: String,
    #[serde(default)]
    explore: Option<ExploreConfig>,
    #[serde(default = "default_harnesses")]
    harnesses: Vec<HarnessId>,
}

impl TryFrom<RawGroveConfig> for GroveConfig {
    type Error = anyhow::Error;

    fn try_from(raw: RawGroveConfig) -> Result<Self> {
        if raw.version != 1 {
            bail!(
                "`version` must be 1 (found {}); migrate via `grove upgrade`",
                raw.version
            );
        }
        Ok(GroveConfig {
            version: raw.version,
            mode: Mode::from_name(&raw.mode)?,
            explore: raw.explore,
            harnesses: raw.harnesses,
        })
    }
}

/// Wire shape of the legacy `.grove/explore.json` file.  The old key for
/// steering level was `mode` (not `steering`); this struct mirrors that shape
/// exactly so we can deserialize without touching `ExploreConfig`'s own raw
/// struct (which correctly rejects `mode`).
#[derive(Deserialize)]
struct LegacyExploreRaw {
    provider: String,
    base_url: String,
    model: String,
    /// The old steering-level key.  Maps to `ExploreConfig::steering` after
    /// parsing.
    mode: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    tap: bool,
    #[serde(default = "default_legacy_trace_retain")]
    trace_retain: u32,
}

fn default_legacy_trace_retain() -> u32 {
    crate::explore::config::DEFAULT_TRACE_RETAIN
}

/// Read `.grove/explore.json`, map its old wire shape to a full [`GroveConfig`]
/// (mode = `McpLlm`, `explore.steering` from the legacy `mode` key), persist
/// `config.json` atomically, and emit a one-time deprecation warning to stderr.
///
/// This function is the only code path that reads `explore.json`.  It does not
/// delete `explore.json` — removal is left to the user; `grove doctor` will
/// warn about its presence.
fn migrate_from_legacy_explore(root: &Path) -> Result<GroveConfig> {
    let path = ExploreConfig::config_path(root);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("reading legacy explore config {}", path.display()))?;
    let raw: LegacyExploreRaw = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a valid legacy explore config", path.display()))?;
    let steering = Steering::from_name(&raw.mode)?;
    let provider = Provider::from_name(&raw.provider)?;
    let explore_cfg = ExploreConfig {
        provider,
        base_url: raw.base_url,
        model: raw.model,
        steering,
        allowed_tools: raw.allowed_tools,
        tap: raw.tap,
        trace_retain: raw.trace_retain,
    };
    let config = GroveConfig {
        version: 1,
        mode: Mode::McpLlm,
        explore: Some(explore_cfg),
        harnesses: default_harnesses(),
    };
    config.validate()?;
    config.save(root)?;
    eprintln!("{DEPRECATION_WARNING}");
    Ok(config)
}

impl<'de> serde::Deserialize<'de> for GroveConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawGroveConfig::deserialize(deserializer)?;
        GroveConfig::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl GroveConfig {
    /// The canonical config path for a project rooted at `root`:
    /// `<root>/.grove/config.json`.
    pub fn config_path(root: &Path) -> PathBuf {
        root.join(".grove").join("config.json")
    }

    /// Read, deserialize, and validate the config under `root`.
    ///
    /// Three-branch cascade:
    /// 1. `config.json` present → read, deserialize, validate (normal path).
    /// 2. `explore.json` present → one-time legacy migration: synthesise a
    ///    [`GroveConfig`] from the old wire shape, write `config.json`
    ///    atomically, emit a deprecation warning, and return the config.
    /// 3. Neither → actionable `grove init` error (unchanged).
    pub fn load(root: &Path) -> Result<Self> {
        let path = Self::config_path(root);
        if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let cfg: GroveConfig = serde_json::from_str(&text)
                .with_context(|| format!("{} is not a valid grove config", path.display()))?;
            cfg.validate()?;
            Ok(cfg)
        } else if ExploreConfig::config_path(root).exists() {
            migrate_from_legacy_explore(root)
        } else {
            bail!(
                "no grove config at {} — run `grove init` to create one, \
                 or `grove config` to set it up",
                path.display()
            )
        }
    }

    /// Validate, then persist to `<root>/.grove/config.json` atomically.
    ///
    /// The write goes to a sibling temp file in the same directory and is then
    /// `rename`d into place. Creates `.grove/` if absent.
    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let dir = root.join(".grove");
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("config.json");
        let tmp = dir.join(format!("config.json.tmp.{}", std::process::id()));
        let body = format!("{}\n", serde_json::to_string_pretty(self)?);
        fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    /// Reject structurally invalid state:
    /// - `version` must be `1` (the only supported wire version).
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!(
                "`version` must be 1 (found {}); migrate via `grove upgrade`",
                self.version
            );
        }
        Ok(())
    }
}

/// Caller's preference for the integration mode — lets `active_mode` honour
/// `--explore` / `--standard` CLI flags without re-threading boolean pairs
/// through every call site.
///
/// Precedence: `ForceStandard` wins over everything, then `ForceExplore`,
/// then the config file is consulted (`None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChoice {
    /// No CLI override — read the declared `mode` from the project config.
    None,
    /// Caller passed `--explore`; return `Mode::McpLlm` without reading config.
    ForceExplore,
    /// Caller passed `--standard`; return `Mode::Mcp` without reading config.
    ForceStandard,
}

/// Resolve the effective [`Mode`] for a project, honouring any CLI override
/// expressed as a [`ModeChoice`].
///
/// Precedence:
/// 1. [`ModeChoice::ForceStandard`] → `Mode::Mcp` (config not read).
/// 2. [`ModeChoice::ForceExplore`]  → `Mode::McpLlm` (config not read).
/// 3. [`ModeChoice::None`] → load [`GroveConfig`] and return `cfg.mode`.
///    If loading fails (no `config.json` and no legacy `explore.json`) the
///    function falls back to `Mode::Mcp` and emits a diagnostic on stderr.
///
/// This function performs no network I/O and no health probing — it is a
/// pure config resolver. The legacy `explore.json` path is reached only via
/// [`GroveConfig::load`]'s own cascade (migrate + write `config.json`);
/// callers no longer sniff `explore.json` existence directly.
pub fn active_mode(root: &Path, force: ModeChoice) -> Mode {
    match force {
        ModeChoice::ForceStandard => Mode::Mcp,
        ModeChoice::ForceExplore => Mode::McpLlm,
        ModeChoice::None => match GroveConfig::load(root) {
            Ok(cfg) => cfg.mode,
            Err(e) => {
                eprintln!(
                    "grove: could not load config ({e}); \
                     defaulting to standard structural surface"
                );
                Mode::Mcp
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique, per-process temp project root; caller cleans up.
    fn temp_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("grove_cfg_{}_{tag}", std::process::id()))
    }

    // T1 — round-trip each integration Mode variant.
    #[test]
    fn serde_round_trip_each_mode() {
        for &name in Mode::LEGAL {
            let mode = Mode::from_name(name).unwrap();
            let cfg = GroveConfig { version: 1, mode, explore: None, harnesses: default_harnesses() };
            let json = serde_json::to_string(&cfg).unwrap();
            let back: GroveConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(cfg, back, "round-trip failed for mode={name}");
        }
    }

    // T2 — explore absent when None.
    #[test]
    fn explore_section_absent_when_none() {
        let cfg = GroveConfig::default();
        let v = serde_json::to_value(&cfg).unwrap();
        assert!(
            v.get("explore").is_none(),
            "explore key must be absent when None: {v}"
        );
    }

    // T3 — explore section round-trips when Some.
    #[test]
    fn explore_section_present_when_some() {
        use crate::explore::{ExploreConfig, Provider, Steering};
        let explore = ExploreConfig {
            provider: Provider::Ollama,
            base_url: "http://localhost:11434/v1".to_string(),
            model: "qwen2.5-coder:7b".to_string(),
            steering: Steering::Standard,
            allowed_tools: vec!["grove".to_string()],
            tap: false,
            trace_retain: 50,
        };
        let cfg = GroveConfig { version: 1, mode: Mode::McpLlm, explore: Some(explore.clone()), harnesses: default_harnesses() };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: GroveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
        assert_eq!(back.explore.unwrap(), explore);
    }

    // T4 — bad mode names the field and lists legal values.
    #[test]
    fn bad_mode_error_names_field_and_legal_values() {
        let json = r#"{"version":1,"mode":"unknown"}"#;
        let err = serde_json::from_str::<GroveConfig>(json).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("mode"), "should name the field: {msg}");
        for legal in Mode::LEGAL {
            assert!(msg.contains(legal), "should list legal value {legal}: {msg}");
        }
    }

    // T4b — a config.json without the `harnesses` field defaults to [claude-code]
    // (backward compatibility with pre-multi-harness configs).
    #[test]
    fn missing_harnesses_defaults_to_claude_code() {
        let json = r#"{"version":1,"mode":"mcp"}"#;
        let cfg: GroveConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.harnesses, vec![HarnessId::ClaudeCode]);
    }

    // T4c — an explicit harnesses array round-trips through serde by slug, and
    // preserves order + membership.
    #[test]
    fn explicit_harnesses_round_trip_by_slug() {
        let json = r#"{"version":1,"mode":"mcp","harnesses":["claude-code","cursor","codex","vscode"]}"#;
        let cfg: GroveConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            cfg.harnesses,
            vec![HarnessId::ClaudeCode, HarnessId::Cursor, HarnessId::Codex, HarnessId::VsCode]
        );
        // Re-serialize: the slugs (not derived kebab) must appear — `vscode`, not `vs-code`.
        let out = serde_json::to_string(&cfg).unwrap();
        assert!(out.contains(r#""vscode""#), "serializes VsCode as `vscode`: {out}");
        assert!(!out.contains("vs-code"), "must not use derived kebab spelling: {out}");
    }

    // T4d — an unknown harness slug is a descriptive error listing legal values.
    #[test]
    fn unknown_harness_slug_is_actionable_error() {
        let json = r#"{"version":1,"mode":"mcp","harnesses":["emacs"]}"#;
        let err = serde_json::from_str::<GroveConfig>(json).unwrap_err().to_string();
        assert!(err.contains("emacs"), "names the bad value: {err}");
        assert!(err.contains("cursor"), "lists legal values: {err}");
    }

    // T5 — steering key in explore section deserializes correctly.
    #[test]
    fn steering_key_in_explore_section() {
        let json = r#"{
            "version": 1,
            "mode": "mcp-llm",
            "explore": {
                "provider": "ollama",
                "base_url": "http://localhost:11434/v1",
                "model": "x",
                "steering": "balanced",
                "allowed_tools": ["grove"]
            }
        }"#;
        let cfg: GroveConfig = serde_json::from_str(json).unwrap();
        use crate::explore::Steering;
        assert_eq!(cfg.explore.unwrap().steering, Steering::Balanced);
    }

    // T6 — save + load round-trip; no leftover temp file.
    #[test]
    fn save_load_round_trip_atomic() {
        let root = temp_root("save_load");
        let _ = fs::remove_dir_all(&root);
        let cfg = GroveConfig::default();

        cfg.save(&root).unwrap();

        let path = GroveConfig::config_path(&root);
        assert!(path.exists(), "config.json should exist after save");

        // No leftover temp file.
        let dir = root.join(".grove");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "temp file leaked: {leftovers:?}");

        let loaded = GroveConfig::load(&root).unwrap();
        assert_eq!(cfg, loaded);

        fs::remove_dir_all(&root).unwrap();
    }

    // T7 — missing file gives actionable error.
    #[test]
    fn missing_file_actionable_error() {
        let root = temp_root("missing");
        let err = GroveConfig::load(&root).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("grove init") || msg.contains("grove config"),
            "error should steer user to setup: {msg}"
        );
    }

    // version != 1 rejected by validate().
    #[test]
    fn bad_version_rejected() {
        let json = r#"{"version":2,"mode":"mcp"}"#;
        let err = serde_json::from_str::<GroveConfig>(json).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("version"), "should name the field: {msg}");
    }

    // -----------------------------------------------------------------------
    // T-5a: legacy explore.json (old `mode` key) migrates → config.json.
    // -----------------------------------------------------------------------
    #[test]
    fn migrate_legacy_explore_writes_config_json() {
        use crate::explore::Steering;
        let root = temp_root("legacy_migrate");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();

        // Write the legacy explore.json with old `mode` key ("balanced")
        let legacy = r#"{
            "provider": "ollama",
            "base_url": "http://localhost:11434/v1",
            "model": "qwen2.5-coder:7b",
            "mode": "balanced",
            "allowed_tools": ["grove"],
            "tap": false,
            "trace_retain": 50
        }"#;
        fs::write(grove_dir.join("explore.json"), legacy).unwrap();

        // Load should trigger migration.
        let cfg = GroveConfig::load(&root).unwrap();

        // Returned config: mode = McpLlm, steering = Balanced.
        assert_eq!(cfg.mode, Mode::McpLlm, "mode should be McpLlm after migration");
        let explore = cfg.explore.as_ref().expect("explore section must be present");
        assert_eq!(explore.steering, Steering::Balanced, "steering should be Balanced (mapped from legacy mode=balanced)");

        // config.json must exist on disk after migration.
        let config_path = GroveConfig::config_path(&root);
        assert!(config_path.exists(), "config.json should exist after migration");

        fs::remove_dir_all(&root).unwrap();
    }

    // -----------------------------------------------------------------------
    // T-5b: second load reads config.json directly; explore.json unmodified.
    // -----------------------------------------------------------------------
    #[test]
    fn second_load_after_migration_reads_config_not_legacy() {
        let root = temp_root("legacy_second_load");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();

        let legacy = r#"{
            "provider": "ollama",
            "base_url": "http://localhost:11434/v1",
            "model": "qwen2.5-coder:7b",
            "mode": "standard",
            "allowed_tools": ["grove"]
        }"#;
        let legacy_path = grove_dir.join("explore.json");
        fs::write(&legacy_path, legacy).unwrap();

        // First load triggers migration.
        let cfg1 = GroveConfig::load(&root).unwrap();

        // Record explore.json mtime before second load.
        let mtime_before = fs::metadata(&legacy_path).unwrap().modified().unwrap();

        // Second load reads config.json — no migration re-runs.
        let cfg2 = GroveConfig::load(&root).unwrap();
        assert_eq!(cfg1, cfg2, "second load must return an equal config");

        // explore.json must not have been touched by the second load.
        let mtime_after = fs::metadata(&legacy_path).unwrap().modified().unwrap();
        assert_eq!(mtime_before, mtime_after, "explore.json must not be modified by the second load");

        fs::remove_dir_all(&root).unwrap();
    }

    // -----------------------------------------------------------------------
    // T-5c: config.json present alongside stale explore.json — stale ignored.
    // -----------------------------------------------------------------------
    #[test]
    fn config_json_present_ignores_stale_explore_json() {
        let root = temp_root("stale_explore");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();

        // Write a proper config.json with mode=mcp.
        let cfg_json = r#"{"version":1,"mode":"mcp"}"#;
        fs::write(grove_dir.join("config.json"), cfg_json).unwrap();

        // Write a stale (but parseable) explore.json alongside it.
        let stale = r#"{
            "provider": "ollama",
            "base_url": "http://localhost:11434/v1",
            "model": "old-model",
            "mode": "aggressive",
            "allowed_tools": []
        }"#;
        fs::write(grove_dir.join("explore.json"), stale).unwrap();

        // Load must use config.json, not explore.json.
        let cfg = GroveConfig::load(&root).unwrap();
        assert_eq!(cfg.mode, Mode::Mcp, "should read config.json, not migrate from explore.json");
        assert!(cfg.explore.is_none(), "explore section should not be populated from stale file");

        fs::remove_dir_all(&root).unwrap();
    }

    // -----------------------------------------------------------------------
    // active_mode tests
    // -----------------------------------------------------------------------

    // AM-1: ForceStandard always returns Mcp, even when a config is present.
    #[test]
    fn active_mode_force_standard_returns_mcp() {
        let root = temp_root("am_force_std");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();
        // Write a config that declares mcp-llm, but ForceStandard must win.
        let cfg_json = r#"{"version":1,"mode":"mcp-llm","explore":{"provider":"ollama","base_url":"http://localhost:11434/v1","model":"x","steering":"standard","allowed_tools":[]}}'"#;
        let _ = fs::write(grove_dir.join("config.json"), cfg_json);
        assert_eq!(active_mode(&root, ModeChoice::ForceStandard), Mode::Mcp);
        let _ = fs::remove_dir_all(&root);
    }

    // AM-2: ForceExplore always returns McpLlm, even when a config is present.
    #[test]
    fn active_mode_force_explore_returns_mcp_llm() {
        let root = temp_root("am_force_exp");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();
        // Write a config that declares plain mcp, but ForceExplore must win.
        let cfg_json = r#"{"version":1,"mode":"mcp"}"#;
        fs::write(grove_dir.join("config.json"), cfg_json).unwrap();
        assert_eq!(active_mode(&root, ModeChoice::ForceExplore), Mode::McpLlm);
        let _ = fs::remove_dir_all(&root);
    }

    // AM-3: None + config.json mode=mcp → Mcp.
    #[test]
    fn active_mode_none_reads_declared_mcp_mode() {
        let root = temp_root("am_mcp");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();
        let cfg_json = r#"{"version":1,"mode":"mcp"}"#;
        fs::write(grove_dir.join("config.json"), cfg_json).unwrap();
        assert_eq!(active_mode(&root, ModeChoice::None), Mode::Mcp);
        let _ = fs::remove_dir_all(&root);
    }

    // AM-4: None + config.json mode=mcp-llm → McpLlm.
    #[test]
    fn active_mode_none_reads_declared_mcp_llm_mode() {
        let root = temp_root("am_mcpllm");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();
        let cfg_json = r#"{"version":1,"mode":"mcp-llm","explore":{"provider":"ollama","base_url":"http://localhost:11434/v1","model":"x","steering":"standard","allowed_tools":[]}}"#;
        fs::write(grove_dir.join("config.json"), cfg_json).unwrap();
        assert_eq!(active_mode(&root, ModeChoice::None), Mode::McpLlm);
        let _ = fs::remove_dir_all(&root);
    }

    // AM-5 (bug-1 regression): config.json mode=mcp + stale explore.json → Mcp.
    // The old determine_surface sniffed explore.json existence; active_mode must
    // not — the declared mode in config.json is the single source of truth.
    #[test]
    fn active_mode_mcp_config_ignores_stale_explore_json() {
        let root = temp_root("am_stale");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();
        // config.json declares mcp.
        fs::write(grove_dir.join("config.json"), r#"{"version":1,"mode":"mcp"}"#).unwrap();
        // stale explore.json sits alongside it.
        let stale = r#"{"provider":"ollama","base_url":"http://localhost:11434/v1","model":"old","mode":"aggressive","allowed_tools":[]}"#;
        fs::write(grove_dir.join("explore.json"), stale).unwrap();
        // Must return Mcp (config.json wins; explore.json is ignored).
        assert_eq!(active_mode(&root, ModeChoice::None), Mode::Mcp,
            "stale explore.json must not override declared mode=mcp in config.json");
        let _ = fs::remove_dir_all(&root);
    }

    // AM-6: no config at all → falls back to Mcp (no panic, no error propagated).
    #[test]
    fn active_mode_no_config_falls_back_to_mcp() {
        let root = temp_root("am_noconfig");
        let _ = fs::remove_dir_all(&root);
        // Neither config.json nor explore.json exist.
        assert_eq!(active_mode(&root, ModeChoice::None), Mode::Mcp,
            "missing config must fall back gracefully to Mcp");
        let _ = fs::remove_dir_all(&root);
    }

    // -----------------------------------------------------------------------
    // T-5d: deprecation warning is structurally correct.
    // -----------------------------------------------------------------------
    // We verify via the named DEPRECATION_WARNING const that the warning text
    // is complete and contains the key information users need. The warning IS
    // emitted to stderr during migration (tests 5a and 5b run the migration
    // path and exercise the eprintln! call); here we assert content quality.
    #[test]
    fn deprecation_warning_emitted() {
        // The const must reference both file paths so the message is actionable.
        assert!(
            DEPRECATION_WARNING.contains("explore.json"),
            "warning should mention explore.json: {DEPRECATION_WARNING}"
        );
        assert!(
            DEPRECATION_WARNING.contains("config.json"),
            "warning should mention config.json: {DEPRECATION_WARNING}"
        );
        assert!(
            DEPRECATION_WARNING.contains("deprecated"),
            "warning should contain the word 'deprecated': {DEPRECATION_WARNING}"
        );
        assert!(
            DEPRECATION_WARNING.contains("migrated"),
            "warning should mention migration: {DEPRECATION_WARNING}"
        );

        // Confirm the migration path is reached when only explore.json is present
        // (side-effect of eprintln! being called — no stderr capture needed).
        let root = temp_root("warn_emitted");
        let _ = fs::remove_dir_all(&root);
        let grove_dir = root.join(".grove");
        fs::create_dir_all(&grove_dir).unwrap();
        let legacy = r#"{
            "provider": "ollama",
            "base_url": "http://localhost:11434/v1",
            "model": "x",
            "mode": "standard",
            "allowed_tools": []
        }"#;
        fs::write(grove_dir.join("explore.json"), legacy).unwrap();
        // migration runs → eprintln!(DEPRECATION_WARNING) is called.
        GroveConfig::load(&root).unwrap();
        // config.json written is our proof the warning branch was fully executed.
        assert!(
            GroveConfig::config_path(&root).exists(),
            "config.json must exist after migration (proves warning path ran)"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
