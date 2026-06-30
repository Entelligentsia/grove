# PLAN REVIEW — GROVE-S01-T03: Split init.rs — provision_project (core) vs harness (CLI)

*grove Supervisor — standalone review*

**Verdict: Approved**

The plan correctly draws the seam at "what touches only grammars/lock" (core) vs
"what writes agent files" (CLI) and preserves the `grove init` stdout/files contract
that AC4 gates on. I verified every load-bearing claim against the actual current
code in `cli/src/init.rs` and `core/src/`; the plan's narration-boundary analysis is
accurate, not a copy of the Engineer's optimism.

## Spec-compliance verification (read the code, not the report)

- **AC1 (provision_project signature + writes no harness files):** confirmed feasible.
  `registry::write_lock_for` is `pub` (core/src/registry.rs:403), `fetch::catalog_grammars`
  is `pub` (core/src/fetch.rs:55), `registry::manifests`/`cache_root` are `pub`. The two
  helpers to be moved (`extension_map`, `is_cached`) depend *only* on these pub core APIs
  — no clap, no `Target`. The move is clean.
- **AC2 (harness stays CLI-side):** `Target` (+ `clap::ValueEnum`), `write_mcp_json`,
  `write_claude_md`, `claude_section`, `CLAUDE_*`/`MCP_*` constants all remain in
  `cli/src/init.rs`. Confirmed none of them read `grove.lock`, so moving the lock write
  *before* the harness writes (into provision_project) has no ordering dependency.
- **AC3 (CLI calls provision_project first):** the `Cmd::Init` arm in `cli/src/main.rs`
  is unchanged (`init::run(&path, target, dry_run)`); `run` keeps its signature. ✓.
- **AC4 (identical behavior):** the stdout-order trace holds. Current order is
  header → detected → fetching/note → wrote[.mcp.json, CLAUDE.md, grove.lock] → trailers.
  The split reproduces it exactly: CLI prints header; provision prints detected/fetching;
  CLI assembles `wrote` as **harness-writes ++ provision-actions** = [.mcp.json, CLAUDE.md, grove.lock];
  CLI prints trailers. I traced all four short-circuit terminals (no-files, dry-run,
  no-cached-grammars) — each prints its terminal line in provision and returns empty,
  CLI returns early. The `Grammars` target correctly reduces to "provision_project with
  no harness step" (writes_mcp=false, writes_steering=false → empty harness ++ [lock]).
- **AC5 (clap-free core):** `core/Cargo.toml` has no `clap` dependency; `provision_project`
  takes `(&Path, bool)` — no `Target`/`ValueEnum`. Invariant preserved.
- **Stack checklist:** walk stays on `ignore::WalkBuilder` (gitignore-aware); lockfile path
  unchanged (deterministic, sha256); cache via `dirs`-backed `registry::cache_root`. No
  checklist item violated. No new external input surface → security scan correctly N/A.

## Advisory notes (non-blocking — address during implementation)

1. **Unused imports after the move — clippy-blocking.** Once `extension_map`/`is_cached`/the
   file walk move to core, `cli/src/init.rs::run` no longer references `WalkBuilder`,
   `BTreeMap`, `HashMap`, or `use grove_core::{fetch, registry}`. The plan hedges ("keeps
   `use grove_core::{fetch, registry}` … if still needed") — it is **not** still needed
   (write_harness no longer writes the lock; harness writers use neither). Leaving any of
   these in fails `cargo clippy -- -D warnings`. Prune them mechanically; add the
   corresponding `use` lines (incl. `HashMap`, `WalkBuilder`) to `core/src/init.rs`.

2. **AC4 baseline capture is implicit.** There is *no* existing e2e `grove init` test in
   `cli/tests/cli.rs` (the only lock test there is `lock_writes_grove_lock`, exercising the
   `lock` subcommand, not `init`). The init coverage lives entirely in `cli/src/init.rs::mod tests`
   calling `write_harness` directly — so the `run()` orchestration/narration is currently
   **untested**. AC4's "identical stdout" claim is only provable against a captured baseline:
   run the current binary per target + `--dry-run`, freeze the stdout/files as expected
   values, refactor, then assert equality. The plan commits to the e2e test (good) but
   doesn't name the baseline-capture step — do it, or "identical" is an unverified assertion.

3. **`provision_project` is not a silent library API.** It prints narration to stdout
   (`detected`/`fetching`/`note`/terminals) and the catalog-unavailable line to stderr
   (`eprintln!` in `extension_map`). This is the deliberate cost of preserving AC4's stdout
   boundary, and it matches the task's stated priority (AC4 > library silence). But a library
   consumer calling `provision_project()` will emit user-facing text — acceptable for this
   task; flag for a future `silent` variant if/when an embedding host needs it.

4. **Test rename.** `grammars_target_writes_only_lock_no_steering_no_mcp_json` asserts the
   lock via `write_harness`; after the refactor `write_harness(Grammars)` writes **nothing**
   (zero harness writes). Update the assertions *and* rename the test — its name will
   otherwise describe behavior that moved to core.

5. **Preserve error context.** `fetch::run(&missing, false).context("auto-fetching detected grammars")`
  must survive the move into `provision_project` verbatim — it's user-facing error text.