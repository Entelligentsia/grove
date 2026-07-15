//! The OpenAI-compatible chat client + health probe for the inner explorer.
//!
//! This module (S02-T02) is the transport layer between the inner explorer
//! agent loop (T03) and a local inference server — either **Ollama** or a
//! **llama.cpp** server. Both speak the OpenAI chat-completions wire protocol,
//! so a single non-streaming client serves both; the only provider-specific
//! knowledge — how tool-call `arguments` are encoded — is normalized here so
//! callers always see a uniform [`ToolCall`].
//!
//! The design is four concerns, layered:
//!
//! 1. **Wire model** — [`ChatRequest`] / [`ChatResponse`] and the [`Message`]
//!    / [`ToolCall`] / [`Tool`] types serde-model the request/response subset
//!    we need. Optional request fields are omitted when unset so we emit a lean
//!    body both providers accept.
//! 2. **Transport trait** — [`ChatClient`] is the seam the agent loop depends
//!    on (never the concrete type), so T03 can substitute a fake in tests.
//!    [`OpenAiCompatClient`] is the real implementation over `ureq` + rustls
//!    (the same blocking HTTP stack `fetch.rs` uses — no new dependency).
//! 3. **Dialect normalization** — tool-call `arguments` arrive as an
//!    already-parsed object (llama.cpp) or a JSON-encoded string (Ollama and
//!    the OpenAI spec). [`ToolCall`]'s custom deserializer absorbs both into a
//!    single normalized `serde_json::Value` (always the parsed object).
//! 4. **Typed errors + health probe** — [`ClientError`] separates
//!    [`Connection`](ClientError::Connection) (the D3 shutdown signal — server
//!    unreachable / refused / timed out) from protocol/HTTP errors.
//!    [`health_probe`] does a pre-flight reachability + model-availability check
//!    against `{base_url}/models`, with [`HealthError`] naming the endpoint or
//!    model in an actionable message.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::config::ExploreConfig;

/// Connect-phase timeout — a refused/unroutable server fails fast rather than
/// hanging the agent loop.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Overall request deadline for a chat completion. Generous, because local
/// models can take a while to generate; a breach surfaces as a transport error
/// and is therefore classified as [`ClientError::Connection`] (a D3 signal).
const CHAT_TIMEOUT: Duration = Duration::from_secs(300);
/// Overall deadline for the lightweight health probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Wire model
// ---------------------------------------------------------------------------

/// A chat message role. Serialized lowercase (`system` / `user` / `assistant`
/// / `tool`), matching the OpenAI schema both providers implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System / steering prompt.
    System,
    /// End-user turn.
    User,
    /// Model turn — may carry `tool_calls`.
    Assistant,
    /// A tool result fed back into the conversation; must carry the
    /// `tool_call_id` it answers.
    Tool,
}

/// A single conversation turn.
///
/// `content` is optional because an assistant turn that only requests tools may
/// omit it; a `tool` turn must set both `content` (the result) and
/// `tool_call_id` (which call it answers). Empty collections/None fields are
/// omitted on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// The turn's role.
    pub role: Role,
    /// The textual content, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool calls requested by an assistant turn (normalized).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For a `tool` turn: the id of the call this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional participant name (rarely used; passed through if set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// A `system` steering message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::text(Role::System, content)
    }

    /// A `user` message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::text(Role::User, content)
    }

    /// An `assistant` message carrying free text (no tool calls).
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::text(Role::Assistant, content)
    }

    /// A `tool` result message answering a specific `tool_call_id`.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Message {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }

    fn text(role: Role, content: impl Into<String>) -> Self {
        Message {
            role,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
        }
    }
}

/// A normalized tool call requested by the model.
///
/// Regardless of how the provider encoded `arguments` on the wire (a JSON
/// object or a JSON-encoded string), `arguments` is always the *parsed* value
/// here — typically a `Value::Object`. On serialization (feeding an assistant
/// turn back to the model) it is re-emitted in the canonical OpenAI shape
/// (`{"id", "type":"function", "function":{"name","arguments":"<json string>"}}`).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Provider-assigned call id (correlates with a later `tool` message).
    pub id: String,
    /// The function name the model wants to invoke.
    pub name: String,
    /// The call arguments, normalized to a parsed JSON value.
    pub arguments: Value,
}

impl Serialize for ToolCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Canonical OpenAI shape: arguments as a JSON-encoded string.
        let arguments =
            serde_json::to_string(&self.arguments).map_err(serde::ser::Error::custom)?;
        let wire = serde_json::json!({
            "id": self.id,
            "type": "function",
            "function": { "name": self.name, "arguments": arguments },
        });
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            id: String,
            #[serde(default)]
            function: RawFunction,
        }
        #[derive(Deserialize, Default)]
        struct RawFunction {
            #[serde(default)]
            name: String,
            #[serde(default)]
            arguments: Value,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(ToolCall {
            id: raw.id,
            name: raw.function.name,
            arguments: normalize_arguments(raw.function.arguments),
        })
    }
}

/// Normalize a wire `arguments` value into a parsed JSON value.
///
/// - A JSON-encoded **string** (Ollama / OpenAI spec) is parsed; if it is empty
///   or unparseable it degrades gracefully (empty object / the raw string).
/// - An already-parsed **object** (llama.cpp) passes through untouched.
/// - `null`/absent becomes an empty object, so callers never special-case it.
fn normalize_arguments(v: Value) -> Value {
    match v {
        Value::String(s) => {
            if s.trim().is_empty() {
                Value::Object(serde_json::Map::new())
            } else {
                serde_json::from_str(&s).unwrap_or(Value::String(s))
            }
        }
        Value::Null => Value::Object(serde_json::Map::new()),
        other => other,
    }
}

/// A tool declaration offered to the model (request side).
///
/// `parameters` is a raw JSON-schema `Value`, so any schema passes through
/// untouched.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    /// Always `"function"` for the current OpenAI tool protocol.
    #[serde(rename = "type")]
    pub kind: String,
    /// The function declaration.
    pub function: ToolFunction,
}

impl Tool {
    /// Construct a `function`-type tool from its name, description, and a
    /// JSON-schema parameters value.
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Tool {
            kind: "function".to_string(),
            function: ToolFunction {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// The function half of a [`Tool`] declaration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolFunction {
    /// The function name the model may call.
    pub name: String,
    /// A human/model-readable description of what it does.
    pub description: String,
    /// JSON-schema describing the call arguments.
    pub parameters: Value,
}

/// A non-streaming chat-completions request.
///
/// `model` is set by [`OpenAiCompatClient::chat`] from config, so callers can
/// leave it empty (via [`ChatRequest::new`]); optional fields are omitted from
/// the wire body when unset.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatRequest {
    /// The model identifier (filled by the client from config).
    pub model: String,
    /// The conversation so far.
    pub messages: Vec<Message>,
    /// Tools the model may call.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// Tool-choice directive (`"auto"`, `"none"`, or a forced function object).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    /// Sampling temperature, if overriding the server default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Max generated tokens (OpenAI `max_completion_tokens`). The reference
    /// bench pins this at 1024; leaving it unset lets small models ramble.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Nucleus sampling (bench uses 0.95).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k sampling — sent inside the bench's qwen `extra_body` (20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// qwen chat-template kwargs — the bench sends `{"enable_thinking": false}`
    /// here, which is decisive for qwen3.x tool-calling quality.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_kwargs: Option<Value>,
    /// Optional `reasoning_effort` passthrough (bench: `"none"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl ChatRequest {
    /// A request from a message list, with no tools and server-default
    /// sampling. `model` is left empty for the client to fill.
    pub fn new(messages: Vec<Message>) -> Self {
        ChatRequest {
            model: String::new(),
            messages,
            tools: Vec::new(),
            tool_choice: None,
            temperature: None,
            max_completion_tokens: None,
            top_p: None,
            top_k: None,
            chat_template_kwargs: None,
            reasoning_effort: None,
        }
    }

    /// Builder: attach tool declarations.
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    /// Builder: set the tool-choice directive.
    pub fn with_tool_choice(mut self, choice: Value) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Builder: apply the reference bench's sampling parameters. `qwen` gates
    /// the `top_k` + `enable_thinking:false` knobs the bench sends only for
    /// qwen models (`llm.py`: `if "qwen" in self.model`).
    pub fn with_bench_sampling(
        mut self,
        temperature: f32,
        top_p: f32,
        max_completion_tokens: u32,
        reasoning_effort: Option<String>,
        qwen: bool,
    ) -> Self {
        self.temperature = Some(temperature);
        self.top_p = Some(top_p);
        self.max_completion_tokens = Some(max_completion_tokens);
        self.reasoning_effort = reasoning_effort;
        if qwen {
            self.top_k = Some(20);
            self.chat_template_kwargs = Some(serde_json::json!({ "enable_thinking": false }));
        }
        self
    }

    /// Builder: apply the `base-q4-v2-hf` reference-explore sampling
    /// (`run_eval.py`): `temperature` + `max_tokens` + `chat_template_kwargs`
    /// carrying `enable_thinking` — and nothing else. Unlike
    /// [`Self::with_bench_sampling`], this sends **no** `top_p` / `top_k` /
    /// `reasoning_effort` (the winning harness passed only these three knobs), and
    /// `think` is the interim winner's setting (**on**), not forced off.
    pub fn with_explore_sampling(mut self, temperature: f32, max_tokens: u32, think: bool) -> Self {
        self.temperature = Some(temperature);
        self.max_completion_tokens = Some(max_tokens);
        self.top_p = None;
        self.top_k = None;
        self.reasoning_effort = None;
        self.chat_template_kwargs = Some(serde_json::json!({ "enable_thinking": think }));
        self
    }
}

/// A non-streaming chat-completions response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The completion choices; we use the first.
    #[serde(default)]
    pub choices: Vec<Choice>,
    /// Token accounting for this completion, when the provider reports it. Both
    /// Ollama and llama.cpp emit an OpenAI-style `usage` object; it is the source
    /// of the per-turn / per-call token metrics the trace subsystem records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// The first choice's message, if any.
    pub fn first_message(&self) -> Option<&Message> {
        self.choices.first().map(|c| &c.message)
    }
}

/// OpenAI-style token accounting for a completion. All fields default to `0`
/// when a provider omits them, so a partial `usage` object never fails to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens consumed by the prompt (input).
    #[serde(default)]
    pub prompt_tokens: u32,
    /// Tokens generated in the completion (output).
    #[serde(default)]
    pub completion_tokens: u32,
    /// Prompt + completion. Providers usually report this; when absent it can be
    /// recomputed as `prompt_tokens + completion_tokens`.
    #[serde(default)]
    pub total_tokens: u32,
}

/// A single completion choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    /// The assistant message for this choice.
    pub message: Message,
    /// Why generation stopped (`stop`, `tool_calls`, …), if reported.
    #[serde(default)]
    pub finish_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Transport trait + errors
// ---------------------------------------------------------------------------

/// The transport seam the agent loop depends on.
///
/// T03 depends on this trait, never [`OpenAiCompatClient`], so a fake can be
/// substituted in tests without a live server.
pub trait ChatClient {
    /// Send a chat-completions request and return the normalized response.
    fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ClientError>;
}

/// A chat-transport error.
///
/// [`Connection`](Self::Connection) is the D3 shutdown signal — the server is
/// unreachable, refused the connection, or the request timed out. The other
/// variants mean the server was reached but the exchange failed.
#[derive(Debug)]
pub enum ClientError {
    /// Transport failure — unreachable / refused / timed out. Carries the URL
    /// and the underlying detail.
    Connection {
        /// The endpoint that could not be reached.
        url: String,
        /// The transport-level detail.
        detail: String,
    },
    /// The server returned a non-2xx status. Carries the status and response
    /// body (for an actionable message).
    Http {
        /// The endpoint.
        url: String,
        /// The HTTP status code.
        status: u16,
        /// The response body, if any.
        body: String,
    },
    /// The server responded but the body could not be parsed as the expected
    /// chat-completions shape.
    Protocol {
        /// The endpoint.
        url: String,
        /// The parse error detail.
        detail: String,
        /// The raw body that failed to parse.
        body: String,
    },
    /// The request could not be encoded to JSON (a programming error).
    Encode(String),
}

impl ClientError {
    /// Whether this is a connection-level failure (the D3 shutdown signal).
    pub fn is_connection(&self) -> bool {
        matches!(self, ClientError::Connection { .. })
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Connection { url, detail } => {
                write!(f, "could not reach the inference server at {url}: {detail}")
            }
            ClientError::Http { url, status, body } => {
                write!(f, "{url} returned HTTP {status}: {}", truncate(body))
            }
            ClientError::Protocol { url, detail, .. } => {
                write!(f, "unexpected response from {url}: {detail}")
            }
            ClientError::Encode(detail) => {
                write!(f, "failed to encode chat request: {detail}")
            }
        }
    }
}

impl std::error::Error for ClientError {}

/// A health-probe error.
#[derive(Debug)]
pub enum HealthError {
    /// The endpoint could not be reached (or did not answer `/models`).
    Unreachable {
        /// The `/models` endpoint probed.
        url: String,
        /// The transport/HTTP detail.
        detail: String,
    },
    /// The endpoint answered but the configured model is not among those served.
    ModelMissing {
        /// The configured model that was not found.
        model: String,
        /// The `/models` endpoint probed.
        url: String,
        /// The model ids the server reported.
        available: Vec<String>,
    },
}

impl fmt::Display for HealthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthError::Unreachable { url, detail } => write!(
                f,
                "inference server unreachable at {url}: {detail} \
                 — is the server running? check `base_url` in .grove/explore.json"
            ),
            HealthError::ModelMissing { model, url, available } => write!(
                f,
                "model `{model}` is not served by {url} (available: {}) \
                 — pull/load it, or fix `model` in .grove/explore.json",
                if available.is_empty() { "none reported".to_string() } else { available.join(", ") }
            ),
        }
    }
}

impl std::error::Error for HealthError {}

fn truncate(s: &str) -> String {
    const MAX: usize = 500;
    if s.len() <= MAX {
        s.to_string()
    } else {
        // Walk back to the nearest char boundary so we never slice through the
        // middle of a multi-byte UTF-8 character (which would panic). Arbitrary
        // server response bodies flow through here, so this must never crash.
        let mut cut = MAX;
        while cut > 0 && !s.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}… ({} bytes)", &s[..cut], s.len())
    }
}

// ---------------------------------------------------------------------------
// Concrete client
// ---------------------------------------------------------------------------

/// The concrete OpenAI-compatible chat client over `ureq` + rustls.
///
/// Construct with [`OpenAiCompatClient::new`] from an [`ExploreConfig`]; it
/// captures the base URL and model and POSTs to `{base_url}/chat/completions`.
pub struct OpenAiCompatClient {
    base_url: String,
    model: String,
    agent: ureq::Agent,
}

impl OpenAiCompatClient {
    /// Build a client from config. The base URL's trailing slash is trimmed so
    /// `{base_url}/chat/completions` is well-formed for both providers.
    pub fn new(cfg: &ExploreConfig) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout(CHAT_TIMEOUT)
            .build();
        OpenAiCompatClient {
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            model: cfg.model.clone(),
            agent,
        }
    }
}

impl ChatClient for OpenAiCompatClient {
    fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ClientError> {
        let mut req = req;
        // The client owns the model identity — it comes from config, not the
        // caller, so a stale/empty request model can't reach the server.
        req.model = self.model.clone();

        let url = format!("{}/chat/completions", self.base_url);
        let body = serde_json::to_string(&req).map_err(|e| ClientError::Encode(e.to_string()))?;

        let resp = self
            .agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&body);

        let resp = match resp {
            Ok(r) => r,
            Err(ureq::Error::Status(status, r)) => {
                let body = r.into_string().unwrap_or_default();
                return Err(ClientError::Http { url, status, body });
            }
            Err(ureq::Error::Transport(t)) => {
                return Err(ClientError::Connection { url, detail: t.to_string() });
            }
        };

        let raw = resp
            .into_string()
            .map_err(|e| ClientError::Connection { url: url.clone(), detail: e.to_string() })?;

        serde_json::from_str(&raw).map_err(|e| ClientError::Protocol {
            url,
            detail: e.to_string(),
            body: raw,
        })
    }
}

// ---------------------------------------------------------------------------
// Health probe
// ---------------------------------------------------------------------------

/// The OpenAI `/models` listing shape (the subset we read).
#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: String,
}

/// Pre-flight reachability + model-availability check.
///
/// GETs `{base_url}/models` (the standard OpenAI listing both providers expose)
/// and confirms the configured model is served. Returns
/// [`HealthError::Unreachable`] if the endpoint can't be reached or doesn't
/// answer, or [`HealthError::ModelMissing`] if it answers but the model is
/// absent. Model matching is tolerant (see [`model_available`]) because
/// llama.cpp may report a file path or alias rather than the configured tag.
pub fn health_probe(cfg: &ExploreConfig) -> Result<(), HealthError> {
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/models");

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .build();

    let resp = agent.get(&url).call().map_err(|e| match e {
        ureq::Error::Status(status, r) => HealthError::Unreachable {
            url: url.clone(),
            detail: format!("HTTP {status}: {}", truncate(&r.into_string().unwrap_or_default())),
        },
        ureq::Error::Transport(t) => {
            HealthError::Unreachable { url: url.clone(), detail: t.to_string() }
        }
    })?;

    let raw = resp
        .into_string()
        .map_err(|e| HealthError::Unreachable { url: url.clone(), detail: e.to_string() })?;

    let listing: ModelsResponse = serde_json::from_str(&raw).map_err(|e| {
        HealthError::Unreachable {
            url: url.clone(),
            detail: format!("unparseable /models response: {e}"),
        }
    })?;

    let available: Vec<String> = listing.data.into_iter().map(|m| m.id).collect();
    if model_available(&cfg.model, &available) {
        Ok(())
    } else {
        Err(HealthError::ModelMissing { model: cfg.model.clone(), url, available })
    }
}

/// List the model ids the provider currently serves (its `/models` listing).
///
/// Powers the `grove config` model dropdown (auto-discovery). Uses the same
/// tolerant, short-deadline GET as [`health_probe`] but returns the raw id list
/// instead of matching one. Any transport / parse failure yields a `String`
/// error so the caller can fall back to free-text entry without blocking on a
/// hard dependency (the local server may simply not be running yet).
pub fn list_models(cfg: &ExploreConfig) -> Result<Vec<String>, String> {
    fetch_models_at(&cfg.base_url, CONNECT_TIMEOUT, PROBE_TIMEOUT)
}

/// GET `{base_url}/models` with explicit timeouts, returning the served model
/// ids. Shared by [`list_models`] (inference-grade deadline) and
/// [`discover_engines`] (a short deadline so a dead local port can't stall the
/// caller). Any transport / parse failure yields a `String` error.
fn fetch_models_at(base_url: &str, connect: Duration, overall: Duration) -> Result<Vec<String>, String> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/models");

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(connect)
        .timeout(overall)
        .build();

    let resp = agent.get(&url).call().map_err(|e| e.to_string())?;
    let raw = resp.into_string().map_err(|e| e.to_string())?;
    let listing: ModelsResponse =
        serde_json::from_str(&raw).map_err(|e| format!("unparseable /models response: {e}"))?;
    Ok(listing
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| !id.is_empty())
        .collect())
}

/// Connect deadline for a discovery probe — short, so the common case (nothing
/// listening on that port → connection refused) returns effectively instantly
/// and a filtered/hung port is capped rather than stalling the config TUI.
const DISCOVER_CONNECT_TIMEOUT: Duration = Duration::from_millis(600);
/// Overall discovery-probe deadline. Long enough for a live local server to
/// answer `/models` (instant on loopback), short enough to stay snappy.
const DISCOVER_PROBE_TIMEOUT: Duration = Duration::from_millis(1200);

/// A well-known local inference server the config TUI probes for auto-detection.
/// Both faces speak the same OpenAI-compatible wire protocol, so the `label` is
/// only a human hint; `base_url` is the server's conventional local default.
#[derive(Debug, Clone)]
pub struct EngineCandidate {
    /// Human label (e.g. `"ollama"`, `"llama.cpp"`).
    pub label: &'static str,
    /// Conventional local OpenAI-compatible base URL for this server.
    pub base_url: &'static str,
}

/// The built-in probe table: the local inference servers grove auto-detects, in
/// display order. Every entry exposes the standard `{base_url}/models` listing.
pub const ENGINE_CANDIDATES: &[EngineCandidate] = &[
    EngineCandidate { label: "ollama", base_url: "http://localhost:11434/v1" },
    EngineCandidate { label: "llama.cpp", base_url: "http://localhost:8080/v1" },
    EngineCandidate { label: "lm-studio", base_url: "http://localhost:1234/v1" },
    EngineCandidate { label: "vllm", base_url: "http://localhost:8000/v1" },
];

/// A probed local inference endpoint: whether it answered `/models`, and the
/// models it serves (empty when it answered with none, or wasn't reachable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredEngine {
    /// Human label carried from the [`EngineCandidate`].
    pub label: String,
    /// The OpenAI-compatible base URL probed.
    pub base_url: String,
    /// `true` when `{base_url}/models` answered.
    pub alive: bool,
    /// The served model ids (empty unless `alive` and the server reported some).
    pub models: Vec<String>,
}

/// Probe every [`ENGINE_CANDIDATES`] endpoint **concurrently** (short deadline)
/// and report which answered. The result preserves candidate order, so callers
/// get a stable list of `{ollama, llama.cpp, lm-studio, vllm}` annotated with
/// liveness + model lists. Never blocks beyond one probe timeout: dead ports
/// (connection refused) return instantly and the slowest live probe bounds the
/// wall time. This is the discovery backing the `grove config` engine picker.
pub fn discover_engines() -> Vec<DiscoveredEngine> {
    let handles: Vec<_> = ENGINE_CANDIDATES
        .iter()
        .map(|c| {
            let label = c.label.to_string();
            let base_url = c.base_url.to_string();
            std::thread::spawn(move || {
                match fetch_models_at(&base_url, DISCOVER_CONNECT_TIMEOUT, DISCOVER_PROBE_TIMEOUT) {
                    Ok(models) => DiscoveredEngine { label, base_url, alive: true, models },
                    Err(_) => DiscoveredEngine { label, base_url, alive: false, models: Vec::new() },
                }
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// Best-effort tolerant model matching.
///
/// An empty listing is treated as a match (some servers report no models even
/// when one is loaded — we don't want a false `ModelMissing`). Otherwise a
/// model is considered available if any reported id equals it, contains it, or
/// shares a file-stem / pre-`:` tag base — covering llama.cpp reporting a
/// `.gguf` path or alias instead of the configured Ollama-style tag.
fn model_available(want: &str, have: &[String]) -> bool {
    if have.is_empty() {
        return true;
    }
    let want_base = want.split(':').next().unwrap_or(want);
    have.iter().any(|id| {
        if id == want || id.contains(want) {
            return true;
        }
        let stem = Path::new(id)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(id);
        stem == want || (!want_base.is_empty() && stem.contains(want_base))
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn unreachable_config() -> ExploreConfig {
        // 127.0.0.1:1 — a reserved, closed port: connections are refused
        // immediately and deterministically, so no live server or network is
        // needed and the test can't flake on DNS/timeouts.
        ExploreConfig {
            base_url: "http://127.0.0.1:1/v1".to_string(),
            model: "test-model".to_string(),
            ..ExploreConfig::default()
        }
    }

    #[test]
    fn discover_engines_returns_every_candidate_in_order() {
        // Probes localhost ports; whatever is (not) running, the result mirrors
        // the candidate table one-for-one, in order — that stable shape is what
        // the config TUI renders. Dead ports come back `alive: false`.
        let found = discover_engines();
        assert_eq!(found.len(), ENGINE_CANDIDATES.len());
        for (got, want) in found.iter().zip(ENGINE_CANDIDATES) {
            assert_eq!(got.label, want.label);
            assert_eq!(got.base_url, want.base_url);
            // A dead endpoint never reports models.
            if !got.alive {
                assert!(got.models.is_empty(), "dead engine must list no models");
            }
        }
    }

    #[test]
    fn chat_request_serializes_lean() {
        let req = ChatRequest::new(vec![
            Message::system("be helpful"),
            Message::user("hello"),
        ]);
        let v: Value = serde_json::to_value(&req).unwrap();
        // Present.
        assert_eq!(v["messages"][0]["role"], "system");
        assert_eq!(v["messages"][1]["role"], "user");
        assert_eq!(v["messages"][1]["content"], "hello");
        // Omitted when unset (lean body both providers accept).
        assert!(v.get("tools").is_none(), "empty tools omitted");
        assert!(v.get("tool_choice").is_none(), "unset tool_choice omitted");
        assert!(v.get("temperature").is_none(), "unset temperature omitted");
        // A user message carries no tool_call_id / tool_calls on the wire.
        assert!(v["messages"][1].get("tool_call_id").is_none());
        assert!(v["messages"][1].get("tool_calls").is_none());
    }

    #[test]
    fn request_with_tools_and_choice_serializes() {
        let tool = Tool::function(
            "search",
            "search the code",
            serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}}),
        );
        let req = ChatRequest::new(vec![Message::user("find foo")])
            .with_tools(vec![tool])
            .with_tool_choice(serde_json::json!("auto"));
        let v: Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["tools"][0]["type"], "function");
        assert_eq!(v["tools"][0]["function"]["name"], "search");
        assert_eq!(v["tools"][0]["function"]["parameters"]["type"], "object");
        assert_eq!(v["tool_choice"], "auto");
    }

    #[test]
    fn tool_message_carries_call_id() {
        let m = Message::tool("call_42", "{\"result\": 1}");
        let v: Value = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "tool");
        assert_eq!(v["tool_call_id"], "call_42");
        assert_eq!(v["content"], "{\"result\": 1}");
    }

    // The two providers encode tool-call `arguments` differently: llama.cpp
    // sends an already-parsed object, Ollama (and the OpenAI spec) sends a
    // JSON-encoded string. Both must normalize to the identical ToolCall.
    const LLAMACPP_RESPONSE: &str = r#"{
        "choices": [{
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "search", "arguments": {"q": "foo", "n": 3} }
                }]
            }
        }]
    }"#;

    const OLLAMA_RESPONSE: &str = r#"{
        "choices": [{
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "search", "arguments": "{\"q\": \"foo\", \"n\": 3}" }
                }]
            }
        }]
    }"#;

    #[test]
    fn provider_dialects_normalize_to_identical_tool_calls() {
        let a: ChatResponse = serde_json::from_str(LLAMACPP_RESPONSE).unwrap();
        let b: ChatResponse = serde_json::from_str(OLLAMA_RESPONSE).unwrap();

        let ta = &a.first_message().unwrap().tool_calls[0];
        let tb = &b.first_message().unwrap().tool_calls[0];

        assert_eq!(ta, tb, "object-args and string-args normalize identically");
        assert_eq!(ta.id, "call_1");
        assert_eq!(ta.name, "search");
        assert_eq!(ta.arguments["q"], "foo");
        assert_eq!(ta.arguments["n"], 3);
    }

    #[test]
    fn empty_and_null_arguments_normalize_to_object() {
        assert_eq!(normalize_arguments(Value::Null), serde_json::json!({}));
        assert_eq!(normalize_arguments(Value::String(String::new())), serde_json::json!({}));
        assert_eq!(normalize_arguments(Value::String("   ".into())), serde_json::json!({}));
    }

    #[test]
    fn unparseable_string_arguments_degrade_gracefully() {
        // A non-JSON arguments string is preserved rather than lost/panicking.
        let got = normalize_arguments(Value::String("not json".into()));
        assert_eq!(got, Value::String("not json".into()));
    }

    #[test]
    fn tool_call_round_trips_through_canonical_shape() {
        let tc = ToolCall {
            id: "call_9".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"q": "bar"}),
        };
        let v: Value = serde_json::to_value(&tc).unwrap();
        // Serialized in canonical OpenAI shape: arguments is a JSON *string*.
        assert_eq!(v["type"], "function");
        assert_eq!(v["function"]["name"], "search");
        assert!(v["function"]["arguments"].is_string());
        // …and deserializing it back yields the same normalized ToolCall.
        let back: ToolCall = serde_json::from_value(v).unwrap();
        assert_eq!(back, tc);
    }

    #[test]
    fn usage_is_parsed_when_present_and_absent() {
        // Present: the token accounting is captured for the trace metrics.
        let with = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}],
            "usage":{"prompt_tokens":120,"completion_tokens":8,"total_tokens":128}}"#;
        let resp: ChatResponse = serde_json::from_str(with).unwrap();
        let u = resp.usage.expect("usage present");
        assert_eq!(u.prompt_tokens, 120);
        assert_eq!(u.completion_tokens, 8);
        assert_eq!(u.total_tokens, 128);

        // Absent: older/leaner responses still parse, usage is None.
        let without = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}]}"#;
        let resp: ChatResponse = serde_json::from_str(without).unwrap();
        assert!(resp.usage.is_none());

        // Partial: a usage object missing a field defaults it to 0, never fails.
        let partial = r#"{"choices":[],"usage":{"prompt_tokens":5}}"#;
        let resp: ChatResponse = serde_json::from_str(partial).unwrap();
        let u = resp.usage.unwrap();
        assert_eq!(u.prompt_tokens, 5);
        assert_eq!(u.completion_tokens, 0);
    }

    #[test]
    fn response_without_tool_calls_is_plain_text() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hi there"}}]}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        let m = resp.first_message().unwrap();
        assert_eq!(m.content.as_deref(), Some("hi there"));
        assert!(m.tool_calls.is_empty());
    }

    #[test]
    fn chat_against_unreachable_url_is_connection_error() {
        let client = OpenAiCompatClient::new(&unreachable_config());
        let err = client
            .chat(ChatRequest::new(vec![Message::user("hi")]))
            .expect_err("a closed port must not yield a response");
        assert!(err.is_connection(), "expected Connection, got {err:?}");
        match err {
            ClientError::Connection { url, .. } => {
                assert!(url.contains("127.0.0.1:1"), "message names the endpoint: {url}");
                assert!(url.ends_with("/chat/completions"));
            }
            other => panic!("expected Connection, got {other:?}"),
        }
    }

    #[test]
    fn health_probe_against_unreachable_url_is_unreachable() {
        let err = health_probe(&unreachable_config())
            .expect_err("a closed port must not pass the health probe");
        match err {
            HealthError::Unreachable { url, .. } => {
                assert!(url.contains("127.0.0.1:1"), "message names the endpoint: {url}");
                assert!(url.ends_with("/models"));
            }
            other => panic!("expected Unreachable, got {other:?}"),
        }
    }

    #[test]
    fn model_matching_is_tolerant() {
        // Exact.
        assert!(model_available("qwen2.5-coder:7b", &["qwen2.5-coder:7b".into()]));
        // llama.cpp reporting a .gguf file path: stem contains the tag base.
        assert!(model_available(
            "qwen2.5-coder:7b",
            &["/models/qwen2.5-coder-7b-instruct.gguf".into()]
        ));
        // Empty listing → treated as available (avoid false ModelMissing).
        assert!(model_available("anything", &[]));
        // Genuinely absent.
        assert!(!model_available("llama3", &["qwen2.5-coder:7b".into()]));
    }

    #[test]
    fn truncate_does_not_panic_on_multibyte_boundary() {
        // A multi-byte char (é = 2 bytes) straddling the 500-byte cutoff must
        // not cause a byte-slice panic. Regression for the code-review blocker.
        let s = format!("{}{}", "a".repeat(499), "é".repeat(50));
        let out = truncate(&s); // must not panic
        assert!(out.ends_with(&format!("({} bytes)", s.len())));
        // Cut point landed on a char boundary strictly below the raw byte max.
        let body = out.split('…').next().unwrap();
        assert!(body.len() <= 500);
        assert!(s.starts_with(body));

        // Short strings pass through unchanged.
        assert_eq!(truncate("héllo"), "héllo");
    }
}
