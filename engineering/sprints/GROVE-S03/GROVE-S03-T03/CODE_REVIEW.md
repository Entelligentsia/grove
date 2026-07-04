# GROVE-S03-T03 Code Review (standalone review)

**Verdict:** Approved

Bug-1 (sticky explore surface) is fixed exactly as planned: `serve` no longer
sniffs `.grove/explore.json` existence; it resolves the effective mode from the
declared `config.mode` via a new shared `active_mode(root, ModeChoice) -> Mode`
resolver in `core::config`.

## Independent verification (not taken from PROGRESS.md)

- **Syntax** — `grove check` clean on `core/src/config.rs` and `cli/src/mcp.rs`.
- **Unit tests** — ran `cargo test -p grove-cst active_mode`: all 6 pass
  (`force_standard_returns_mcp`, `force_explore_returns_mcp_llm`,
  `none_reads_declared_mcp_mode`, `none_reads_declared_mcp_llm_mode`,
  `mcp_config_ignores_stale_explore_json` (bug-1 regression),
  `no_config_falls_back_to_mcp`).
- **Integration tests** — ran the two `cli` tests: `bug1_serve_mcp_mode_ignores_stale_explore_json`
  and `explore_mode_unhealthy_provider_falls_back_to_standard_surface` both pass.
  The bug1 test asserts exactly 7 tools AND absence of the `explore` delegating
  tool (matching the plan-review testing recommendation).
- **Lint** — `cargo clippy --release --tests -- -D warnings` clean.

## Correctness / AC coverage

- **AC-1** `active_mode` present in `core::config`; precedence `ForceStandard →
  Mcp`, `ForceExplore → McpLlm`, `None → GroveConfig::load(root).mode`, load
  failure → `Mcp` with stderr diagnostic. Verified in source.
- **AC-2** `determine_surface` keys off `active_mode(root, force) != Mode::McpLlm
  → Standard`. This is the **explicit catch-all** the plan review asked for —
  Skill/Both/Grammars/Mcp all route to Standard, so future `Mode` variants won't
  misroute.
- **AC-3** `mode: "mcp"` + stale `explore.json` → standard surface immediately.
  Confirmed by unit test AM-5 and the serve-level bug1 integration test.
- **AC-4** `mcp-llm` health-gated fallback preserved — `health_probe(&cfg)` path
  in `determine_surface` is unchanged; the migrated-fixture fallback test still
  exercises port-1 connection-refused → Standard.
- **AC-5** `--standard` over `--explore` precedence preserved in the flag→
  `ModeChoice` mapping (`force_standard` checked first).
- **AC-6** all tests green, clippy clean, release build clean.

## Architecture / conventions

- Shared resolver lives in `core::config` and is re-exported from
  `core/src/lib.rs` (`active_mode, GroveConfig, Mode, ModeChoice`) — consistent
  with the existing re-export block. `serve` and `doctor` can both consume it.
- The double `GroveConfig::load` (once inside `active_mode`, once in
  `determine_surface` to fetch the `explore` section) is benign and documented
  in a code comment: on the legacy path the first load migrates + writes
  `config.json`, the second reads it directly, so no double-migration.
- Missing-`explore`-section-under-`mcp-llm` is handled gracefully (diagnostic +
  Standard fallback) rather than panicking.

## Advisory (non-blocking)

1. **AM-1 test fixture has a malformed JSON string** — the `cfg_json` literal in
   `active_mode_force_standard_returns_mcp` ends with a stray trailing `'`
   (`...}}'`), so the written `config.json` is unparseable. The test is still
   *correct* (it asserts `ForceStandard → Mcp`, a path that never reads config,
   and it passes), but the fixture no longer demonstrates "a valid mcp-llm
   config is overridden by ForceStandard" as its comment claims. Consider
   dropping the stray quote so the fixture matches its intent. Cosmetic only —
   does not affect the verdict.
