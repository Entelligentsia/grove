# CODE REVIEW — GROVE-S01-T03: Split init.rs — provision_project (core) vs harness (CLI)

*(standalone review)*

## Verdict: **Approved**

The core/CLI split is correct, the clap-free invariant holds, `grove init` behavior is
byte-for-byte preserved, and all acceptance criteria are met. Verified independently
against the actual working-tree source — not the Engineer's report.

---

## Spec Compliance (AC1–AC6)

| AC | Status | Evidence (independently verified) |
| --- | --- | --- |
| **AC1** — `core/src/init.rs` exposes clap-free `provision_project` | ✅ | `pub fn provision_project(root: &Path, dry_run: bool) -> Result<Vec<String>>`; `pub mod init;` in `core/src/lib.rs`. Scans extensions (`WalkBuilder`), auto-fetches missing (`fetch::run`), writes `grove.lock` (`registry::write_lock_for`). Writes no `.mcp.json`/`CLAUDE.md`. |
| **AC2** — `Vec<String>` consumed by CLI `wrote` | ✅ | Returns `vec!["grove.lock (N grammars)"]` on happy path, empty `Vec` on any short-circuit. CLI does `wrote.extend(provisioned)` so provisioning actions appear last, preserving order. |
| **AC3** — Harness retained CLI-side | ✅ | `Target` enum + `clap::ValueEnum` derive, `write_mcp_json`, `write_claude_md`, `claude_section`, `CLAUDE_*`/`MCP_*` constants all unchanged (not in diff). |
| **AC4** — CLI calls `provision_project` first, then harness | ✅ | `run()` calls `provision_project(root, dry_run)?`, early-returns on empty, else `write_harness(root, target)` then `wrote.extend(provisioned)`. `main.rs` Init arm `init::run(&path, target, dry_run)` unchanged. |
| **AC5** — `grove init` behaves identically per target + `--dry-run` | ✅ | e2e `init_provisions_and_wires_harness_per_target` (mcp / grammars / `--dry-run`) asserts files + stdout shape + `wrote` order. Harness unit tests cover skill/both. See advisory A1. |
| **AC6** — build/test/clippy green; clap-free | ✅ | `cargo build --release --locked --workspace` ✓; `cargo clippy --workspace -- -D warnings` ✓; `cargo test` 87 core + 32 CLI bin + 18 e2e ✓; `cargo tree -p grove-core` → no clap; `core/Cargo.toml` has no clap dep. |

## Correctness

**`wrote` ordering preserved.** Original `write_harness` pushed `.mcp.json`, `CLAUDE.md`,
`grove.lock` in that order. New code pushes harness writes first (`.mcp.json`?, `CLAUDE.md`?)
then `wrote.extend(provisioned)` appends `grove.lock (N grammars)` — identical order. The
e2e test asserts `i_mcp < i_claude < i_lock`.

**Lock round-trip is consistent.** `write_lock_for` writes `{"version":1,"grammars":[{"name":...}]}`
with names `sort()`ed. `locked_langs` reads `doc["grammars"][].name` in file order. The original
path derived `langs` from a `BTreeMap` (sorted keys) → same order reaches `write_claude_md`.
Steering content is therefore byte-identical.

**Short-circuit contract holds.** All four terminals (`no files`, `dry-run`, `no grammars`,
fetch-offline `note`) print inside `provision_project` and return an empty `Vec`; `run()`
early-returns. The narration boundary is identical to the original.

**Disk-write order changed, harmlessly.** The lock is now written *before* the harness files
(in `provision_project`) rather than *after* (in `write_harness`). This is safe because
`write_mcp_json` does not read the lock, and `write_claude_md` now reads it via `locked_langs`
— which is satisfied since `provision_project` wrote it first. The stdout shape depends on the
`wrote` Vec, not disk-write order.

## Design Deviation (sound)

The plan-review assumed `write_claude_md` would continue to receive `langs` as a parameter.
The implementation instead drops the `langs` parameter from `write_harness` entirely and
reads the language list back from the freshly-written lock via the new `registry::locked_langs`.
This is a cleaner decoupling: `provision_project`'s return stays the planned `Vec<String>` of
wrote-actions (not widened to carry a language list), and `write_harness` becomes a pure
harness writer. The dependency on the lock existing is satisfied by the `run()` control flow
(non-empty `provisioned` ⇒ lock written) and by `seed_lock` in the unit tests. Documented in
PROGRESS §Notes/Deviations.

## Test Authenticity

PROGRESS.md test evidence is **authentic** — re-ran all commands independently:
- `cargo build --release --locked --workspace` → Finished, no warnings.
- `cargo clippy --workspace -- -D warnings` → clean (the pruned CLI imports `WalkBuilder`/
  `BTreeMap`/`HashMap`/`{fetch,registry}` and the new core imports resolve).
- `cargo test --release --locked --workspace` → 87 + 32 + 18, 0 failed.
- e2e `init_provisions_and_wires_harness_per_target` is deterministic offline
  (`GROVE_REGISTRY_URL=http://127.0.0.1:1` forces catalog fallback to dev-stub manifests;
  seeded `XDG_CACHE_HOME` makes `is_cached("rust")` pass).

## Security

No new external input surface. `fetch`/`registry` paths are unchanged. `locked_langs` parses
the lock with `serde_json` and accesses by key — no path traversal, no injection. No concerns.

## Advisory Notes (non-blocking)

- **A1 — e2e target coverage.** The plan's testing strategy named all four targets
  (`mcp`/`skill`/`both`/`grammars`) for the e2e case; the implementation covers `mcp`,
  `grammars`, and `--dry-run` at the e2e level. `skill` and `both` are exercised by the
  harness unit tests and share the unchanged `writes_mcp`/`writes_steering` routing, so the
  gap is low-risk. Consider adding `skill`/`both` e2e assertions in a follow-up if the
  orchestration wiring is ever touched again.
- **A2 — `locked_langs` silent empty.** `locked_langs` returns an empty `Vec` if the
  `grammars` key is absent or malformed (`.unwrap_or_default()`). Acceptable since
  `write_lock_for` always emits the key for locks `provision_project` just wrote, but a
  hand-corrupted external lock would silently yield empty steering. Low risk; no action needed.

## Knowledge Writeback

No stack-checklist update required — internal refactor with no new architecture, stack, or
domain patterns. The lock round-trip (`write_lock_for` ↔ `locked_langs`) is a minor new
internal contract but is self-contained within `grove_core`.