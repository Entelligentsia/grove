# Validation Report — GROVE-S04-T06 (standalone review)

## Executive Summary

The required fix from the previous validation round has been applied and verified. The degrade-path integration test now controls the child `PATH` deterministically by setting it to an empty temporary directory, so `grove-explore` is provably absent. The complementary non-TTY fail-fast test uses only the build-artifacts directory on `PATH`, so `grove-explore` is provably present. All acceptance criteria now pass, and the workspace remains green.

**Verdict:** Approved

## Acceptance Criteria

| # | Criterion | Verdict | Evidence |
| 1 | `init --as mcp-llm` registers both servers and writes the two-surface steering unchanged from T04. | **Pass** | Unit test `reconcile_harness_mcp_llm_writes_mcp_json_explore_and_steering` passes; integration tests `mcp_llm_mcp_json_no_duplicate_grove_entry`, `mcp_llm_steering_block_idempotency`, and `mcp_llm_agents_md_created_and_appended` pass. `reconcile_harness_for` writes both `grove` and `grove-explore` registrations. |
| 2 | When `grove-explore` is on PATH and stdout is a TTY, `init` shells out to `grove-explore config <root>`. | **Pass (code-level)** | `run()` calls `std::process::Command::new(&bin).arg("config").arg(root)` only when `explore_binary_on_path()` is `Some` and `stdout.is_terminal()`. Not directly testable in the non-TTY subprocess harness, but the branch is present and correctly ordered. |
| 3 | When `grove-explore` is on PATH but stdout is not a TTY, `init` fails fast with the existing interactive-terminal message. | **Pass** | Integration test `init_first_run_grove_explore_present_non_tty_fails_fast` passes: sets PATH to only the built `grove-explore` binary directory, asserts non-zero exit, and checks stderr for "interactive terminal". |
| 4 | When `grove-explore` is not on PATH, `init` degrades gracefully: registers both servers, exits 0, and prints "run `grove-explore config` to finish setup". | **Pass** | Integration test `init_first_run_grove_explore_absent_degrades` is now PATH-controlled via `grove_mcp_llm_with_path`, which replaces the child's entire PATH with an empty temporary directory. The test asserts exit 0, both server registrations in `.mcp.json`, the bare `grove-explore` fallback command value, and the setup hint in stdout. Code inspection confirms `explore_command_value()` falls back to bare `grove-explore` and both `write_json_mcp_explore_server` and `write_toml_mcp_explore_server` use it. |
| 5 | `--dry-run` performs no shell-out and no writes, and still previews planned files. | **Pass** | `run()` skips the first-run guard and returns early when `dry_run` is true, before the shell-out / reconcile block. Tests `mcp_llm_dry_run_output_shape` and `mcp_llm_dry_run_twice_is_stable` assert no files are written, stable output, and that `grove-explore` is previewed. |
| 6 | Existing `init --as mcp|skill|both|grammars` behavior is unchanged; existing `tests/cli.rs` init assertions continue to pass. | **Pass** | `init_provisions_and_wires_harness_per_target` and all mode-specific unit tests pass; the 43 cli integration tests are green. |
| 7 | `cargo test --release --locked` passes. | **Pass** | `cargo test --release --locked` completed successfully: 136 core + 65 cli unit + 43 cli integration + 42 grove-explore bin + 54 grove-explore core + 1 doc-test = 341 tests. |
| 8 | `cargo clippy --all-targets --workspace --locked -- -D warnings` passes warning-free. | **Pass** | Clippy completed with no warnings. |
| 9 | All modified source files end with a newline. | **Pass** | `tail -c1` shows `0a` for both `cli/src/init.rs` and `cli/tests/cli.rs`. |

## Additional Notes

- The duplicated interactive-terminal bail message in `run()` is a minor maintainability issue but does not affect correctness; consider extracting it to a constant in a follow-up.
- `explore_binary_on_path()` is called multiple times per run (guard, TUI block, writers); while acceptable for a one-shot CLI, caching the result would be a small future improvement.

## Validation Commands Run

```bash
cargo test --release --locked
cargo clippy --all-targets --workspace --locked -- -D warnings
```
