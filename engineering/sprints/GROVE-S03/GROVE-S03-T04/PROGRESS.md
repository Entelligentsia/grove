# PROGRESS — GROVE-S03-T04: `reconcile_harness` single harness writer + transition-matrix test

## Summary

Implemented the full `reconcile_harness` refactor as specified in the approved plan.
Key deliverables:

1. **`strip_steering_block(root, filename)`** — removes the `<!-- grove:start -->…<!-- grove:end -->` block from a file, stripping the `\n\n` separator `write_steering_md` inserts, ensuring a single trailing newline, and leaving empty files empty (not deleted). No-ops when file is absent or has no grove block.

2. **`strip_grove_entry_from_mcp_json(root)`** — removes the `"grove"` key from `.mcp.json`'s `mcpServers`, preserving all other servers. No-ops when file is absent or has no grove entry.

3. **`Target::to_mode(self) -> Mode`** — bridges CLI `Target` enum (clap-facing) to config `Mode` enum (config-facing). 1-to-1 mapping, added to the `Target` impl block.

4. **`mode_to_target(mode: Mode) -> Target`** — inverse bridge used internally by `reconcile_harness` to select the correct `write_claude_md` content variant.

5. **`reconcile_harness(root, old_mode, new_mode)`** — single harness writer. Drives all three artifacts (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`) toward `new_mode`:
   - `Mcp`/`Both` → write `.mcp.json [serve]` + write CLAUDE.md + strip AGENTS.md
   - `Skill` → strip `.mcp.json` grove entry + write CLAUDE.md + strip AGENTS.md
   - `McpLlm` → write `.mcp.json [serve, --explore]` + write CLAUDE.md + write AGENTS.md
   - `Grammars` → strip `.mcp.json` + strip CLAUDE.md + strip AGENTS.md
   - Returns only the written artifacts list (stripped silently)

6. **`run` updated** — loads prior `GroveConfig` (old_mode), improves the non-TTY guard (also checks `config.json` having `mcp-llm` mode), calls `reconcile_harness`, saves `GroveConfig` with the new mode after provision/TUI, preserving `explore` config across mode switches.

7. **`write_harness` removed** — replaced by `reconcile_harness`; all former callers in `mod tests` updated to `reconcile_harness(dir, None, Mode::*)`.

8. **New tests added** (all in `mod tests`):
   - `strip_steering_block_removes_block_leaves_host_content`
   - `strip_steering_block_empties_file_when_block_only`
   - `strip_steering_block_noop_when_no_block`
   - `strip_steering_block_noop_when_absent`
   - `strip_grove_entry_from_mcp_json_removes_grove_key`
   - `strip_grove_entry_from_mcp_json_noop_when_absent`
   - `strip_grove_entry_from_mcp_json_noop_when_no_grove_key`
   - `reconcile_harness_transition_matrix` — 20 ordered (A→B) pairs across 5 modes
   - `reconcile_harness_preserves_host_content` — host content safety across all 3 files
   - `reconcile_harness_then_save_config_active_mode` — config.json write + active_mode round-trip

## Test Evidence

```
running 87 tests
...
test init::tests::reconcile_harness_transition_matrix ... ok
test init::tests::reconcile_harness_preserves_host_content ... ok
test init::tests::reconcile_harness_then_save_config_active_mode ... ok
test init::tests::strip_steering_block_removes_block_leaves_host_content ... ok
test init::tests::strip_steering_block_empties_file_when_block_only ... ok
test init::tests::strip_steering_block_noop_when_no_block ... ok
test init::tests::strip_steering_block_noop_when_absent ... ok
test init::tests::strip_grove_entry_from_mcp_json_removes_grove_key ... ok
test init::tests::strip_grove_entry_from_mcp_json_noop_when_absent ... ok
test init::tests::strip_grove_entry_from_mcp_json_noop_when_no_grove_key ... ok
test init::tests::grammars_target_writes_no_harness_files ... ok
test init::tests::skill_target_writes_steering_but_no_mcp_json ... ok
test init::tests::mcp_target_writes_mcp_json_and_steering ... ok
test init::tests::both_target_writes_mcp_json_and_steering ... ok
test init::tests::reconcile_harness_mcp_llm_writes_mcp_json_explore_and_steering ... ok

test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s

     Running tests/cli.rs (target/debug/deps/cli-eb29f6a98d0dbd43)

running 29 tests
...
test init_provisions_and_wires_harness_per_target ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.44s
```

Core tests also pass (161 + 1 doc-test).

Clippy: `cargo clippy -p grove-cst-cli -- -D warnings` → clean.

## Files Changed

- `cli/src/init.rs` — all changes (new helpers, reconcile_harness, updated run, new tests)
