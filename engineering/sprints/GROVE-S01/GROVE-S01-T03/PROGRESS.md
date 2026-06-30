# PROGRESS — GROVE-S01-T03: Split init.rs — provision_project (core) vs harness (CLI)

## Summary

Split `cli/src/init.rs` along the seam "what touches only grammars/lock" (core)
vs "what writes agent files" (CLI), per the approved PLAN.md:

- **New `core/src/init.rs`** exposes the clap-free
  `pub fn provision_project(root: &Path, dry_run: bool) -> anyhow::Result<Vec<String>>`.
  It owns the extension→language map (`extension_map`), per-language file counting,
  the dry-run / no-files / no-cached-grammars short-circuits, auto-fetch of missing
  grammars, the cached-language filter, and the `grove.lock` write
  (`registry::write_lock_for`). It prints exactly the provisioning narration it
  printed before (`detected …`, the `(dry run …)` note, `fetching …`, the offline
  `note …`, and the `no files matched` / `no grammars available` terminals). The
  helpers `extension_map` and `is_cached` moved here alongside it.
- **`core/src/lib.rs`** gains `pub mod init;`, exposing provisioning to library
  consumers and the CLI.
- **`cli/src/init.rs::run()`** reduced to: print the `grove init  scanning … (as
  {target})` header, call `provision_project`, return early if it returns an empty
  `Vec` (the short-circuit contract — core already printed its terminal line),
  otherwise write the harness for the chosen `Target` and assemble the combined
  `wrote` block as **harness-writes followed by provision-actions** (preserving
  today's order: `.mcp.json`, `CLAUDE.md`, `grove.lock`).
- **`cli/src/init.rs::write_harness`** reduced to harness-only (`.mcp.json` +
  `CLAUDE.md`); it no longer writes the lock and no longer takes a `langs` slice.
- **`core/src/registry.rs`** gains `pub fn locked_langs(path) -> Result<Vec<String>>`,
  a reader for the just-written `grove.lock`. The lock is the canonical list of the
  grammars a project needs; the CLI reads it back to name those languages in the
  CLAUDE.md steering block (the data the old `write_harness(langs)` parameter
  carried, now sourced from the lock the core wrote — keeping `provision_project`'s
  return type the planned `Vec<String>` of wrote-actions).

### Contract

`provision_project` returns the provisioning wrote-actions on the happy path
(`["grove.lock (N grammars)"]`) and an **empty `Vec`** on any short-circuit
(dry-run / no files / no cached grammars), telling the CLI to stop. With the
`Grammars` target the CLI writes no harness files, so the `wrote` list is exactly
`provision_project`'s return — the clean correctness check the task notes.

### clap-free invariant

`grove-core` stays clap-free: `provision_project` takes `(&Path, bool)` — no
`Target`, no `ValueEnum`. The `Target` enum (+ `clap::ValueEnum` derive),
`write_mcp_json`, `write_claude_md`, `claude_section`, and the `CLAUDE_*` / `MCP_*`
constants stay CLI-side. Verified via `cargo tree -p grove-core` (no clap).

### Behavior unchanged (AC4)

`grove.lock`, `.mcp.json`, and `CLAUDE.md` outputs are byte-for-byte unchanged; the
stdout narration boundary is identical (provision narrates detected/fetching/
dry-run/terminals; the CLI prints header + wrote/ready/skill/grammars trailers in
today's order). Not a material/version-bumping change.

## Files Changed

| File | Change |
| --- | --- |
| `core/src/init.rs` | **New.** `provision_project` + moved `extension_map` / `is_cached`; 2 unit tests (empty-project + dry-run short-circuits). |
| `core/src/lib.rs` | Add `pub mod init;`. |
| `core/src/registry.rs` | Add `pub fn locked_langs(path)` reader for `grove.lock`. |
| `cli/src/init.rs` | `run()` delegates provisioning to core then writes harness; `write_harness` reduced to harness-only (no lock, no `langs` param) sourcing steering langs via `registry::locked_langs`; removed moved helpers + unused imports; updated `use`s; reworked the harness unit tests to assert harness-only writes. |
| `cli/tests/cli.rs` | Add end-to-end `init_provisions_and_wires_harness_per_target` (mcp / grammars / `--dry-run`) — AC4 regression guard for files + stdout shape. |

## Test Evidence

### `cargo build --release --locked --workspace`

```
   Compiling grove-core v0.1.11 (.../core)
   Compiling grove v0.1.11 (.../cli)
    Finished `release` profile [optimized] target(s) in 11.18s
```

### `cargo clippy --workspace -- -D warnings`

```
    Checking grove-core v0.1.11 (.../core)
    Checking grove v0.1.11 (.../cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```

Clean — no warnings (the previously-flagged unused imports `WalkBuilder`,
`BTreeMap`, `HashMap`, `grove_core::{fetch, registry}` were pruned from the CLI and
added to core, per the plan-review advisory).

### `cargo test --release --locked --workspace`

```
test init::tests::provision_dry_run_writes_no_lock_and_returns_empty ... ok
test init::tests::provision_empty_project_writes_nothing_and_returns_empty ... ok
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (grove-core lib)

test init::tests::grammars_target_writes_no_harness_files ... ok
test init::tests::skill_target_writes_steering_but_no_mcp_json ... ok
test init::tests::mcp_target_writes_mcp_json_and_steering ... ok
test init::tests::both_target_writes_mcp_json_and_steering ... ok
test init::tests::target_default_is_mcp_and_flags_route_correctly ... ok
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (grove bin unittests)

test init_provisions_and_wires_harness_per_target ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (cli/tests/cli.rs)
```

All suites green: 87 core lib + 32 CLI bin unit + 18 CLI e2e. The harness unit
tests now assert harness-only writes (grammars → 0 harness writes; skill → 1;
mcp/both → 2); lock coverage moved to the core-side provision tests.

### `cargo tree -p grove-core` (clap-free check)

```
NO CLAP IN grove-core
```

## Knowledge Writeback

No KB updates required — internal refactor; no new architecture, stack, or domain
discoveries. `grove init` behavior, the `grove.lock` / `.mcp.json` / `CLAUDE.md`
formats, and the CLI surface are unchanged.

## Notes / Deviations

- The plan-review advisory to preserve the fetch `.context("auto-fetching detected
  grammars")` string was honored — it moved verbatim into core.
- The `grammars_target_writes_only_lock` unit test was renamed to
  `grammars_target_writes_no_harness_files` (its assertion changed: the lock now
  comes from core, so the harness path writes nothing for that target).
- `write_harness` no longer receives `langs`; the CLAUDE.md steering language list
  is read back from the lock core just wrote via the new `registry::locked_langs`.
  This keeps `provision_project`'s return type the planned `Vec<String>` of
  wrote-actions rather than widening it to also carry the language list.
