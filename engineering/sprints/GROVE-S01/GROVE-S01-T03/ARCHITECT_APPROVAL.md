# ARCHITECT APPROVAL — GROVE-S01-T03: Split init.rs — provision_project (core) vs harness (CLI)

**Verdict:** Approved

## Architectural Review

The implementation cleanly separates grammar provisioning from agent-harness wiring along the core/CLI boundary established by GROVE-S01-T02. This is the correct architectural seam:

- **Core (`core/src/init.rs`):** `provision_project(root: &Path, dry_run: bool) -> Result<Vec<String>>` owns scan + fetch + `grove.lock` — the grammar-system concern. It is clap-free (verified: `core/Cargo.toml` has no clap dep; `cargo tree -p grove-core` clean), takes only `&Path` + `bool`, and writes no harness files. This is a consumable library API.
- **CLI (`cli/src/init.rs`):** retains the inherently CLI-coupled harness — `Target` enum + `clap::ValueEnum` derive, `write_mcp_json`, `write_claude_md`, and the `CLAUDE_*`/`MCP_*` constants. `run()` delegates provisioning to core, early-returns on the empty-Vec short-circuit contract, then writes harness glue.

### Contract soundness
The empty-`Vec` short-circuit contract is well-designed: all four terminal paths (no files, dry-run, offline-missing, no-cached-grammars) print their narration inside `provision_project` and return `Ok(Vec::new())`, telling the CLI to stop without printing duplicate trailers. The happy path returns `vec!["grove.lock (N grammars)"]`, which the CLI consumes via `wrote.extend(provisioned)` — preserving the user-facing `wrote` order: `.mcp.json` → `CLAUDE.md` → `grove.lock`.

### Design deviation (sound)
`write_harness` reads the lock via `registry::locked_langs(root.join("grove.lock"))` instead of taking a `langs` parameter. This is cleaner decoupling than the plan's pass-through: the harness sources steering langs from the lock the core just wrote, so the two halves communicate through the on-disk artifact rather than an in-memory slice. Control flow guarantees the lock exists before `write_harness` is called (provision succeeds → non-empty Vec → harness write).

## Cross-Cutting Concerns

- **No other modules impacted.** The only new public surface is `pub mod init` in `lib.rs` and `pub fn locked_langs` in `registry.rs`. Both are additive.
- **`locked_langs` silent-empty on missing `grammars` key** (advisory A2 from code review): low risk — `write_lock_for` always emits the `grammars` array, so the round-trip is sound. Acceptable for this sprint.

## Operational Impact

- **Distribution:** No user action required. `grove init` behavior and all written artifacts (`grove.lock`, `.mcp.json`, `CLAUDE.md`) are byte-for-byte unchanged.
- **Version bump:** Not required — internal refactor, no CLI surface or stored-format change.
- **Migration:** None.
- **Security:** No new external input surface; fetch/registry paths unchanged.

## Independently Verified

- `cargo build --release --locked --workspace` — green
- `cargo clippy --workspace -- -D warnings` — clean
- `cargo test --release --locked --workspace` — 137 tests pass (87 core + 32 CLI bin + 18 e2e)
- `cargo tree -p grove-core` — clap-free confirmed
- AC4 e2e regression guard (`init_provisions_and_wires_harness_per_target`) present at `cli/tests/cli.rs:323`

## Deployment Notes

No deployment changes. The refactor is internal; the published `grove` binary behaves identically.

## Follow-Up Items

1. **Advisory A1 — e2e target coverage:** The e2e test covers `mcp`/`grammars`/`--dry-run` directly; `skill`/`both` are covered by unit tests + unchanged routing. A future sprint may add direct e2e coverage for these two targets for completeness, but it is not blocking.
2. **`locked_langs` robustness:** Consider returning an error or warning if the `grammars` key is absent in a lock file not produced by `write_lock_for`, to surface corruption early. Low priority.

## Sign-Off

The implementation is consistent with the project's core/CLI architecture, introduces no cross-cutting risk, preserves `grove init` behavior byte-for-byte, and is independently verified green across build/lint/test. Approved for commit.