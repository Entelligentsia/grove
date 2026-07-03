# GROVE-S02-T01: Explore config model + persistence (`core::explore::config`)

**Sprint:** GROVE-S02
**Estimate:** M
**Pipeline:** default

---

## Objective

Create the shared vocabulary of the mcp-llm subsystem: a typed, serde-backed
configuration model for the inner explorer — provider, endpoint, model, steering
mode, and the allowed-bash-tools allowlist — persisted project-locally. Every
other S02 task (client, agent loop, serve mode, TUI, init) consumes these types,
so this lands first.

## Acceptance Criteria

1. New module `core::explore` (declared `pub mod explore;` in `core/src/lib.rs`)
   with `config.rs` exposing:
   - `ExploreConfig { provider: Provider, base_url: String, model: String, mode: Mode, allowed_tools: Vec<String> }`
   - `enum Provider { Ollama, LlamaCpp }` — OpenAI-compat transport for both;
     Ollama is the default (`http://localhost:11434/v1`).
   - `enum Mode { Standard, Balanced, Aggressive }`.
2. Load/save/validate against the project-local file `.grove/explore.json`
   (sibling to the existing `.grove/grammars/` convention): `ExploreConfig::load(root)`,
   `save(root)`, with atomic write (same pattern as registry writes).
3. Validation fails fast with descriptive, actionable errors (missing file →
   "run `grove init --as mcp-llm` or `grove config`"; bad enum value → names the
   field and the legal values).
4. `Default` impl produces the Ollama defaults; `allowed_tools` defaults to
   `["grove", "rg", "grep", "find"]`.
5. Unit tests (inline `#[cfg(test)] mod tests`): serde round-trip, defaults,
   validation errors, atomic-write behavior.
6. `grove-core` remains clap-free (`cargo tree -p grove-core` shows no clap);
   the string "fastcontext" appears nowhere.
7. `cargo build --release --locked --workspace` warning-clean;
   `cargo clippy -- -D warnings` clean; `cargo test --release --locked` green.

## Context

Sprint requirements D1–D5 (`../SPRINT_REQUIREMENTS.md`). The config file is the
contract between the TUI (T05, writes it), init (T06, provisions it), and
`grove serve` (T04, reads it to select explore mode). Keep the model minimal —
mode-specific steering *content* (prompts) belongs to T03, not the config.

## Artifacts Involved

- New: `core/src/explore/mod.rs` (or `explore.rs`), `core/src/explore/config.rs`.
- Edited: `core/src/lib.rs` (`pub mod explore;`).
- Verify: existing core tests unaffected.

## Operational Impact

- **Version bump:** not required (lands with the sprint's final release bump).
- **Regeneration:** none — no user-facing behavior yet.
- **Backward compat:** additive module; no existing surface touched.
