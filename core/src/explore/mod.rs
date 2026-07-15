//! The mcp-llm inner explorer subsystem.
//!
//! The OpenAI-compatible transport is split into four cohesive modules:
//! - [`wire`] — the chat-completions serde model + tool-call normalization.
//! - [`client`] — the [`ChatClient`] transport trait + [`OpenAiCompatClient`].
//! - [`health`] — the `/models` health probe + listing.
//! - [`discovery`] — local inference-engine auto-detection for `grove config`.
//!
//! Around it:
//! - [`config`] — the shared vocabulary persisted to `.grove/explore.json`.
//! - [`steering`] — per-mode system prompt text.
//! - [`toolset`] — tool schema registry, gating, and dispatch.
//! - [`agent`] — the bounded inner agent loop [`run_explore`].

pub mod agent;
pub mod client;
pub mod config;
pub mod discovery;
pub mod grounding;
pub mod health;
pub mod steering;
pub mod toolset;
pub mod trace;
pub mod wire;

pub use agent::{
    run_explore, run_explore_reporting, ExploreAnswer, ExploreError, NoopReporter, ProgressReporter,
};
pub use client::{ChatClient, ClientError, OpenAiCompatClient};
pub use discovery::{discover_engines, DiscoveredEngine, EngineCandidate, ENGINE_CANDIDATES};
pub use health::{health_probe, list_models, HealthError};
pub use wire::{ChatRequest, ChatResponse, Message, Role, Tool, ToolCall, Usage};
pub use config::{ExploreConfig, Provider, Steering};
pub use trace::{SessionMeta, TraceWriter};
