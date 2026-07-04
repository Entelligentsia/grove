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

use crate::explore::ExploreConfig;

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
}

impl Default for GroveConfig {
    fn default() -> Self {
        GroveConfig {
            version: 1,
            mode: Mode::Mcp,
            explore: None,
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
        })
    }
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
    /// A missing file yields an actionable error message; a bad `mode` value
    /// or unknown version fails fast, naming the offending field.
    pub fn load(root: &Path) -> Result<Self> {
        let path = Self::config_path(root);
        if !path.exists() {
            bail!(
                "no grove config at {} — run `grove init` to create one, \
                 or `grove config` to set it up",
                path.display()
            );
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: GroveConfig = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a valid grove config", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
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
            let cfg = GroveConfig { version: 1, mode, explore: None };
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
        let cfg = GroveConfig { version: 1, mode: Mode::McpLlm, explore: Some(explore.clone()) };
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
}
