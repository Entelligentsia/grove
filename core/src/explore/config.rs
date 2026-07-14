//! The explore configuration model — `.grove/explore.json`.
//!
//! [`ExploreConfig`] is the shared vocabulary for the mcp-llm inner explorer
//! subsystem (client + agent loop land in later S02 tasks). It is a typed,
//! serde-backed record persisted project-locally under `.grove/` — the same
//! sibling convention as the `.grove/grammars/` grammar registry.
//!
//! Persistence is fail-fast and hardened:
//! - [`ExploreConfig::load`] reads + validates, steering the user to
//!   `grove init --as mcp-llm` / `grove config` when the file is absent.
//! - [`ExploreConfig::save`] writes **atomically** (temp file in the same
//!   directory, then `rename`), so an interrupted write never leaves a torn
//!   config behind.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer, Serialize};

/// The inference backend serving the OpenAI-compatible transport. Both variants
/// speak the same wire protocol; the distinction records operator intent and
/// steers defaults. Serialized lowercase (`"ollama"`, `"llamacpp"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Ollama's OpenAI-compatible endpoint (the default).
    Ollama,
    /// A `llama.cpp` server exposing the OpenAI-compatible endpoint.
    LlamaCpp,
}

impl Provider {
    /// The legal on-disk spellings, in declaration order — used both to parse
    /// and to enumerate valid values in error messages.
    pub const LEGAL: &'static [&'static str] = &["ollama", "llamacpp"];

    pub(crate) fn from_name(s: &str) -> Result<Self> {
        match s {
            "ollama" => Ok(Provider::Ollama),
            "llamacpp" => Ok(Provider::LlamaCpp),
            other => bail!(
                "invalid `provider` value `{other}`: expected one of {}",
                Self::LEGAL.join(", ")
            ),
        }
    }
}

/// The steering level applied to the inner explorer (merit / plan-first /
/// strict). Steering *content* lives in [`crate::explore::steering`]; this enum
/// only names the three levels. Serialized lowercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Steering {
    /// Merit-based steering — the default, least intrusive.
    Standard,
    /// Plan-first steering.
    Balanced,
    /// Strict steering — the mandatory grove-first tool policy (structural
    /// tools before text search).
    Strict,
}

impl Steering {
    /// The legal on-disk spellings, in declaration order.
    pub const LEGAL: &'static [&'static str] = &["standard", "balanced", "strict"];

    pub(crate) fn from_name(s: &str) -> Result<Self> {
        match s {
            "standard" => Ok(Steering::Standard),
            "balanced" => Ok(Steering::Balanced),
            "strict" => Ok(Steering::Strict),
            // Deprecated alias: `strict` was named `aggressive` before it was
            // renamed to drop the negative framing. Accept it on read so
            // existing configs keep loading; it is no longer an advertised value.
            "aggressive" => Ok(Steering::Strict),
            other => bail!(
                "invalid `steering` value `{other}`: expected one of {}",
                Self::LEGAL.join(", ")
            ),
        }
    }
}

/// The explore subsystem configuration, persisted to `.grove/explore.json`.
///
/// Field names are the JSON keys. Construct with [`ExploreConfig::default`] for
/// the llama.cpp reference-rig defaults, then persist with [`ExploreConfig::save`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExploreConfig {
    /// The inference backend.
    pub provider: Provider,
    /// The OpenAI-compatible base URL (e.g. `http://localhost:11434/v1`).
    pub base_url: String,
    /// The model identifier the provider serves.
    pub model: String,
    /// The steering level.
    pub steering: Steering,
    /// The tools the inner explorer is permitted to invoke.
    pub allowed_tools: Vec<String>,
    /// When true, `grove serve --explore` records every LLM request/response of
    /// the session to a structured per-session trace under `.grove/traces/`
    /// (an in-process "tap" — no proxy needed). Browse it with `grove tap`.
    #[serde(default)]
    pub tap: bool,
    /// How many past trace sessions to keep under `.grove/traces/` before the
    /// oldest are pruned. `0` keeps all. Default [`DEFAULT_TRACE_RETAIN`].
    #[serde(default = "default_trace_retain")]
    pub trace_retain: u32,
}

/// Default number of trace sessions retained under `.grove/traces/`.
pub const DEFAULT_TRACE_RETAIN: u32 = 50;

fn default_trace_retain() -> u32 {
    DEFAULT_TRACE_RETAIN
}

/// Raw wire shape with `String` enum fields, so enum parse failures can name
/// the offending field (`provider` / `steering`) and enumerate legal values —
/// serde's stock unknown-variant error names neither the field nor the struct.
#[derive(Deserialize)]
struct RawExploreConfig {
    provider: String,
    base_url: String,
    model: String,
    steering: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    tap: bool,
    #[serde(default = "default_trace_retain")]
    trace_retain: u32,
}

impl TryFrom<RawExploreConfig> for ExploreConfig {
    type Error = anyhow::Error;

    fn try_from(raw: RawExploreConfig) -> Result<Self> {
        Ok(ExploreConfig {
            provider: Provider::from_name(&raw.provider)?,
            base_url: raw.base_url,
            model: raw.model,
            steering: Steering::from_name(&raw.steering)?,
            allowed_tools: raw.allowed_tools,
            tap: raw.tap,
            trace_retain: raw.trace_retain,
        })
    }
}

impl<'de> Deserialize<'de> for ExploreConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawExploreConfig::deserialize(deserializer)?;
        ExploreConfig::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl Default for ExploreConfig {
    fn default() -> Self {
        ExploreConfig {
            // The `base-q4-v2-hf` reference serving rig: qwen3.5-4b (Q4_K_M) on a
            // llama.cpp server (the combination the winning explore harness was
            // proven on). Existing on-disk configs are untouched — only fresh
            // `grove init --as mcp-llm` / `grove config` runs start from this.
            provider: Provider::LlamaCpp,
            base_url: "http://localhost:8080/v1".to_string(),
            model: "qwen3.5-4b".to_string(),
            steering: Steering::Standard,
            allowed_tools: vec![
                "grove".to_string(),
                "rg".to_string(),
                "grep".to_string(),
                "find".to_string(),
            ],
            tap: false,
            trace_retain: DEFAULT_TRACE_RETAIN,
        }
    }
}

impl ExploreConfig {
    /// The on-disk config path for a project rooted at `root`:
    /// `<root>/.grove/explore.json` (sibling to `.grove/grammars/`).
    pub fn config_path(root: &Path) -> PathBuf {
        root.join(".grove").join("explore.json")
    }

    /// Read, deserialize, and validate the config under `root`.
    ///
    /// A missing file yields an actionable error steering the user to
    /// `grove init --as mcp-llm` / `grove config`; a malformed enum value or an
    /// empty required field fails fast, naming the offending field.
    pub fn load(root: &Path) -> Result<Self> {
        let path = Self::config_path(root);
        if !path.exists() {
            bail!(
                "no explore config at {} — run `grove init --as mcp-llm` to create one, \
                 or `grove config` to set it up",
                path.display()
            );
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: ExploreConfig = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a valid explore config", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate, then persist to `<root>/.grove/explore.json` atomically.
    ///
    /// The write goes to a sibling temp file in the same directory and is then
    /// `rename`d into place, so a partial/torn config can never be observed if
    /// the process is interrupted mid-write. Creates `.grove/` if absent.
    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let dir = root.join(".grove");
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("explore.json");
        let tmp = dir.join(format!("explore.json.tmp.{}", std::process::id()));
        let body = format!("{}\n", serde_json::to_string_pretty(self)?);
        fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    /// Reject structurally invalid state, naming the offending field.
    pub fn validate(&self) -> Result<()> {
        if self.base_url.trim().is_empty() {
            bail!("`base_url` must not be empty (e.g. http://localhost:11434/v1)");
        }
        if self.model.trim().is_empty() {
            bail!("`model` must not be empty (set a model identifier your provider serves)");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique, per-process temp project root; caller cleans up.
    fn temp_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("grove_explore_cfg_{}_{tag}", std::process::id()))
    }

    #[test]
    fn serde_round_trip_defaults() {
        let cfg = ExploreConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ExploreConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn fixed_fixture_deserializes() {
        let json = r#"{
            "provider": "llamacpp",
            "base_url": "http://localhost:8080/v1",
            "model": "custom",
            "steering": "strict",
            "allowed_tools": ["grove"]
        }"#;
        let cfg: ExploreConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.provider, Provider::LlamaCpp);
        assert_eq!(cfg.steering, Steering::Strict);
        assert_eq!(cfg.base_url, "http://localhost:8080/v1");
        assert_eq!(cfg.model, "custom");
        assert_eq!(cfg.allowed_tools, vec!["grove".to_string()]);
    }

    #[test]
    fn strict_serializes_lowercase_and_aggressive_is_a_read_alias() {
        // Current value serializes as `strict`.
        assert_eq!(serde_json::to_string(&Steering::Strict).unwrap(), "\"strict\"");
        assert_eq!(Steering::from_name("strict").unwrap(), Steering::Strict);
        // Deprecated pre-rename spelling still loads (back-compat), mapping to Strict.
        assert_eq!(Steering::from_name("aggressive").unwrap(), Steering::Strict);
    }

    #[test]
    fn defaults_are_the_llamacpp_reference_rig() {
        let cfg = ExploreConfig::default();
        assert_eq!(cfg.provider, Provider::LlamaCpp);
        assert_eq!(cfg.base_url, "http://localhost:8080/v1");
        assert_eq!(cfg.model, "qwen3.5-4b");
        assert_eq!(cfg.steering, Steering::Standard);
        assert_eq!(cfg.allowed_tools, vec!["grove", "rg", "grep", "find"]);
        assert_eq!(cfg.trace_retain, DEFAULT_TRACE_RETAIN);
    }

    #[test]
    fn trace_retain_defaults_when_absent() {
        // A config file written before trace_retain existed must still load,
        // defaulting the field rather than failing.
        let json = r#"{
            "provider": "ollama",
            "base_url": "http://localhost:11434/v1",
            "model": "x",
            "steering": "standard",
            "allowed_tools": ["grove"]
        }"#;
        let cfg: ExploreConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.trace_retain, DEFAULT_TRACE_RETAIN);
    }

    #[test]
    fn provider_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Provider::Ollama).unwrap(), "\"ollama\"");
        assert_eq!(serde_json::to_string(&Provider::LlamaCpp).unwrap(), "\"llamacpp\"");
        assert_eq!(serde_json::to_string(&Steering::Standard).unwrap(), "\"standard\"");
    }

    #[test]
    fn missing_file_steers_to_init() {
        let root = temp_root("missing");
        let err = ExploreConfig::load(&root).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("grove init --as mcp-llm") && msg.contains("grove config"),
            "message should steer to setup: {msg}"
        );
    }

    #[test]
    fn bad_enum_names_field_and_legal_values() {
        let json = r#"{
            "provider": "gpt4all",
            "base_url": "http://localhost:11434/v1",
            "model": "x",
            "steering": "standard",
            "allowed_tools": []
        }"#;
        let err = serde_json::from_str::<ExploreConfig>(json).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("provider"), "should name the field: {msg}");
        assert!(msg.contains("ollama") && msg.contains("llamacpp"), "should list legal values: {msg}");
    }

    #[test]
    fn empty_required_fields_rejected() {
        let cfg = ExploreConfig { base_url: "   ".to_string(), ..ExploreConfig::default() };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("base_url"), "{err}");

        let cfg = ExploreConfig { model: String::new(), ..ExploreConfig::default() };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("model"), "{err}");
    }

    #[test]
    fn save_then_load_round_trips_atomically() {
        let root = temp_root("save_load");
        let _ = fs::remove_dir_all(&root);
        let cfg = ExploreConfig::default();

        cfg.save(&root).unwrap();

        let path = ExploreConfig::config_path(&root);
        assert!(path.exists(), "explore.json should exist after save");

        // No leftover temp file beside the config after a successful save.
        let dir = root.join(".grove");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "temp file leaked: {leftovers:?}");

        let loaded = ExploreConfig::load(&root).unwrap();
        assert_eq!(cfg, loaded);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_mode_key_rejected() {
        // The old JSON key "mode" is not accepted — T02 owns migration.
        // Missing "steering" field must fail deserialization.
        let json = r#"{
            "provider": "ollama",
            "base_url": "http://localhost:11434/v1",
            "model": "x",
            "mode": "standard",
            "allowed_tools": []
        }"#;
        let err = serde_json::from_str::<ExploreConfig>(json).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("steering") || msg.contains("missing"), "should fail on missing steering: {msg}");
    }
}
