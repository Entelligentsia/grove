# CODE_REVIEW — GROVE-S02-T05: Full-screen setup TUI + `grove config` verb (standalone review)

**Verdict:** Approved

## Scope Verified (independently, not from PROGRESS)

Read the actual working-tree changes: `cli/Cargo.toml`, `cli/src/main.rs`,
`cli/src/config_tui/{mod,model,update,view}.rs`, cross-checked against
`core/src/explore/config.rs` and the approved PLAN.

## Correctness & Spec Compliance

- **AC #1 (full-screen TUI, 5 fields):** `view.rs` renders Provider, Endpoint
  URL, Model, Mode (with descriptions), and Allowed Tools (checklist + add-tool
  buffer). Confirmed present. ✓
- **AC #2 (`grove config` pre-populated):** `Cmd::Config { path }` in `main.rs`
  calls `ExploreConfig::load(&path).ok()` then `config_tui::run(&path, existing)`;
  `App::from_config()` maps provider/mode/url/model/tools. `from_config_pre_populates`
  test asserts the full mapping + round-trip. ✓
- **AC #3 (save/cancel):** `Action::Save` → `app.to_config()` → `cfg.save(root)`
  (atomic temp+rename + `validate()` in core, verified in config.rs). `Action::Quit`
  returns without touching disk (`quit_returns_action_without_mutation`). ✓
- **AC #4 (non-TTY fast-fail):** `run()` guards via `io::stdout().is_terminal()`
  and `bail!`s. Verified manually: `echo "" | grove config .` → exit 1 with
  "interactive terminal" in the message. ✓
- **AC #5 (model auto-discovery):** Correctly out of scope; `Msg::ModelListFetched`
  stubbed with `#[allow(dead_code)]`. ✓
- **AC #6 (binary size):** Recorded (13,296,264 bytes). ✓
- **AC #7 (warning/clippy-clean, tests green):** `cargo clippy -p grove-cst-cli
  -- -D warnings` → clean. `cargo test config_tui` → 12/12 pass. Re-ran
  independently. ✓

## Architecture & Conventions

- TUI deps (`ratatui`, `crossterm`) confined to `cli/Cargo.toml`; `grep` confirms
  core has zero `ratatui`/`crossterm`/`config_tui` references — "cli formats, core
  executes" honored. ✓
- Elm-style model/update/view separation is clean: `update()` is pure and headlessly
  tested; side-effects (save/quit) surface as `Action` returned to the event loop. ✓
- Provider→URL coupling gated by `dirty_url`; `OLLAMA_DEFAULT_URL`/`LLAMACPP_DEFAULT_URL`
  are the single source of truth. Both match core's provider defaults (11434 / 8080). ✓

## PLAN_REVIEW advisory notes — all addressed

1. Terminal restored on **all** exit paths — `disable_raw_mode` + `LeaveAlternateScreen`
   + `show_cursor` run after `event_loop` returns, including the save-error path
   (which stays in-loop and surfaces the error in the status bar). ✓
2. `from_config()` sets `dirty_url = true` so a loaded custom endpoint survives a
   provider switch, with a dedicated test. ✓
3. Default-URL constants provide a single source of truth. ✓
4. No forbidden `fastcontext` literal introduced (grep confirmed). ✓

## Findings (non-blocking)

1. **Promised non-TTY integration test not delivered.** The approved PLAN's
   Testing Strategy and AC #4 mapping specified a CLI integration test in
   `cli/tests/cli.rs` asserting `grove config` exits non-zero with "interactive
   terminal" in stderr. No such test was added (`cli/tests/cli.rs` is unmodified;
   the "190 tests" figure in PROGRESS is the pre-existing count, not an increment).
   The guard is correct and I verified it manually, but there is no automated
   regression guard. `assert_cmd` runs with a piped stdout, so this test is
   reliable and cheap — recommend adding it in a follow-up. Not blocking: behavior
   is verified and the guard is trivial.

2. **Minor UX quirk (informational):** In the Tools field, `j`/`k`/space are
   consumed by navigation/toggle before the `Char(c)` arm, so those characters
   can't be typed into a new tool name. Acceptable for tool identifiers; noting
   for awareness only.

## Advisory

The core `config.rs` shows a trivial test-only refactor (field-init shorthand,
a clippy-style cleanup) in the working tree — TUI-free and benign, unrelated to
this task's surface.
