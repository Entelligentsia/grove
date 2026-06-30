# PLAN — GROVE-S01-T03: Split init.rs — provision_project (core) vs harness (CLI)

🌱 *grove Engineer*

**Task:** GROVE-S01-T03
**Sprint:** GROVE-S01
**Estimate:** M

---

## Objective

Separate Grove's project *provisioning* (extension scan + grammar fetch + `grove.lock`) from
its agent-*harness* setup (`.mcp.json` + `CLAUDE.md`). Provisioning becomes a clean, clap-free
core API — `grove_core::init::provision_project(root, dry_run) -> Result<Vec<String>>` — that a
library consumer can call directly without polluting the project directory with agent files. The
harness (the `Target` enum, the `.mcp.json` writer, the `CLAUDE.md` steering writer) stays a CLI
concern. `grove init` must behave **identically** to today for every target and `--dry-run`: same
files written, same stdout shape.

## Approach

Today `cli/src/init.rs::run()` interleaves both halves: it scans extensions, counts files,
short-circuits on dry-run / no-matches, fetches missing grammars, filters to cached languages,
then calls `write_harness()` which writes `.mcp.json` + `CLAUDE.md` + `grove.lock` and returns the
list of wrote-actions printed under a single `wrote` header.

The split draws the seam at "what touches only grammars/lock" (core) vs "what writes agent files"
(CLI), preserving stdout by keeping the **narration boundary** identical:

- **Core — `provision_project(root, dry_run)`** owns: build the extension→language map
  (`extension_map`), count project files per language, the dry-run / no-files / no-cached-grammars
  short-circuits, auto-fetch of missing grammars, the cached-language filter, and the `grove.lock`
  write (`registry::write_lock_for`). It prints exactly the provisioning narration it prints today
  (`detected …`, the `(dry run …)` note, `fetching …`, the offline `note …`, the
  `no files matched` / `no grammars available` terminals). It returns the **provisioning
  wrote-actions** — on the happy path `["grove.lock (N grammars)"]`; on any short-circuit an
  **empty Vec**, which is the contract telling the CLI "nothing was provisioned, stop." The helpers
  `extension_map`, `is_cached` move to core alongside it (both already depend only on
  `grove_core::{fetch, registry}`).

- **CLI — `cli/src/init.rs::run()`** owns: print the `grove init  scanning … (as {target})` header
  (it names `target`, a CLI concept), call `provision_project`, and **if it returns empty, return**
  (provision already printed its terminal line). Otherwise write the harness for the chosen `Target`
  — `.mcp.json` (MCP/Both) and the `CLAUDE.md` steering block (every target but `Grammars`) — then
  assemble the `wrote` list as **harness-writes followed by provision-actions** (preserving today's
  order: `.mcp.json`, `CLAUDE.md`, `grove.lock`), print the single `wrote` block, and print the
  `ready` / `skill` / `grammars` trailers as today. The `Target` enum (with its `clap::ValueEnum`
  derive), `write_mcp_json`, `write_claude_md`, `claude_section`, and the `CLAUDE_*` / `MCP_*`
  constants stay CLI-side. `write_harness` is reduced to harness-only (mcp + steering) — it no
  longer writes the lock.

- **Wiring** — `core/src/lib.rs` gains `pub mod init;`. `cli/src/init.rs` adds
  `use grove_core::init::provision_project;` (keeps `use grove_core::{fetch, registry}` for the
  harness's lock-count formatting if still needed). `cli/src/main.rs` `Cmd::Init` arm is unchanged
  (still `init::run(&path, target, dry_run)?`) — the CLI `run` keeps the same signature.

This keeps `grove-core` clap-free: `provision_project` takes `root: &Path, dry_run: bool` — no
`Target`, no `ValueEnum`. The `Grammars` target becomes the clean correctness check the task notes:
CLI with `Grammars` writes no harness files, so the `wrote` list is exactly `provision_project`'s
return — i.e. `provision_project` with no harness step.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `core/src/init.rs` | **New.** `pub fn provision_project(root, dry_run) -> Result<Vec<String>>` + moved `extension_map` and `is_cached` helpers; provisioning narration `println!`s. | Provisioning becomes a clap-free core API. |
| `core/src/lib.rs` | Add `pub mod init;`. | Expose the new module to library consumers + the CLI. |
| `cli/src/init.rs` | Reduce `run()` to header + `provision_project()` call + harness writes + `wrote`/trailer printing; reduce `write_harness` to mcp+steering only (drop lock write); remove the moved `extension_map`/`is_cached`; add `use grove_core::init::provision_project`. | Harness stays CLI; provisioning delegates to core. |
| `cli/src/main.rs` | Verify the `Cmd::Init` arm still compiles against `init::run` (expected: no change). | Init wiring per AC3. |
| `cli/src/init.rs` (`mod tests`) | Update the `write_harness` target tests so they assert harness files only (no `grove.lock` / adjusted `wrote` counts); the `Grammars` case now produces zero harness writes. | These tests call `write_harness` directly and currently assert the lock — which moves to core. |
| `cli/tests/cli.rs` | Add/extend an end-to-end `grove init` case (per target + `--dry-run`) asserting identical files-written + stdout shape, if a seam is now untested. | AC4 gating check that behavior is unchanged. |

## Plugin Impact Assessment

- **Version bump required?** No — internal refactor; `grove init` output and written files are
  unchanged (per task Operational Impact).
- **Migration entry required?** No — no schema or stored-artifact changes.
- **Security scan required?** No — no new external input surface; fetch/registry paths are unchanged.
- **Schema change?** No — `grove.lock` format and `.mcp.json` / `CLAUDE.md` contents are byte-for-byte unchanged.

## Testing Strategy

- **Build/test:** `cargo build --release --locked --workspace` and `cargo test --release --locked --workspace` green.
- **Lint:** `cargo clippy --workspace -- -D warnings` clean.
- **clap-free invariant:** `cargo tree -p grove-core` shows no `clap` dependency (the new core
  module must not reference `Target`/`ValueEnum`).
- **Harness unit tests (`cli/src/init.rs`):** the existing `*_target_*` tests that call
  `write_harness` directly are updated to reflect harness-only writes; `mcp`/`both` assert
  `.mcp.json` + `CLAUDE.md` (count 2), `skill` asserts `CLAUDE.md` only (count 1), `grammars`
  asserts zero harness writes. The `claude_section_*` / `write_mcp_json_*` / `write_claude_md_*`
  tests are unaffected.
- **Provisioning coverage:** add a core-side unit test (or e2e via `grove init`) asserting
  `grove.lock` is written and the returned actions list the lock, and that `--dry-run` writes no
  files and returns empty.
- **Identical-behavior gate (AC4):** exercise `grove init` per target (`mcp`/`skill`/`both`/`grammars`)
  and `--dry-run` via `cli/tests/cli.rs`, asserting the same files are written and the stdout shape
  (scanning → detected → fetching → wrote → trailers) is unchanged.

## Acceptance Criteria

- [ ] `core/src/init.rs` exposes `pub fn provision_project(root: &Path, dry_run: bool) -> anyhow::Result<Vec<String>>`, declared `pub mod init;` in `core/src/lib.rs`; it scans extensions, fetches missing grammars to cache, writes/refreshes `grove.lock`, and writes **no** `.mcp.json` / `CLAUDE.md`.
- [ ] The returned `Vec<String>` lists the provisioning actions and is consumed by the CLI for its `wrote` output.
- [ ] `cli/src/init.rs` retains the harness: `Target` enum (`Mcp`/`Skill`/`Both`/`Grammars`) with the `clap::ValueEnum` derive, the `.mcp.json` writer, and the `CLAUDE.md` steering writer.
- [ ] The CLI's `init` calls `grove_core::init::provision_project()` first, then writes harness files for the chosen `Target`.
- [ ] `grove init` behaves identically (files + stdout shape) for every target and `--dry-run`, verified by `cli/tests/cli.rs`.
- [ ] `cargo build/test --release --locked --workspace` green; `cargo clippy --workspace -- -D warnings` clean; `grove-core` remains clap-free (`cargo tree -p grove-core`).

## Operational Impact

- **Distribution:** No user action — `grove init` behavior and outputs are unchanged.
- **Backwards compatibility:** Identical `grove init` behavior for every target and `--dry-run` is the gating criterion; no stored format or CLI surface changes.
- **Dependency:** Depends on **GROVE-S01-T02** (grove-core exists; `fetch`/`registry` already core-side, so provisioning's heavy lifting is available to the new module).
