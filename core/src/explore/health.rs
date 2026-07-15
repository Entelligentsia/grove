//! Pre-flight health probe + `/models` listing for the inner explorer.
//!
//! Both faces of grove hit `{base_url}/models` — the standard OpenAI listing
//! Ollama and llama.cpp expose — for two purposes: [`health_probe`] confirms the
//! configured model is served before a run, and [`list_models`] powers the
//! `grove config` model dropdown. [`fetch_models_at`] is the shared, timeout-
//! parameterized GET underneath both (and reused by engine discovery).

use std::fmt;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use super::client::{truncate, CONNECT_TIMEOUT};
use super::config::ExploreConfig;

/// Overall deadline for the lightweight health probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

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
/// [`discover_engines`](super::discovery::discover_engines) (a short deadline so
/// a dead local port can't stall the caller). Any transport / parse failure
/// yields a `String` error.
pub(crate) fn fetch_models_at(
    base_url: &str,
    connect: Duration,
    overall: Duration,
) -> Result<Vec<String>, String> {
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
}
