# PLAN_REVIEW — GROVE-S02-T05: Full-screen setup TUI + `grove config` verb

_(standalone review)_

**Verdict:** Approved

## Independent Verification

I did not take the plan at its word — I read the actual T01 config model
(`core/src/explore/config.rs`) and `cli/src/main.rs` and confirmed every
integration assumption:

- `Provider` enum = `Ollama` / `LlamaCpp` with `Provider::LEGAL` slice — matches the plan's `provider: usize` index model. ✓
- `Mode` enum = `Standard` / `Balanced` / `Aggressive` with `Mode::LEGAL`. ✓
- `ExploreConfig::default()` → Ollama, `http://localhost:11434/v1`; llama.cpp default `http://localhost:8080/v1` (confirmed in config test data, line 234). Both URLs the plan hard-codes for the provider→URL coupling are correct. ✓
- `save(&self, root)` writes atomically (temp file + `fs::rename`) and calls `validate()` internally; `validate()` rejects empty `base_url` **and** empty `model`; `load()` and `default()` exist as the plan assumes. ✓
- `allowed_tools: Vec<String>` matches the checkbox model. ✓
- `main.rs` uses a clap-derive `enum Cmd` with per-verb subcommands — `Cmd::Config` slots into the established pattern cleanly. ✓

## Assessment by Category

- **Correctness / feasibility:** The Elm-style model/update/view split is sound and makes the state machine headlessly testable exactly as AC #7 requires. All 5 required fields (AC #1) are covered by concrete widgets.
- **Architecture:** Honors the "cli formats, core executes" discipline — TUI deps (`ratatui`/`crossterm`) confined to `cli/Cargo.toml`, validation delegated to T01's `validate()`. Core stays TUI-free. ✓
- **Testing:** Nine headless `update()` tests plus a non-TTY CLI integration test. Test #8 (blank model or URL fails) is valid because `validate()` guards both fields. Good coverage of the coupling logic and round-trip.
- **AC coverage:** 1–4 fully addressed; #5 correctly scoped out as nice-to-have with a dead-code extension point; #6 measured in PR description; #7 addressed via clippy/tests + `#[allow(dead_code)]` for the stub.

## Advisory Notes (non-blocking — address during implementation)

1. **Terminal restore on error/panic.** The plan's event loop enters raw mode + alternate screen but does not describe restoring the terminal on the error path (e.g. if `save()` returns `Err` mid-loop). Wrap the loop so `disable_raw_mode` + `LeaveAlternateScreen` run on **every** exit path (a drop-guard or explicit restore before propagating `Err`). Otherwise a save failure could leave the user's terminal corrupted.

2. **`dirty_url` initialization in `App::from_config()`.** For the `grove config` re-entry flow (AC #2), a pre-populated config may carry a custom `base_url`. Ensure `from_config()` initializes `dirty_url = true` (or otherwise treats a non-default loaded URL as user-owned) so the first provider switch does not clobber the user's existing custom endpoint. Add a test asserting this to the existing #2/#9 test coverage.

3. **llama.cpp default URL duplication.** `http://localhost:8080/v1` is hard-coded in the TUI's coupling logic but has no single source of truth in `core` (only Ollama's default lives in `ExploreConfig::default()`). Minor duplication is acceptable, but consider a small `Provider::default_base_url()` helper in T01's module so both providers' defaults live in one place.

4. **"fastcontext" string check (AC #7).** The plan doesn't explicitly call out the forbidden-string gate. Confirm no `"fastcontext"` literal is introduced (naming/labels use "mcp-llm" / "explore" vocabulary).
