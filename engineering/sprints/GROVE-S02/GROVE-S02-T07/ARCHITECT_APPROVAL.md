# Architect Approval — GROVE-S02-T07

**Verdict:** Approved

## Scope Approved

The sprint-closing task for S02 (`grove init --as mcp-llm` / delegated local-LLM
exploration): cross-cutting integration tests, a naming guard, mcp-llm documentation
across README/CHANGELOG/CLAUDE.md, and the unanimous 0.3.0 version bump.

## Architectural Assessment

- **Coherence with the explore subsystem.** The task's integration tests exercise
  the seams of the new subsystem introduced across S02 — non-TTY fast-fail in
  `config_tui`, `.mcp.json` object-key dedup on repeat init, and loop-level
  allowlist gating (`is_in_toolset` → `corrective_refusal`). These are the exact
  cross-module boundaries where regressions would otherwise surface silently. Test
  placement matches module ownership; no architectural leakage.
- **Provenance hygiene resolved.** The earlier code-review cycle correctly caught
  that `steering.rs` and `toolset.rs` were untracked and absent from `files_changed`.
  That is now fixed — `mod.rs` wires `pub mod agent/steering/toolset` with re-exports,
  so the explore agent loop is no longer orphaned at HEAD. This closes a real
  latent-breakage risk (a green tree that would fail on clean checkout).
- **Naming guard as an architectural invariant.** `naming_guard_no_fastcontext_in_source`
  encodes the fastcontext→explore rename as an enforced repo-wide invariant with a
  scoped scan (excludes `cli/tests/` so it cannot self-trip). This is the right
  cheap, durable guard against terminology drift.
- **Allowlist enforcement (AC1c).** Loop-level `allowed_tools` gating is verified
  distinct from the shell-dispatch refusal path — the toolset gate and dispatch gate
  are independently tested. Correct defense-in-depth for the delegated-LLM tool
  surface.

## Cross-Cutting Concerns

- Stack-checklist (AC4) documented and satisfied: stdio hygiene, exit codes, no
  `unsafe`, rustls-only. Consistent with `architecture/stack.md`.
- No new external dependencies introduced by the tests; std walk preferred per prior
  review advisory.

## Deployment Notes

- **Release-bearing.** Version 0.3.0 is unanimous across `cli/Cargo.toml`,
  `core/Cargo.toml`, `dist/npm/package.json`, with `Cargo.lock` refreshed — per
  RELEASING.md. Committing this task readies the 0.3.0 release surface: `grove config`,
  `grove serve --explore`/`--standard`, `grove init --as mcp-llm`, health-gated
  startup, and the inner explorer engine.
- No migrations, no persistent-state changes. Operational impact is limited to the
  new opt-in local-LLM mode; standard mode is unaffected.

## Follow-Up Items (future sprints)

- Consider a CI job that runs a clean-checkout build to catch untracked-source
  regressions structurally (the class of issue caught manually this cycle).
- Health/fallback semantics for `serve --explore` are documented; a future sprint
  could add an integration test that simulates an unhealthy local LLM and asserts
  the fallback path end-to-end.

## Gate Confirmation

205/205 tests pass; `clippy -D warnings` clean; `build --release --locked` clean.
Code review: approved. Validation: approved. All five ACs satisfied.
