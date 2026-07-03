# Progress — GROVE-S02-T07: Integration tests, naming guard, docs

## Summary

Revised implementation addressing the blocking code-review finding: `core/src/explore/steering.rs`, `toolset.rs`, and the updated `mod.rs` / `config.rs` were missing from `files_changed` in the previous implementation summary. These four files — which contain the core::explore agent subsystem — are present on disk and compile cleanly, but without them in `files_changed` the `commit-task` choreography would not stage them, leaving the sealed tree incomplete.

**Fix applied:** Updated `IMPLEMENTATION-SUMMARY.json` to add all four missing files to `files_changed`. All gates re-verified on the working tree.

---

## What Was Done

### Code-Review Blocking Fix (revision scope)

The code review identified that:
- `core/src/explore/steering.rs` — UNTRACKED at HEAD
- `core/src/explore/toolset.rs` — UNTRACKED at HEAD  
- `core/src/explore/mod.rs` — Modified (wired steering + toolset modules) but absent from files_changed
- `core/src/explore/config.rs` — Modified (allowed_tools field) but absent from files_changed

All four files were already implemented and working in the prior implementation phase. The only defect was that the `IMPLEMENTATION-SUMMARY.json::files_changed` array did not list them. The `commit-task.cjs` stages exclusively from that list, so omitting these files would silently drop the inner explorer subsystem from the commit.

**Fix:** Rewrote `IMPLEMENTATION-SUMMARY.json` with the complete 13-file manifest including all four `core/src/explore/` entries.

---

## AC Summary (all unchanged from prior implementation)

| AC | Test / Change | Status |
|---|---|---|
| AC1a — `grove config` non-TTY | `config_in_non_tty_fails_fast` in `cli/tests/cli.rs` | ✅ |
| AC1b — dry-run twice stable | `mcp_llm_dry_run_twice_is_stable` in `cli/tests/cli.rs` | ✅ |
| AC1b — `.mcp.json` dedup | `mcp_llm_mcp_json_no_duplicate_grove_entry` in `cli/tests/cli.rs` | ✅ |
| AC1c — allowlist enforcement | `allowlist_enforcement_find_refused` in `core/src/explore/agent.rs` | ✅ |
| AC2 — naming guard | `naming_guard_no_fastcontext_in_source` in `cli/tests/cli.rs` | ✅ |
| AC3 — README mcp-llm section | `README.md` new section | ✅ |
| AC3 — CHANGELOG | `CHANGELOG.md` `[0.3.0]` section | ✅ |
| AC3 — CLAUDE.md | `CLAUDE.md` architecture + commands update | ✅ |
| AC4 — stack-checklist pass | Documented in PR description | ✅ |
| AC5 — full gates green | `cargo build`, `cargo clippy`, `cargo test` all green | ✅ |

---

## Test Evidence

### `cargo build --release --locked`
```
Finished `release` profile [optimized] target(s) in 0.08s
```

### `cargo clippy -- -D warnings`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

### `cargo test --release --locked`
```
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

Total: **205 tests, 0 failures.**

---

## Files Changed

| File | Change |
|---|---|
| `cli/tests/cli.rs` | 4 new integration tests (AC1a, AC1b×2, AC2) |
| `core/src/explore/agent.rs` | 1 new unit test: `allowlist_enforcement_find_refused` (AC1c) |
| `core/src/explore/mod.rs` | Wired `steering` + `toolset` sub-modules; re-exports |
| `core/src/explore/config.rs` | `allowed_tools` field on `ExploreConfig` |
| `core/src/explore/steering.rs` | Per-mode system prompt strings; `balanced_phase2_prompt`; 2 unit tests |
| `core/src/explore/toolset.rs` | Tool schema registry; `is_in_toolset`; `dispatch_tool`; 7 unit tests |
| `README.md` | New "Delegated local-LLM mode" section (AC3) |
| `CHANGELOG.md` | New `[0.3.0]` dated section (AC3 + version bump) |
| `CLAUDE.md` | Architecture map + Commands block updated (AC3) |
| `cli/Cargo.toml` | Version `0.2.0 → 0.3.0`; `grove-cst` pin updated |
| `core/Cargo.toml` | Version `0.2.0 → 0.3.0` |
| `dist/npm/package.json` | Version `0.2.0 → 0.3.0` |
| `Cargo.lock` | Refreshed |
