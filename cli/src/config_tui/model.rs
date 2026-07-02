//! Pure data types for the config TUI (Elm-style Model layer).

use grove_core::{ExploreConfig, Mode, Provider};

/// Which field currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Provider,
    Url,
    Model,
    Mode,
    Tools,
}

impl Field {
    /// Advance focus to the next field (wraps around).
    pub fn next(self) -> Self {
        match self {
            Field::Provider => Field::Url,
            Field::Url => Field::Model,
            Field::Model => Field::Mode,
            Field::Mode => Field::Tools,
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
            Field::Tools => Field::Mode,
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
    /// User pressed save.
    Save,
    /// User pressed quit/cancel.
    Quit,
    /// Stub extension point for future model auto-discovery (AC #5, out-of-scope).
    #[allow(dead_code)]
    ModelListFetched(Vec<String>),
}

/// Default base URLs for each provider.
pub const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434/v1";
pub const LLAMACPP_DEFAULT_URL: &str = "http://localhost:8080/v1";

/// The full TUI state.
#[derive(Debug, Clone)]
pub struct App {
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
    /// `true` after the user has manually edited `base_url`, preventing
    /// provider-switch from clobbering a custom endpoint.
    pub dirty_url: bool,
    /// Last validation error to display in the status bar.
    pub last_error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        let cfg = ExploreConfig::default();
        Self {
            provider: 0, // Ollama
            base_url: cfg.base_url,
            model: cfg.model,
            mode: 0, // Standard
            tools: cfg
                .allowed_tools
                .into_iter()
                .map(|t| (t, true))
                .collect(),
            tool_cursor: 0,
            add_tool_buf: String::new(),
            focus: Field::Provider,
            dirty_url: false,
            last_error: None,
        }
    }
}

impl App {
    /// Pre-populate TUI state from an existing [`ExploreConfig`].
    ///
    /// `dirty_url` is set to `true` so that a provider switch does **not**
    /// clobber a custom endpoint the user previously saved.
    pub fn from_config(cfg: ExploreConfig) -> Self {
        let provider = match cfg.provider {
            Provider::Ollama => 0,
            Provider::LlamaCpp => 1,
        };
        let mode = match cfg.mode {
            Mode::Standard => 0,
            Mode::Balanced => 1,
            Mode::Aggressive => 2,
        };
        // Existing tools are shown as selected; no unselected entries from load.
        let tools = cfg.allowed_tools.into_iter().map(|t| (t, true)).collect();
        Self {
            provider,
            base_url: cfg.base_url,
            model: cfg.model,
            mode,
            tools,
            tool_cursor: 0,
            add_tool_buf: String::new(),
            focus: Field::Provider,
            dirty_url: true, // loaded URL is custom; don't clobber on provider switch
            last_error: None,
        }
    }

    /// Convert TUI state back to a validated [`ExploreConfig`].
    pub fn to_config(&self) -> anyhow::Result<ExploreConfig> {
        let provider = match self.provider {
            0 => Provider::Ollama,
            _ => Provider::LlamaCpp,
        };
        let mode = match self.mode {
            0 => Mode::Standard,
            1 => Mode::Balanced,
            _ => Mode::Aggressive,
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
            mode,
            allowed_tools,
        };
        cfg.validate()?;
        Ok(cfg)
    }
}
