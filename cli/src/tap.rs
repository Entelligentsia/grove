//! `grove tap` — turn on in-process explore tracing and browse the recorded
//! sessions in a full-screen TUI.
//!
//! One tool, no proxy: it flips `tap` on in the project's explore config (so
//! `grove serve --explore` records every session to `.grove/traces/`), then opens
//! the trace browser ([`crate::trace_tui`]). `--no-enable` opens the browser
//! without touching the config — handy for read-only inspection.

use std::path::Path;

use anyhow::{Context, Result};

use grove_core::ExploreConfig;

use crate::trace_tui;

/// Enable tracing (unless `no_enable`) and launch the trace browser.
pub fn run(root: &Path, no_enable: bool) -> Result<()> {
    if !no_enable {
        match ExploreConfig::load(root) {
            Ok(mut cfg) if !cfg.tap => {
                cfg.tap = true;
                cfg.save(root)
                    .context("enabling tap in .grove/explore.json")?;
                eprintln!(
                    "grove tap: tracing enabled in .grove/explore.json \
                     — restart `grove serve` for it to take effect"
                );
            }
            Ok(_) => {} // already on
            Err(_) => {
                eprintln!(
                    "grove tap: no .grove/explore.json yet — run `grove init --as mcp-llm` \
                     or `grove config` to set up explore mode; showing any existing traces"
                );
            }
        }
    }
    trace_tui::run(root)
}
