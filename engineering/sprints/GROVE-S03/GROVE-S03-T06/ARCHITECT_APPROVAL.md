# Architect Approval — GROVE-S03-T06

**Verdict:** Approved

## Rationale

The task adds a read-only `verify_lock(path) -> Result<Option<Vec<LockVerifyEntry>>>`
primitive plus `LockVerifyStatus` and `LockVerifyEntry` to `core/src/registry.rs`,
re-exported through the curated `core/src/lib.rs` surface. It recomputes each cached
grammar wasm's sha256 and compares against the pinned `grove.lock`, enabling grove
doctor (T07) to detect drift.

The change is architecturally coherent:

- **Additive only.** No existing signatures, lock-file format, or `Cargo.toml`
  dependencies change. Nothing downstream is forced to migrate.
- **Reuses existing primitives.** Verification runs through the same `sha256()`
  helper that `write_lock_for`/`wasm_sha256()` pin with, so a freshly written lock
  round-trips to all-`Match` — no format skew risk between writer and verifier.
- **Path resolution is consistent** with `resolve()`: `registry_root()/<name>/grammar.wasm`,
  covering dev-stubs, cache_root, and `GROVE_REGISTRY`.
- **Signal mapping is clean and non-swallowing.** `Ok(None)` for absent lock,
  `NotFound → Missing`, hash-eq → `Match`, diff → `Mismatch`; other I/O errors
  propagate via anyhow context. Malformed entries are deliberately `continue`-skipped
  rather than unwrapped — a non-panicking choice the plan-review asked to be decided,
  and it is decided.

Independently corroborated by the code review (8/8 plan/AC items, clippy clean,
`verify_lock` 4/4) and validation (all five AC PASS, full suite green).

## Cross-Cutting Concerns

None. The primitive is isolated to the registry module and introduces no new
coupling. The doc comment correctly directs the T07 consumer to pass the resolved
`grove.lock` file path (not a project root), preventing a foreseeable misuse at the
integration boundary.

Advisory (non-blocking, carried forward): the lock top-level `version` field is not
validated. Acceptable for a read-only advisory primitive; if T07 begins gating on
lock schema version, fold that check in there rather than here.

## Deployment Notes

- **Version bump:** not required on its own (additive read-only primitive; ships
  under the sprint's aggregate release).
- **Regeneration:** none.
- **Migration:** none — lock format unchanged.
- **Security scan:** not required.

## Follow-Up Items

- **T07 (grove doctor):** consume `verify_lock` at the resolved `grove.lock` path;
  surface `Mismatch`/`Missing` as actionable diagnostics. This is the intended caller.
- Consider lock `version` validation if/when doctor gates on schema evolution.
