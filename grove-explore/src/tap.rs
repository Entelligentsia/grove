//! `grove-explore tap` — turn on in-process explore tracing and browse the
//! recorded sessions in a full-screen TUI.
//!
//! One tool, no proxy: it flips `tap` on in the project's explore config (so
//! `grove-explore` records every session to `.grove/traces/`), then opens
//! the trace browser ([`crate::trace_tui`]). `--no-enable` opens the browser
//! without touching the config — handy for read-only inspection.
//!
//! AC3: strings updated from explore.json / grove serve / grove tap to
//! config.json / grove-explore. Tap flag is now written to `.grove/config.json`
//! (via GroveConfig read-modify-write) instead of the legacy `.grove/explore.json`.

use std::path::Path;

use anyhow::{Context, Result};

use grove_core::config::GroveConfig;
use grove_explore_core::ExploreConfig;

use crate::trace_tui;

/// Enable tracing (unless `no_enable`) and launch the trace browser.
pub fn run(root: &Path, no_enable: bool) -> Result<()> {
    if !no_enable {
        // AC3 + AC5: read-modify-write via GroveConfig (.grove/config.json)
        // instead of the legacy ExploreConfig (.grove/explore.json).
        match GroveConfig::load(root) {
            Ok(mut grove_cfg) => {
                // Deserialize the explore section to check/flip tap.
                let mut explore_cfg: ExploreConfig = grove_cfg.explore
                    .as_ref()
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                if !explore_cfg.tap {
                    explore_cfg.tap = true;
                    // Re-serialize back into grove_cfg.explore, preserving all
                    // other fields (mode, harnesses, etc.).
                    grove_cfg.explore = Some(
                        serde_json::to_value(&explore_cfg)
                            .expect("ExploreConfig is always serializable"),
                    );
                    grove_cfg
                        .save(root)
                        .context("enabling tap in .grove/config.json")?;
                    eprintln!(
                        "grove-explore tap: tracing enabled — restart the `grove-explore` \
                         server for it to take effect"
                    );
                }
                // already on — no-op
            }
            Err(_) => {
                eprintln!(
                    "grove-explore tap: no explore config yet — run \
                     `grove init --as mcp-llm` or `grove-explore config` to set up; \
                     showing any existing traces"
                );
            }
        }
    }
    trace_tui::run(root)
}
