# ARCHITECT_APPROVAL — GROVE-S02-T01: Explore config model + persistence (`core::explore::config`)

**Verdict:** Approved

## Rationale

- **Architectural fit.** The new `core::explore` module is purely additive to the `grove-cst` crate: `pub mod explore;` plus curated re-exports (`ExploreConfig`, `Mode`, `Provider`) in `lib.rs`, exactly matching the existing re-export style (`engine`, `ops`, `init`, `registry`). No existing surface was modified.
- **Convention coherence.** Persistence lands at project-local `.grove/explore.json`, a sibling to the established `.grove/grammars/` location (registry.rs:152). This keeps all grove project state under one dotdir — the right shared vocabulary for the S02 mcp-llm explorer subsystem that T02/T03 will build on.
- **Hardening, not just parity.** The atomic save (temp file `.tmp.<pid>` + `fs::rename`) is stronger than the registry's plain `fs::write`; a future sprint could retrofit this pattern onto the registry.
- **Error ergonomics.** Field-named enum errors via `RawExploreConfig` + manual `Deserialize`, and missing-file steering to `grove init --as mcp-llm` / `grove config`, satisfy the fail-fast AC and give downstream CLI work a clean UX foundation.
- **Dependency posture.** `cargo tree -p grove-cst` shows no clap; no `fastcontext` string in core/ — the core crate remains CLI-free as required.
- **Gates.** Build warning-clean, `clippy -D warnings` clean, 8/8 explore tests, 146/146 workspace tests — all independently re-run by code review and validation.

## Deployment Notes

- No version bump required — lands with the sprint's final release bump (per task prompt).
- No regeneration, migrations, or user-facing behavior changes. `.grove/explore.json` is only created when a future `grove init --as mcp-llm` / save path writes it.
- Windows caveat (acknowledged, non-blocking): `fs::rename` over an existing file can fail on Windows — fails cleanly with an error, never a torn file. Acceptable for Unix-first targets.

## Follow-ups for Future Sprints

1. Consider retrofitting the atomic temp+rename write pattern onto `registry.rs`, which still uses plain `fs::write`.
2. If Windows becomes a supported target, revisit `fs::rename`-over-existing semantics (e.g. `ReplaceFileW`-style fallback or remove-then-rename).
3. Temp filename is keyed on PID only; if same-process concurrent saves ever become plausible, add a counter/nonce.
4. Docs nit: task AC/PLAN reference `cargo tree -p grove-core`; the package is `grove-cst`. Fix in any templates that propagate this.
