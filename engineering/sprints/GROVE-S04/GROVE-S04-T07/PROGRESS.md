# PROGRESS — GROVE-S04-T07: `doctor` re-keying

## Summary

Implemented the approved plan:

1. **Re-keyed the explore check group** in `core/src/doctor.rs` to run only when a `grove-explore` server is registered in any selected harness, instead of when `mode == McpLlm`.
2. **Added harness-agnostic registration detection** (`has_explore_registration`) that scans Claude Code `.mcp.json`, Cursor `.cursor/mcp.json`, Codex `~/.codex/config.toml`, Gemini `.gemini/settings.json`, Windsurf `.windsurf/mcp.json`, and VS Code `.vscode/mcp.json` using existing `HarnessId` format metadata.
3. **Extended per-harness drift checks** to validate both the structural `grove` entry and the explore `grove-explore` entry against the declared mode, including a stale-layout check for any `grove` entry still carrying the removed `--explore` argument.
4. **Updated `harness_serve_surface`** to report based on explore-registration presence rather than config-mode inference.
5. **Removed orphaned `--explore` / `--standard` flags** from `grove doctor` in `cli/src/main.rs`.
6. **Conservatively pruned `ModeChoice`** in `core/src/config.rs`: dropped only `ForceExplore`/`ForceStandard`, kept `ModeChoice::None` and `active_mode`, preserving the public API for `core/src/lib.rs` and `cli/src/init.rs`.
7. **Refreshed tests** in `core/src/doctor.rs` and `cli/tests/cli.rs` for registration gating, multi-harness/non-Claude `mcp-llm` fixtures, stale-layout detection, unexpected explore entries, and missing explore entries.

## Test Evidence

```text
$ cargo test --release --locked
... 139 core tests passed; 0 failed
... 65 cli unit tests passed; 0 failed
... 43 cli integration tests passed; 0 failed
... 42 grove-explore tests passed; 0 failed
... 54 grove-explore-core tests passed; 0 failed
```

Full output showed all suites green, including the new tests:
- `doctor::tests::mcp_llm_stale_layout_single_registration_is_fail`
- `doctor::tests::mcp_mode_with_cursor_explore_registration_is_fail`
- `doctor::tests::mcp_llm_cursor_only_explore_registration_triggers_explore_group`
- `doctor::tests::mcp_llm_without_explore_registration_skips_explore_group`
- `doctor::tests::mcp_llm_missing_explore_entry_is_fail`
- `doctor_help_documents_verb` (asserts `--explore` / `--standard` absent from help)

```text
$ cargo clippy --all-targets --workspace --locked -- -D warnings
Finished `dev` profile (unoptimized + debuginfo) target(s)
```

No warnings or errors.

## Files Changed

| File | Change |
|---|---|
| `core/src/doctor.rs` | Re-keyed explore group trigger, added `has_explore_registration`/`has_stale_explore_arg`/`read_server_args`, extended harness drift checks for `grove` + `grove-explore`, added stale-layout detection, updated `harness_serve_surface`, refreshed/added unit tests. |
| `core/src/config.rs` | Removed `ModeChoice::ForceExplore`/`ForceStandard`, simplified `active_mode`, removed obsolete force-variant unit tests. |
| `cli/src/main.rs` | Removed `--explore` / `--standard` fields from `Cmd::Doctor` and deleted the `force` plumbing. |
| `cli/tests/cli.rs` | Strengthened `doctor_help_documents_verb` to assert removed flags are not documented. |

## Acceptance Criteria

- [x] Explore checks run only when a selected harness registers `grove-explore`.
- [x] Structural checks, `--json` shape, and exit-code gate are preserved.
- [x] Drift matrix understands two-registration layout and stale single-registration layout.
- [x] `grove doctor` no longer accepts `--explore` or `--standard`.
- [x] `cargo test --release --locked` passes.
- [x] `cargo clippy --all-targets --workspace --locked -- -D warnings` passes.
- [x] All modified files end with a newline.
