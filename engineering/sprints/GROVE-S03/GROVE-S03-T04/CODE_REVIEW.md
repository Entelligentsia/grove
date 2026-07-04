# CODE_REVIEW — GROVE-S03-T04 (standalone review)

**Verdict:** Approved

`reconcile_harness` refactor lands cleanly. Verified independently against the
approved PLAN.md and the seven acceptance criteria — reading the actual diff of
`cli/src/init.rs`, building, and running the full test suite.

## Spec compliance (AC-by-AC)

1. **AC#1 — single harness writer + strip direction** ✓ `reconcile_harness(root,
   old_mode: Option<Mode>, new_mode: Mode) -> Result<Vec<String>>` present.
   Drives each of `.mcp.json`, `CLAUDE.md`, `AGENTS.md` purely from `new_mode`
   (unconditional write-or-strip per mode), so `old_mode`'s residue is always
   removed. Robust: correctness does not depend on the caller supplying an
   accurate `old_mode`.
2. **AC#2 — load/reconcile/persist** ✓ `run` loads prior `GroveConfig` (old_mode),
   calls `reconcile_harness`, then constructs and `save`s a new `GroveConfig`
   with `mode = new_mode`. `mode` recorded verbatim on every non-dry run.
3. **AC#3 — leaving mcp-llm** ✓ Because reconciliation is mode-exact, any target
   other than McpLlm strips the AGENTS.md grove block and reverts `.mcp.json`
   grove args to `["serve"]` (Mcp/Both) or removes the entry (Skill/Grammars).
   Directly asserted by the transition-matrix `assert_agents_md_consistent` /
   `assert_mcp_json_consistent` helpers for every `McpLlm→B` row.
4. **AC#4 — sentinel-only + host preservation** ✓ `strip_steering_block` operates
   only between `CLAUDE_START`/`CLAUDE_END`; `strip_grove_entry_from_mcp_json`
   removes only the `grove` key from `mcpServers`. `reconcile_harness_preserves_host_content`
   explicitly asserts host CLAUDE.md/AGENTS.md text and a sibling `.mcp.json`
   server survive verbatim.
5. **AC#5 — transition matrix** ✓ `reconcile_harness_transition_matrix` enumerates
   all 20 ordered A→B pairs across the 5 modes, seeding A then transitioning to
   B and asserting `.mcp.json` args, CLAUDE.md block, and AGENTS.md block are all
   consistent with B. `reconcile_harness_then_save_config_active_mode` covers the
   `active_mode` surface-boot consistency and config round-trip.
6. **AC#6 — existing behaviour unchanged** ✓ `cargo test -p grove-cst-cli`:
   87 unit + 29 integration tests pass, including `init_provisions_and_wires_harness_per_target`,
   the mcp-llm idempotency/no-duplicate tests, and `config_in_non_tty_fails_fast`.
   Report order `.mcp.json` → `CLAUDE.md` preserved (match arms ordered).
7. **AC#7 — clean build/lint/test** ✓ `cargo clippy -p grove-cst-cli --all-targets`
   warning-free (the unused `_old_mode` param is underscore-prefixed; no lint).
   Strip paths ensure a single trailing newline; block-only files are emptied,
   not deleted.

## Code quality / architecture

- `Target::to_mode` / `mode_to_target` are clean 1-to-1 bridges between the
  clap-facing enum and the config enum; content selection still flows through
  the existing `write_claude_md`/`write_agents_md` builders — no duplication.
- Dry-run safety verified: `provision_project(root, true)` returns empty and the
  `provisioned.is_empty()` guard short-circuits `run` **before** `reconcile_harness`
  and `save`, so a dry-run never mutates `.mcp.json`, steering files, or config.json.
- Non-TTY guard correctly widened: `already_configured = old_mode == Some(McpLlm)
  || .grove/explore.json exists` — treats a config.json-declared mcp-llm project
  as already provisioned, unblocking CI re-runs.
- explore-config preservation across mode switches is handled (load from TUI-written
  explore.json for McpLlm, else carry forward `old_cfg.explore`).

## Advisory notes (non-blocking)

- `reconcile_harness`'s doc comment says it "drives from on-disk state to new_mode";
  it actually drives purely from `new_mode` (writing the mode's exact harness).
  Wording only — behaviour is correct and stronger than the comment implies.
- `strip_steering_block`'s host-precedes branch uses `after.trim()`, which collapses
  any leading/trailing whitespace of trailing host content to a single `\n\n`
  separator. Acceptable for the single-hop matrix; noted for future multi-block
  scenarios.
