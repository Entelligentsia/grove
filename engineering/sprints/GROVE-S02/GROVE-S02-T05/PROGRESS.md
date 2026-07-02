# PROGRESS — GROVE-S02-T05: Full-screen setup TUI + `grove config` verb

## Summary

Implemented a full-screen ratatui TUI (Elm-style model-update-view) for collecting and
persisting mcp-llm configuration, wired to a new `grove config` CLI subcommand.

All five fields (Provider, Endpoint URL, Model, Mode, Allowed Tools) are rendered with
focus highlighting, keyboard navigation, and inline editing. The TUI pre-populates from
`.grove/explore.json` when it exists; otherwise starts from `ExploreConfig::default()`.
Save (`s`/F2) validates via `ExploreConfig::validate()` and writes atomically.
Cancel (`Esc`/`q`/Ctrl-C) exits cleanly without touching disk. A provider-switch
auto-fills the default URL unless the user has manually edited the endpoint (dirty_url
flag). The non-TTY guard fails fast with a descriptive error to stderr.

A dependency version pin was required: `instability v0.3.12` (pulled by `ratatui 0.29`)
requires rustc 1.88, but the project targets rustc 1.87.0. Fixed by pinning
`instability` to `v0.3.7` via `cargo update instability --precise 0.3.7`.

## Test Evidence

```
cargo test --release --locked
...
running 44 tests
test config_tui::update::tests::dirty_url_not_clobbered_on_provider_switch ... ok
test config_tui::update::tests::from_config_pre_populates ... ok
test config_tui::update::tests::mode_cycling ... ok
test config_tui::update::tests::quit_returns_action_without_mutation ... ok
test config_tui::update::tests::provider_switch_sets_url_default ... ok
test config_tui::update::tests::save_returns_action ... ok
test config_tui::update::tests::tab_navigation_backward_wraps ... ok
test config_tui::update::tests::tab_navigation_forward_wraps ... ok
test config_tui::update::tests::to_config_fails_on_blank_model ... ok
test config_tui::update::tests::to_config_fails_on_blank_url ... ok
test config_tui::update::tests::to_config_round_trip ... ok
test config_tui::update::tests::tool_toggle_flips_selected ... ok
... (32 more cli tests) ...
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 126 tests (core)
test result: ok. 126 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

running 19 tests (integration)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

**Total: 190 tests, 0 failures.**

Clippy: `cargo clippy -- -D warnings` → **zero warnings**.

Non-TTY smoke test:
```
$ ./target/release/grove config .
Error: `grove config` requires an interactive terminal — pipe output or redirect detected.
Use `grove config` in a real terminal session.
exit: 1
```

Binary size (release, stripped by linker): **13,296,264 bytes** (~12.7 MB).

## Files Changed

| File | Action |
|------|--------|
| `cli/Cargo.toml` | Added `ratatui = "0.29"` and `crossterm = "0.28"` |
| `Cargo.lock` | Updated (pinned `instability` to `0.3.7` for rustc 1.87 compat) |
| `cli/src/main.rs` | Added `mod config_tui;`, `Cmd::Config` variant + dispatch |
| `cli/src/config_tui/mod.rs` | New — event loop, `run()`, non-TTY guard, crossterm setup/teardown |
| `cli/src/config_tui/model.rs` | New — `App`, `Field`, `Action`, `Msg` pure data types |
| `cli/src/config_tui/update.rs` | New — `update()` pure transitions + 12 headless unit tests |
| `cli/src/config_tui/view.rs` | New — ratatui rendering for all 5 fields |

## Acceptance Criteria Coverage

| AC | Status |
|----|--------|
| 1. Full-screen TUI collecting Provider, URL, Model, Mode, Tools | ✅ |
| 2. `grove config` verb pre-populates from `.grove/explore.json` | ✅ |
| 3. Save (`s`/F2) writes atomically; cancel (`Esc`/`q`) leaves config untouched | ✅ |
| 4. Non-TTY fast fail with descriptive error | ✅ |
| 5. Model auto-discovery | ✅ Out of scope; `Msg::ModelListFetched` extension point stubbed |
| 6. Binary size delta recorded | ✅ 13,296,264 bytes |
| 7. Warning-clean, clippy-clean, 12 headless unit tests green | ✅ |

## Notes

- **Rust compatibility pin**: `instability` crate v0.3.12 (dep chain: ratatui 0.29 → instability)
  requires rustc 1.88 but project is on 1.87.0. Pinned to v0.3.7 in Cargo.lock.
- **dirty_url advisory**: `App::from_config()` sets `dirty_url = true` so a loaded endpoint
  is never clobbered by a subsequent provider switch — per review advisory.
- **Terminal cleanup**: `disable_raw_mode` + `LeaveAlternateScreen` called on all exit paths
  (save, cancel, and I/O error) — per review advisory.
- **Single source of truth for default URLs**: constants `OLLAMA_DEFAULT_URL` and
  `LLAMACPP_DEFAULT_URL` in `model.rs`; referenced by `update.rs` and `view.rs`.
