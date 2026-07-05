# CODE_REVIEW — GROVE-S03-T05: `config` TUI mode badge + inert explore-section rendering

_(standalone review)_

**Verdict:** Approved

## Scope verified

Reviewed the uncommitted working-tree changes (task status `implemented`, not yet
committed) across the six planned files. The merge-base diff conflated T01–T04's
committed work (Mode→Steering rename, `reconcile_harness`); the T05 delta is the
`git diff` of the working tree only, which is what this review assessed.

## Correctness (against PLAN.md + task prompt)

1. **AC1 — mode badge.** `view.rs` outer title now renders
   `" grove config   mode: {…} "` with a match over all five `Mode` variants
   (`mcp` / `skill` / `both` / `mcp-llm ✓` / `grammars`). Reads
   `app.grove_mode`, sourced from `GroveConfig` via `App::from_grove_config`. ✔
2. **AC2 — mcp-llm live.** `Default`/`from_config` set `explore_active = true`
   under McpLlm; all existing edit paths untouched, all 17 pre-existing update
   tests still green. ✔
3. **AC3 — non-mcp-llm inert/greyed + notice.** Every edit `Msg` variant returns
   `None` early when `!explore_active`; `Msg::Save` sets `last_error` and no-ops.
   `render_explore_notice` renders the yellow warning row; every render fn gates
   `focused = app.explore_active && app.focus == …`, so `border_style(false)`
   applies the `DIM` (DarkGray) border to all fields when inactive. Footer swaps
   to the "inactive — Esc to cancel" hint. ✔
4. **AC4 — never mutates mode.** No mode selector added; `grove_mode` is
   read-only in `App`. Save builds `GroveConfig{ mode: app.grove_mode, … }` and,
   because Save is blocked unless McpLlm, mode round-trips unchanged. ✔
5. **AC5 — tests.** 5 new update tests (`badge_reflects_grove_mode`,
   `explore_inert_blocks_all_edits`, `save_blocked_when_inert`,
   `save_allowed_when_mcp_llm`, `tab_and_quit_always_work`) — all pass;
   existing suite unmodified and green. ✔
6. **AC6 — build/clippy/test/newlines.** Independently ran
   `cargo clippy -p grove-cst-cli -- -D warnings` (clean),
   `cargo test -p grove-cst-cli config_tui` (22 passed) and `init::` (30 passed).
   All six touched files end with a newline. ✔

## Architecture & conventions

- Elm-style layering preserved: `grove_mode`/`explore_active` are pure model
  state; the gate lives in `update`; presentation stays in `view`. Clean.
- The three plan-review advisories were all honoured: `view.rs` imports
  `grove_core::config::Mode`; `mod.rs` retains `ExploreConfig` (still used) and
  does not import an unused `Mode`.
- `mod.rs` Save now wraps `to_config()` output into a `GroveConfig`; `main.rs`
  and `init.rs` load `GroveConfig` with graceful fallback to legacy
  `ExploreConfig::load` — backward-compatible with pre-migration projects.

## Security / business rules

No auth/input-injection surface. The load-time preference chain in `init.rs`
(config.json → legacy explore.json → prior cfg) is safe and preserves existing
explore config across mode switches. The inert-save guard correctly prevents an
explore config being written into an active state under a non-explore mode.

## Advisory notes (non-blocking)

- Inactive-field "greying" is conveyed via DIM borders + the yellow notice + the
  footer hint; field *body text* is not additionally dimmed. This satisfies AC3
  as written and is consistent with the existing focused/unfocused styling
  convention — no change required.
