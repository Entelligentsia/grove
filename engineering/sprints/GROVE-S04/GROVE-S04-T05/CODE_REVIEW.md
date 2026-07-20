# CODE_REVIEW.md — GROVE-S04-T05 (standalone review)

**Verdict:** Approved

Re-review after the prior `Revision Required` (AC7 cold-target blocker). Every
acceptance criterion was re-verified independently against the on-disk working
tree (the T05 change is uncommitted); the prior blocker was reproduced from a
genuinely cold target, not accepted on report.

## Prior blocker — RESOLVED (independently reproduced)

**AC7 — workspace green from a cold `grove-explore` target.** The fix rewrites
`grove_explore_bin()` (cli/tests/cli.rs:58) with `std::sync::Once` + an
on-demand `cargo build --locked -p grove-explore-bin` (honours `$CARGO`,
detects release/debug from the output dir, `bin.exists()` short-circuits warm
runs). I verified this is real, not a report artifact:

1. `cargo build --release --locked -p grove-cst-cli --bin grove` → built.
2. `rm -f target/release/grove-explore` → confirmed absent.
3. `cargo test --release --locked -p grove-cst-cli --test cli` → observed
   `Compiling grove-explore-bin` **mid-run** (the on-demand build firing — the
   cli crate does not cargo-depend on grove-explore-bin, so this is the true
   test of the fix), then **42 passed; 0 failed**; the binary was materialised.
4. `cargo test --release --locked --workspace` → **340 passed, 0 failed**
   (136+65+42+42+54+1+0).
5. `cargo clippy --all-targets --workspace --locked -- -D warnings` → clean
   (exit 0).

`Once::call_once` serialises concurrent harness threads onto one build; the
nested cargo invocation runs during test execution (build lock already
released), a known-safe pattern. This is a robust fix, not a workaround.

## Spec compliance (all re-confirmed)

- **AC1** — config_tui, trace_tui and tap live verbatim in `grove-explore/src/`;
  `main.rs` dispatches `serve`/`config`/`tap`, with bare invocation
  (`.mcp.json` registration) defaulting to `Serve` via
  `cli.cmd.unwrap_or(Cmd::Serve …)`. ✓
- **AC2** — `cli/src/main.rs` Config/Tap arms print `note: \`grove …\` is
  deprecated; use \`grove-explore …\`` to stderr, then exec-forward via
  `find_explore_binary()` (sibling-of-current-exe, PATH fallback), and degrade
  gracefully with an actionable message when the binary is absent. ✓
- **AC3** — `grove-explore/src/tap.rs` does a `GroveConfig` read-modify-write to
  `.grove/config.json`, flipping only `explore.tap` and preserving `mode` /
  `harnesses`; the hint reads "restart the `grove-explore` server"; no
  user-facing string references the deprecated `.grove/explore.json` (remaining
  `explore.json` mentions are code comments about legacy migration). ✓
- **AC4** — `grove_mode` / `explore_active` fields deleted from
  `config_tui/model.rs`; mode badge + explore-notice row + inert-render guards
  removed from `view.rs`; inert unit tests deleted. Only AC4 documentation
  comments remain. ✓
- **AC5** — the config save path (config_tui/mod.rs event_loop) re-reads both
  `mode` and `harnesses` from the on-disk `GroveConfig` before writing, so the
  TUI never forces `mode` to `McpLlm`. `config_round_trip_preserves_mode_and_harnesses`
  (update.rs:399) seeds `mode=mcp` and asserts `readback.mode == Mode::Mcp`. ✓
- **AC6** — independently via `cargo tree`: `ratatui`/`crossterm` do not appear
  in `grove-cst-cli`'s dependency tree (`… did not match any packages`) and are
  present in `grove-explore-bin`. ✓
- **AC7** — reproduced above. ✓

## Advisory notes (non-blocking; not gates)

1. `config_round_trip_preserves_mode_and_harnesses` re-implements the save
   sequence inline rather than driving the real `event_loop` `Action::Save`
   arm. It is a faithful mirror of the production logic today, but a future
   refactor of the save path would not be caught by this test. Consider
   extracting the "rebuild GroveConfig from ExploreConfig + on-disk
   mode/harnesses" step into a named helper both the event loop and the test
   call, so the invariant is tested at its true home.
2. The first-run guard in `init.rs:136` still keys on the presence of
   `.grove/explore.json` as its sentinel. Pre-existing (not a regression from
   this task) and acceptable during the one-release deprecation window; worth
   migrating to a `config.json`-based sentinel when explore.json support is
   dropped.
