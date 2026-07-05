# CODE_REVIEW — GROVE-S03-T07: `grove doctor` (standalone review)

**Verdict:** Approved

## Scope verified
Isolated the true T07 footprint from the sprint diff (merge-base diff bundled
committed T02–T06). Working-tree changes reviewed: new `core/src/harness.rs`,
new `core/src/doctor.rs`, `core/src/lib.rs` (module wiring), `cli/src/init.rs`
(constant migration only, 8 lines), `cli/src/main.rs` (Cmd::Doctor + rendering),
`cli/tests/cli.rs` (integration tests). The `reconcile_harness` refactor visible
in the cumulative diff is committed T04 work, not T07 — confirmed via
`git diff HEAD`.

## Independent verification (not trusting PROGRESS.md)
- `cargo clippy -- -D warnings` — clean.
- `cargo test --release --locked` — **green**: 177 core (incl. 9 doctor + 3
  harness), 92 CLI unit, 33 CLI integration, 1 doc-test. 0 failed.
- Ran and read the actual harness matrix / drift / explore tests — assertions are
  meaningful (real Fail/Warn/Ok discrimination, not tautologies).
- Both new files end with a trailing newline (AC8).

## AC compliance
1. ✅ `Status/Check/Report` shapes exact; `Report.mode` is the core declared
   `Mode` (not `Surface`); `diagnose` is pure/read-only (single optional health
   probe, no writes).
2. ✅ Mode resolution via shared `active_mode(root, force)`; `--explore/--standard`
   map to `ModeChoice::ForceExplore/ForceStandard` with correct precedence.
3. ✅ Universal checks present: version, config_present (Ok/Fail/Info),
   legacy_explore_json warn, four harness sub-checks, registry_root,
   grammar_cache, project_languages, lock_integrity. Harness drift → Fail naming
   the mismatched file with a `grove init --as <mode>` hint.
4. ✅ `lock_integrity` delegates to T06 `verify_lock`; Mismatch→Fail,
   Missing→Warn, absent→Warn (never Fail).
5. ✅ Explore checks gated on `McpLlm`: config validate, provider_reachable /
   model_served surfacing `HealthError` hints verbatim, allowed_tools_known
   (Warn on unknown), tap_config (Info).
6. ✅ `Cmd::Doctor { path, explore, standard }` + global `--json`; grouped
   human table or machine JSON `{ path, mode, ok, checks[], summary }`; exit code
   from `Report::ok()` under both formats (integration-tested both).
7. ✅ Unit tests use existing seams + tempdir fixtures (no cross-crate private
   helpers); harness matrix + hand-induced drift; integration tests cover
   help/clean/json/drift.
8. ✅ Build+clippy clean, tests green, `--help` documents `doctor`, files
   newline-terminated.

## Advisory notes (non-blocking)
1. `Report.mode` reports the *declared* config mode while harness checks run
   against `active_mode(root, force)`. Running `grove doctor --explore` on an
   `mcp`-declared project shows `mode: Mcp` in the header but validates harness
   against `McpLlm`. This matches AC1's literal wording ("the core declared
   Mode"), so it is correct-as-specified — but a future polish could echo the
   forced/effective mode in the human header to avoid confusion.
2. AC5's "`steering` legal" is covered transitively by `ExploreConfig::validate()`
   rather than a dedicated named check. Acceptable; flagging only for traceability.
3. Cosmetic: removing the `MCP_SERVER_KEY` const from `init.rs` left a
   double blank line before `pub fn run`. Trivial; rustfmt-neutral.
