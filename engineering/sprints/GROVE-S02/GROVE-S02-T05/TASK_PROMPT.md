# GROVE-S02-T05: Full-screen setup TUI + `grove config` verb

**Sprint:** GROVE-S02
**Estimate:** L
**Pipeline:** default

---

## Objective

The user-facing setup experience: a solid full-screen TUI that collects the
mcp-llm configuration (provider, endpoint URL, model, mode, allowed bash tools)
and persists it via the T01 config model — launched by `grove init --as mcp-llm`
(T06) and re-launchable at any time as `grove config` to edit the existing
setup.

## Acceptance Criteria

1. A full-screen TUI (ratatui or equivalent maintained crate; dependency lives
   in `cli/` only — core stays TUI-free) collecting:
   - **Provider:** Ollama (default) / llama.cpp — selection list.
   - **Endpoint URL:** text input, pre-filled with the provider default
     (`http://localhost:11434/v1` for Ollama).
   - **Model:** text input.
   - **Mode:** standard / balanced / aggressive selection, each with a
     one-line description of its steering behavior.
   - **Allowed bash tools:** multi-select checkboxes (`grove`, `rg`, `grep`,
     `find` seeded; user-extensible entry for additional binaries).
2. New `grove config` verb: opens the same TUI **pre-populated** from
   `.grove/explore.json`; saving persists via T01 (`ExploreConfig::save`,
   atomic) and takes effect on the next `grove serve` start. If no config
   exists, it starts from defaults (same flow as first-run).
3. Save/cancel semantics: explicit save writes; quit/cancel leaves the
   existing config untouched.
4. **Non-TTY invocation does not hang:** when stdin/stdout is not a terminal,
   `grove config` (and the init-embedded flow) fails fast with a descriptive
   message. No interactive fallback prompts.
5. [Nice-to-have] Model auto-discovery: when the provider endpoint is
   reachable, offer the model list from it (e.g. Ollama `/api/tags` or
   OpenAI-compat `/models`) instead of free-text only.
6. Build/binary impact of the TUI dependency measured and noted in the PR
   description (release binary size before/after; LTO profile per stack
   checklist).
7. Warning-clean, clippy clean, tests green (TUI state-machine logic unit
   tested headlessly — model/update functions, not terminal rendering); no
   "fastcontext" string.

## Context

Depends on **T01** only (config model), so it runs in parallel with the
T02→T03→T04 engine chain. Decision D4: solid TUI + `grove config` re-entry.
Keep the TUI code cleanly separated (state machine vs rendering) so logic is
testable without a terminal — follow the existing "cli formats, core executes"
discipline: validation rules live in T01's config model, the TUI only collects.

## Artifacts Involved

- New: `cli/src/tui.rs` (or `cli/src/config_tui/` module).
- Edited: `cli/src/main.rs` (new `Config` subcommand), `cli/Cargo.toml`
  (TUI dependency).
- Verify: `cargo build --release --locked` binary size delta.

## Operational Impact

- **Version bump:** not required (sprint-final).
- **Regeneration:** none.
- **Backward compat:** new verb; no existing surface touched.
