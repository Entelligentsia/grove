//! Pure data types for the config TUI (Elm-style Model layer).

use grove_core::{config::GroveConfig, ExploreConfig, Provider, Steering};
use grove_core::config::Mode;

/// Which field currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Provider,
    Url,
    Model,
    Mode,
    Tap,
    Tools,
}

impl Field {
    /// Advance focus to the next field (wraps around).
    pub fn next(self) -> Self {
        match self {
            Field::Provider => Field::Url,
            Field::Url => Field::Model,
            Field::Model => Field::Mode,
            Field::Mode => Field::Tap,
            Field::Tap => Field::Tools,
            Field::Tools => Field::Provider,
        }
    }

    /// Retreat focus to the previous field (wraps around).
    pub fn prev(self) -> Self {
        match self {
            Field::Provider => Field::Tools,
            Field::Url => Field::Provider,
            Field::Model => Field::Url,
            Field::Mode => Field::Model,
            Field::Tap => Field::Mode,
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
    /// Provider list — up/prev.
    ProviderUp,
    /// Provider list — down/next.
    ProviderDown,
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
    /// Mode list — up/prev.
    ModeUp,
    /// Mode list — down/next.
    ModeDown,
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

/// Default base URLs for each provider.
pub const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434/v1";
pub const LLAMACPP_DEFAULT_URL: &str = "http://localhost:8080/v1";

/// The full TUI state.
#[derive(Debug, Clone)]
pub struct App {
    /// The grove integration mode, read from `GroveConfig` — never mutated by TUI.
    pub grove_mode: Mode,
    /// `true` when `grove_mode == Mode::McpLlm`; explore fields are editable only then.
    pub explore_active: bool,
    /// Index into `Provider::LEGAL`.
    pub provider: usize,
    /// The base URL currently in the text buffer.
    pub base_url: String,
    /// The model currently in the text buffer.
    pub model: String,
    /// Index into `Mode::LEGAL`.
    pub mode: usize,
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
    /// `true` after the user has manually edited `base_url`, preventing
    /// provider-switch from clobbering a custom endpoint.
    pub dirty_url: bool,
    /// Last validation error to display in the status bar.
    pub last_error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        App::from_grove_config(GroveConfig {
            version: 1,
            mode: Mode::McpLlm,
            explore: None,
        })
        .reset_dirty()
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
    /// `dirty_url` is set to `true` so that a provider switch does **not**
    /// clobber a custom endpoint the user previously saved.
    pub fn from_config(cfg: ExploreConfig) -> Self {
        let provider = match cfg.provider {
            Provider::Ollama => 0,
            Provider::LlamaCpp => 1,
        };
        let mode = match cfg.steering {
            Steering::Standard => 0,
            Steering::Balanced => 1,
            Steering::Strict => 2,
        };
        // Existing tools are shown as selected; no unselected entries from load.
        let tools = cfg.allowed_tools.into_iter().map(|t| (t, true)).collect();
        Self {
            grove_mode: Mode::McpLlm,
            explore_active: true,
            provider,
            base_url: cfg.base_url,
            model: cfg.model,
            mode,
            tools,
            tool_cursor: 0,
            add_tool_buf: String::new(),
            focus: Field::Provider,
            tap: cfg.tap,
            trace_retain: cfg.trace_retain,
            model_dropdown: false,
            model_list: Vec::new(),
            model_cursor: 0,
            model_status: None,
            dirty_url: true, // loaded URL is custom; don't clobber on provider switch
            last_error: None,
        }
    }

    /// The default-construction path: same as `from_config(default)` but with
    /// `dirty_url` cleared so provider switches refresh the URL default.
    fn reset_dirty(mut self) -> Self {
        self.dirty_url = false;
        self
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
        let provider = match self.provider {
            0 => Provider::Ollama,
            _ => Provider::LlamaCpp,
        };
        let steering = match self.mode {
            0 => Steering::Standard,
            1 => Steering::Balanced,
            _ => Steering::Strict,
        };
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
