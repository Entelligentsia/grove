//! Pure data types for the config TUI (Elm-style Model layer).

use grove_core::explore::{DiscoveredEngine, ENGINE_CANDIDATES};
use grove_core::{config::GroveConfig, ExploreConfig, Provider, Steering};
use grove_core::config::Mode;

/// Which field currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Engine,
    Url,
    Model,
    Tap,
    Tools,
}

impl Field {
    /// Advance focus to the next field (wraps around).
    pub fn next(self) -> Self {
        match self {
            Field::Engine => Field::Url,
            Field::Url => Field::Model,
            Field::Model => Field::Tap,
            Field::Tap => Field::Tools,
            Field::Tools => Field::Engine,
        }
    }

    /// Retreat focus to the previous field (wraps around).
    pub fn prev(self) -> Self {
        match self {
            Field::Engine => Field::Tools,
            Field::Url => Field::Engine,
            Field::Model => Field::Url,
            Field::Tap => Field::Model,
            Field::Tools => Field::Tap,
        }
    }
}

/// High-level actions that the event loop dispatches on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Validate and persist the config, then exit.
    Save,
    /// Exit without touching the on-disk config.
    Quit,
    /// Fetch the provider's model list (blocking IO the loop performs, then feeds
    /// back a [`Msg::ModelListFetched`] / [`Msg::ModelFetchError`]).
    FetchModels,
}

/// Application-level messages produced by the event loop and consumed by
/// `update()`.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Focus moves forward.
    TabNext,
    /// Focus moves backward.
    TabPrev,
    /// Engine list — move the highlight up (prev).
    EngineUp,
    /// Engine list — move the highlight down (next).
    EngineDown,
    /// URL field — a printable character was typed.
    UrlChar(char),
    /// URL field — backspace.
    UrlBackspace,
    /// Model field — a printable character was typed.
    ModelChar(char),
    /// Model field — backspace.
    ModelBackspace,
    /// Model field — open the auto-discovery dropdown (triggers a fetch).
    ModelDropdownOpen,
    /// Model dropdown — move the highlight up/down.
    ModelUp,
    ModelDown,
    /// Model dropdown — accept the highlighted entry.
    ModelSelect,
    /// Model dropdown — close without selecting.
    ModelClose,
    /// Model dropdown — the fetched model ids arrived.
    ModelListFetched(Vec<String>),
    /// Model dropdown — the fetch failed (message shown, free-text still works).
    ModelFetchError(String),
    /// Tools list — cursor up.
    ToolsUp,
    /// Tools list — cursor down.
    ToolsDown,
    /// Tools list — toggle selected tool.
    ToolsToggle,
    /// Tools list — a character typed into the add-tool buffer.
    ToolsAddChar(char),
    /// Tools list — backspace in the add-tool buffer.
    ToolsAddBackspace,
    /// Tools list — confirm add-tool buffer as a new tool entry.
    ToolsAddConfirm,
    /// Tap field — toggle session tracing on/off.
    TapToggle,
    /// User pressed save.
    Save,
    /// User pressed quit/cancel.
    Quit,
}

/// The candidate engines rendered before (or without) a live probe: the built-in
/// table, every entry marked not-yet-detected. `mod::run` replaces this with the
/// real [`grove_core::explore::discover_engines`] result at TUI startup.
pub fn unprobed_engines() -> Vec<DiscoveredEngine> {
    ENGINE_CANDIDATES
        .iter()
        .map(|c| DiscoveredEngine {
            label: c.label.to_string(),
            base_url: c.base_url.to_string(),
            alive: false,
            models: Vec::new(),
        })
        .collect()
}

/// The (cosmetic) provider enum grove persists, derived from an endpoint URL.
/// Ollama's default port keys `Ollama`; everything else records `LlamaCpp`.
/// grove never branches on this at request time (both are OpenAI-compatible) —
/// it only labels the trace header and seeds a default URL.
fn provider_for_url(url: &str) -> Provider {
    if url.contains("11434") {
        Provider::Ollama
    } else {
        Provider::LlamaCpp
    }
}

/// The full TUI state.
#[derive(Debug, Clone)]
pub struct App {
    /// The grove integration mode, read from `GroveConfig` — never mutated by TUI.
    pub grove_mode: Mode,
    /// `true` when `grove_mode == Mode::McpLlm`; explore fields are editable only then.
    pub explore_active: bool,
    /// Locally-detected inference engines (built-in candidates, annotated with
    /// liveness + model lists by the startup probe).
    pub engines: Vec<DiscoveredEngine>,
    /// Highlighted row in `engines`.
    pub engine_cursor: usize,
    /// The base URL currently in the text buffer.
    pub base_url: String,
    /// The model currently in the text buffer.
    pub model: String,
    /// `(name, selected)` pairs.
    pub tools: Vec<(String, bool)>,
    /// Cursor position in the tools list.
    pub tool_cursor: usize,
    /// Buffer for a new tool name being typed.
    pub add_tool_buf: String,
    /// Which field is focused.
    pub focus: Field,
    /// Session tracing to `.grove/traces/` (browse with `grove tap`).
    pub tap: bool,
    /// Trace-session retention, carried through unchanged (edited via config file).
    pub trace_retain: u32,
    /// Whether the model auto-discovery dropdown is open.
    pub model_dropdown: bool,
    /// Model ids fetched from the provider's `/models` listing.
    pub model_list: Vec<String>,
    /// Highlighted entry within the filtered dropdown.
    pub model_cursor: usize,
    /// Transient dropdown status (fetching / error / empty), shown in the list.
    pub model_status: Option<String>,
    /// Last validation error to display in the status bar.
    pub last_error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        App::from_grove_config(GroveConfig {
            mode: Mode::McpLlm,
            ..Default::default()
        })
    }
}

impl App {
    /// Construct TUI state from a [`GroveConfig`].
    ///
    /// Sets `grove_mode` and `explore_active` from the config; populates
    /// explore fields from `cfg.explore` when present, or uses defaults.
    pub fn from_grove_config(cfg: GroveConfig) -> Self {
        let explore_active = cfg.mode == Mode::McpLlm;
        let grove_mode = cfg.mode;
        let explore_cfg = cfg.explore.unwrap_or_default();
        let mut app = App::from_config(explore_cfg);
        app.grove_mode = grove_mode;
        app.explore_active = explore_active;
        app
    }

    /// Pre-populate TUI state from an existing [`ExploreConfig`].
    ///
    /// The engine list starts unprobed; the cursor is aligned to the loaded
    /// endpoint (matching a candidate URL, else falling back to the recorded
    /// provider) so the highlighted engine reflects the saved config.
    pub fn from_config(cfg: ExploreConfig) -> Self {
        let engines = unprobed_engines();
        let engine_cursor = engine_index_for(&engines, &cfg.base_url, cfg.provider);
        // Existing tools are shown as selected; no unselected entries from load.
        let tools = cfg.allowed_tools.into_iter().map(|t| (t, true)).collect();
        Self {
            grove_mode: Mode::McpLlm,
            explore_active: true,
            engines,
            engine_cursor,
            base_url: cfg.base_url,
            model: cfg.model,
            tools,
            tool_cursor: 0,
            add_tool_buf: String::new(),
            focus: Field::Engine,
            tap: cfg.tap,
            trace_retain: cfg.trace_retain,
            model_dropdown: false,
            model_list: Vec::new(),
            model_cursor: 0,
            model_status: None,
            last_error: None,
        }
    }

    /// The engine currently highlighted, if any.
    pub fn current_engine(&self) -> Option<&DiscoveredEngine> {
        self.engines.get(self.engine_cursor)
    }

    /// The dropdown entries matching the current model buffer (case-insensitive
    /// substring). An empty buffer shows the full list.
    pub fn model_filtered(&self) -> Vec<String> {
        let needle = self.model.to_lowercase();
        self.model_list
            .iter()
            .filter(|m| needle.is_empty() || m.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    /// Convert TUI state back to a validated [`ExploreConfig`].
    pub fn to_config(&self) -> anyhow::Result<ExploreConfig> {
        // `provider` is a cosmetic label; derive it from the endpoint actually
        // configured (the engine picker or a hand-typed custom URL).
        let provider = provider_for_url(&self.base_url);
        // `steering` no longer selects a prompt arm (the inner harness is the
        // single flat v2 prompt); it is persisted only for on-disk back-compat.
        let steering = Steering::Standard;
        let allowed_tools = self
            .tools
            .iter()
            .filter(|(_, sel)| *sel)
            .map(|(name, _)| name.clone())
            .collect();
        let cfg = ExploreConfig {
            provider,
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            steering,
            allowed_tools,
            tap: self.tap,
            trace_retain: self.trace_retain,
        };
        cfg.validate()?;
        Ok(cfg)
    }
}

/// Pick the engine row that best matches a loaded config: the candidate whose
/// `base_url` equals the saved URL (ignoring a trailing slash), else the first
/// candidate consistent with the recorded provider (ollama's port for
/// `Ollama`), else row 0. Keeps the highlighted engine aligned with the config.
fn engine_index_for(engines: &[DiscoveredEngine], base_url: &str, provider: Provider) -> usize {
    let want = base_url.trim_end_matches('/');
    if let Some(i) = engines.iter().position(|e| e.base_url.trim_end_matches('/') == want) {
        return i;
    }
    let ollama = matches!(provider, Provider::Ollama);
    engines
        .iter()
        .position(|e| e.base_url.contains("11434") == ollama)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_derived_from_endpoint_port() {
        assert_eq!(provider_for_url("http://localhost:11434/v1"), Provider::Ollama);
        assert_eq!(provider_for_url("http://localhost:8080/v1"), Provider::LlamaCpp);
        // Any non-ollama endpoint (lm-studio, vllm, custom) records LlamaCpp —
        // the label is cosmetic; grove never branches on it.
        assert_eq!(provider_for_url("http://localhost:1234/v1"), Provider::LlamaCpp);
        assert_eq!(provider_for_url("http://box:9000/v1"), Provider::LlamaCpp);
    }

    #[test]
    fn engine_index_matches_url_then_falls_back_to_provider() {
        let engines = unprobed_engines();
        // Exact URL match wins (llama.cpp candidate).
        assert_eq!(
            engine_index_for(&engines, "http://localhost:8080/v1/", Provider::Ollama),
            1,
            "trailing-slash-tolerant URL match beats the provider hint"
        );
        // No URL match → fall back to the provider hint (ollama → 11434 row).
        assert_eq!(
            engine_index_for(&engines, "http://custom:9999/v1", Provider::Ollama),
            engines.iter().position(|e| e.base_url.contains("11434")).unwrap(),
        );
    }

    #[test]
    fn unprobed_engines_cover_the_candidate_table() {
        let engines = unprobed_engines();
        assert_eq!(engines.len(), ENGINE_CANDIDATES.len());
        assert!(engines.iter().all(|e| !e.alive && e.models.is_empty()));
    }
}
