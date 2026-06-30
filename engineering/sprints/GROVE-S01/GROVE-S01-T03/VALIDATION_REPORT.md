# VALIDATION_REPORT — GROVE-S01-T03 (standalone review)

Split `init.rs` — `provision_project` (clap-free core) vs harness (CLI).

**Verdict:** Approved

## Acceptance Criteria

### AC1 — `provision_project` in core, clap-free, provisioning-only — ✅ PASS
`core/src/init.rs#provision_project@29` has the exact signature
`pub fn provision_project(root: &Path, dry_run: bool) -> Result<Vec<String>>`.
Source confirms it: builds an extension→language map, scans project files,
auto-fetches missing grammars to the OS cache, and writes/refreshes
`grove.lock` via `registry::write_lock_for`, returning
`vec![format!("grove.lock ({n} grammars)")]`. It writes **no** `.mcp.json` and
**no** `CLAUDE.md`. `core/src/lib.rs:9` declares `pub mod init;`. Short-circuit
contract holds: empty project, dry-run, and no-cached-grammars all print a
terminal line and return `Ok(Vec::new())`.

### AC2 — harness stays CLI-side — ✅ PASS
`cli/src/init.rs` retains `pub enum Target` (`Mcp`/`Skill`/`Both`/`Grammars`)
with `#[derive(... ValueEnum)]` (line 22), and the `write_mcp_json` (line 126)
and `write_claude_md` (line 148) writers. The clap `ValueEnum` derive stays
CLI-side; `cargo tree -p grove-core` shows no clap.

### AC3 — CLI calls provision first, then harness — ✅ PASS
`cli/src/init.rs#run` calls `provision_project(root, dry_run)?` first; on an
empty return it early-exits (short-circuit contract), else `write_harness` runs
and `wrote = harness-writes ++ provisioned`, preserving the
`.mcp.json` → `CLAUDE.md` → `grove.lock` output order.

### AC4 — `grove init` behaves identically (files + stdout) — ✅ PASS
`cli/tests/cli.rs::init_provisions_and_wires_harness_per_target` passes. It
asserts, deterministically offline (seeded `XDG_CACHE_HOME` + dead
`GROVE_REGISTRY_URL` + dev-stub `GROVE_REGISTRY`):
- **mcp target:** writes `.mcp.json` + `CLAUDE.md` + `grove.lock`; stdout
  narrates detection/language/wrote; wrote order `i_mcp < i_claude < i_lock`.
- **grammars target:** writes only `grove.lock`, no `.mcp.json`/`CLAUDE.md`.
- **--dry-run:** narrates detection + dry-run note, writes nothing.
Harness unit tests additionally cover skill/both routing (harness-only writes).

### AC5 — build/test/clippy green; grove-core clap-free — ✅ PASS
Independently re-run:
- `cargo build --release --locked --workspace` → Finished, clean.
- `cargo clippy --workspace -- -D warnings` → Finished, no warnings.
- `cargo test --release --locked --workspace` → **137 passed, 0 failed**
  (grove bin 32 · cli e2e 18 · grove_core lib 87).
- `cargo tree -p grove-core` → no clap dependency.

## Regression & Edge Coverage
- No regressions: all pre-existing unit + e2e tests pass.
- Edge cases covered by tests: empty/no-match project, dry-run, offline
  (uncached grammar note), grammars-only target (no harness), idempotent
  `CLAUDE.md`/`.mcp.json` writes, invalid existing `.mcp.json` error path.
- Boundary on lock round-trip (`write_lock_for` ↔ `locked_langs`) validated by
  prior code review; steering language list remains byte-identical.

All acceptance criteria are met with test evidence. Task validated.
