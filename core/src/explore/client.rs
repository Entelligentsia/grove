//! The OpenAI-compatible chat **transport** for the inner explorer.
//!
//! This is the seam between the inner explorer agent loop and a local inference
//! server — either **Ollama** or a **llama.cpp** server. Both speak the OpenAI
//! chat-completions wire protocol (modeled in [`super::wire`]), so a single
//! non-streaming client serves both.
//!
//! Two concerns live here:
//!
//! 1. **Transport trait** — [`ChatClient`] is the seam the agent loop depends on
//!    (never the concrete type), so a fake can be substituted in tests.
//!    [`OpenAiCompatClient`] is the real implementation over `ureq` + rustls
//!    (the same blocking HTTP stack `fetch.rs` uses — no new dependency).
//! 2. **Typed errors** — [`ClientError`] separates
//!    [`Connection`](ClientError::Connection) (the D3 shutdown signal — server
//!    unreachable / refused / timed out) from protocol/HTTP errors.
//!
//! The `/models` health probe + listing lives in [`super::health`]; engine
//! discovery in [`super::discovery`]; the wire types in [`super::wire`].

use std::fmt;
use std::time::Duration;

use super::config::ExploreConfig;
use super::wire::{ChatRequest, ChatResponse};

/// Connect-phase timeout — a refused/unroutable server fails fast rather than
/// hanging the agent loop. Shared with the health probe in [`super::health`].
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Overall request deadline for a chat completion. Generous, because local
/// models can take a while to generate; a breach surfaces as a transport error
/// and is therefore classified as [`ClientError::Connection`] (a D3 signal).
const CHAT_TIMEOUT: Duration = Duration::from_secs(300);

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

/// Truncate an arbitrary server response body to a bounded, char-safe snippet
/// for error messages. Shared with [`super::health`]'s probe error paths.
pub(crate) fn truncate(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::wire::Message;

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
