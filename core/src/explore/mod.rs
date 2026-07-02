//! The mcp-llm inner explorer subsystem.
//!
//! This task (T01) establishes the [`config`] model — the shared vocabulary
//! persisted to `.grove/explore.json`. The provider client and agent loop land
//! in later S02 tasks and will be namespaced under this module.

pub mod config;

pub use config::{ExploreConfig, Mode, Provider};
