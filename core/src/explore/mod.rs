//! The mcp-llm inner explorer subsystem.
//!
//! - [`config`] — the shared vocabulary persisted to `.grove/explore.json`
//!   (T01).
//! - [`client`] — the OpenAI-compatible chat client + health probe (T02).
//! - [`steering`] — per-mode system prompt text (T03).
//! - [`toolset`] — tool schema registry, gating, and dispatch (T03).
//! - [`agent`] — the bounded inner agent loop [`run_explore`] (T03).

pub mod agent;
pub mod client;
pub mod config;
pub mod grounding;
pub mod steering;
pub mod toolset;

pub use agent::{
    run_explore, run_explore_reporting, ExploreAnswer, ExploreError, NoopReporter, ProgressReporter,
};
pub use client::{
    health_probe, ChatClient, ChatRequest, ChatResponse, ClientError, HealthError, Message,
    OpenAiCompatClient, Role, Tool, ToolCall,
};
pub use config::{ExploreConfig, Mode, Provider};
