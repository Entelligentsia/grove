# GROVE-S01-T03: Split init.rs — provision_project (core) vs harness (CLI)

**Sprint:** GROVE-S01
**Estimate:** M
**Pipeline:** default

---

## Objective

Separate Grove's project *provisioning* (extension scan + grammar fetch + lockfile)
from its agent-*harness* setup (`.mcp.json` + `CLAUDE.md`). Provisioning becomes a
clean core API a library consumer can call directly; the harness stays a CLI
concern. This realizes the issue's key example: a consumer calls
`provision_project()` without polluting the directory with agent files.

## Acceptance Criteria

1. `core/src/init.rs` exposes
   `pub fn provision_project(root: &Path, dry_run: bool) -> anyhow::Result<Vec<String>>`,
   declared `pub mod init;` in `core/src/lib.rs`. It scans file extensions, fetches
   missing grammars to the OS cache, and writes/refreshes `grove.lock` — and writes
   **no** `.mcp.json` and **no** `CLAUDE.md`. The returned `Vec<String>` lists the
   provisioning actions (used by the CLI for its output).
2. `cli/src/init.rs` retains the harness logic: the `Target` enum (`Mcp`, `Skill`,
   `Both`, `Grammars`), `.mcp.json` writer, and the `CLAUDE.md` steering block
   writer. The clap `ValueEnum` derive stays CLI-side.
3. The CLI's `init` command calls `grove_core::init::provision_project()` first, then
   writes the harness files for the chosen `Target`.
4. `grove init` behaves **identically** to before for every target and `--dry-run`:
   same files written, same stdout shape. Verified by the existing `tests/cli.rs`
   init coverage (extend if a seam is now untested).
5. `cargo build/test --release --locked --workspace` green; clippy clean;
   `grove-core` still clap-free (`cargo tree -p grove-core`).

## Context

Depends on **T02** (grove-core exists; `fetch`/`registry` already moved to core, so
provisioning's heavy lifting is already core-side). Watch the `Grammars` target:
it provisions grammars + lock and writes no project files — i.e. it is essentially
`provision_project` with no harness step, a useful correctness check that the split
is clean.

## Artifacts Involved

- New: `core/src/init.rs` (provisioning); `pub mod init;` in `core/src/lib.rs`.
- Edited: `cli/src/init.rs` (harness only, calls core), `cli/src/main.rs` (init wiring).
- Verify: `cli/tests/cli.rs` init cases (mcp/skill/both/grammars + dry-run).

## Operational Impact

- **Version bump:** not required.
- **Regeneration:** none — `grove init` output and written files unchanged.
- **Backward compat:** identical `grove init` behavior is a gating criterion.
