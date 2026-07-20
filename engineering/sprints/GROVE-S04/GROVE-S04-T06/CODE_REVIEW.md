# Code Review — GROVE-S04-T06 (standalone review)

`init --as mcp-llm` funnel — shell-out to `grove-explore config` + graceful PATH degrade.

This review verifies the **revision** applied after the validation phase returned
`Revision Required`: the degrade-path integration test was not PATH-controlled.
The fix is confined to `cli/tests/cli.rs`; no runtime source (`cli/src/init.rs`)
changed in this iteration.

## Spec Compliance (verified independently)

### AC4 — PATH-controlled degrade test (the required fix)

**Resolved.** The validation finding was that
`init_first_run_grove_explore_absent_degrades` inherited the parent PATH instead
of guaranteeing `grove-explore` is absent. The fix:

- `grove_mcp_llm_with_path` (renamed from `grove_mcp_llm_with_extra_path`) now
  **replaces** the child PATH entirely when `Some(p)` is supplied
  (`cmd.env("PATH", p.as_os_str())`), instead of prepending to the parent PATH.
  When `None` is supplied, the parent PATH is inherited unchanged — preserving
  the behavior every other caller relies on.
- `init_first_run_grove_explore_absent_degrades` creates an empty temp directory
  (`base/empty_path`) and passes it as the sole PATH entry. With PATH pointing
  at an empty directory, `which::which("grove-explore")` cannot resolve the
  binary regardless of the parent environment → the degrade path is exercised
  **deterministically**. The test asserts exit 0, both server registrations in
  `.mcp.json`, the bare `grove-explore` fallback command, and the
  `grove-explore config … finish setup` stdout hint.
- `init_first_run_grove_explore_present_non_tty_fails_fast` passes the
  built-binary directory as the sole PATH entry, so `grove-explore` is
  provably present and the non-TTY fail-fast guard fires.

The rename + semantics change is sound: every existing `grove_mcp_llm` caller
goes through the `None` delegation, so no prior test regressed.

### Remaining ACs (re-confirmed)

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | Both servers registered + two-surface steering | Pass | `reconcile_harness_mcp_llm_writes_mcp_json_explore_and_steering`, `mcp_llm_mcp_json_no_duplicate_grove_entry`, `mcp_llm_steering_block_idempotency`, `mcp_llm_agents_md_created_and_appended` all green. |
| 2 | PATH+TTY shells out to `grove-explore config <root>` | Pass (code-level) | `run()` calls `std::process::Command::new(&bin).arg("config").arg(root)` only when `explore_binary_on_path().is_some()` and `stdout.is_terminal()`. Branch ordering matches the 4-cell matrix (init.rs:142–160). |
| 3 | PATH+non-TTY fails fast with interactive-terminal message | Pass | `init_first_run_grove_explore_present_non_tty_fails_fast` green; guard at init.rs:108 fires before `provision_project`. |
| 4 | Absent-PATH degrades: both servers, exit 0, setup hint | Pass | `init_first_run_grove_explore_absent_degrades` green with deterministic empty-PATH; `explore_command_value()` falls back to bare `grove-explore`; `explore_missing_degrade` set at init.rs:161. |
| 5 | `--dry-run` no shell-out / no writes, previews planned files | Pass | `mcp_llm_dry_run_output_shape` + `mcp_llm_dry_run_twice_is_stable` green; dry-run returns before the first-run block. |
| 6 | Non-mcp-llm modes unchanged | Pass | `init_provisions_and_wires_harness_per_target` + mode-specific unit tests green; 43 cli integration tests green. |
| 7 | `cargo test --release --locked` | Pass | 136 + 65 + 43 + 42 + 54 + 1 = 341 passed, 0 failed (re-run independently). |
| 8 | `cargo clippy --all-targets --workspace --locked -- -D warnings` | Pass | Warning-free (re-run independently). |
| 9 | Modified files end with newline | Pass | `tail -c1` → `0a` for both `cli/src/init.rs` and `cli/tests/cli.rs`. |

## Code Quality

- `explore_binary_on_path() -> Option<PathBuf>` cleanly separates *presence*
  (guard + first-run branch) from `explore_command_value() -> PathBuf` (always
  succeeds, bare-name fallback) — correct split for the guard-vs-writer concern.
- `write_json_mcp_explore_server` and `write_toml_mcp_explore_server` both use
  `explore_command_value()`, so `reconcile_harness_for` completes during the
  degrade path instead of erroring — the core invariant that makes graceful
  degrade work.
- The old `find_explore_binary` is removed from `init.rs`; `main.rs` retains its
  own copy for the deprecated `config`/`tap` shims — consistent with the plan.
- Test helper change is minimal and well-documented: the doc comment on
  `grove_mcp_llm_with_path` states the replace-vs-inherit semantics explicitly.

## Advisories (non-blocking)

1. **Duplicated bail message.** The interactive-terminal message is repeated
   verbatim between the top guard (init.rs:111) and the defensive re-check
   (init.rs:147). A `const` would prevent future drift. (Carried forward from
   prior review; still minor.)
2. **Repeated `which::which` calls.** `explore_binary_on_path()` is invoked up
   to 4× per init run (top guard + first-run block + 2 writer calls via
   `explore_command_value`). Not a correctness issue for a one-shot CLI, but
   caching the probe result once in `run()` would be cleaner. (Carried forward;
   still minor.)

**Verdict: Approved**

The validation-required fix is correctly applied and independently verified:
both PATH-controlled integration tests are deterministic, the workspace is
green, and all nine acceptance criteria hold.