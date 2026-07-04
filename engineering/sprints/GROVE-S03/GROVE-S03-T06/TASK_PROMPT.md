# GROVE-S03-T06: `grove.lock` wasm sha256 verify primitive (core)

**Sprint:** GROVE-S03
**Estimate:** S
**Pipeline:** default

---

## Objective

Add the integrity primitive `grove doctor` needs: given a project's `grove.lock`,
recompute each cached grammar wasm's sha256 and compare it to the lock's `wasm`
field, reporting per-language match/mismatch/missing. This is a **new** verify
step (the lockfile is written today but never re-verified). Standalone and
read-only; consumed by T07.

## Acceptance Criteria

1. A new `core/` function (e.g. `registry::verify_lock(root) ->
   Result<Vec<LockVerifyEntry>>`, or a `core::lock` module) reads `grove.lock`,
   resolves each locked language's cached wasm, recomputes its sha256, and
   returns a typed per-language result (`{ lang, expected, actual,
   status: Match|Mismatch|Missing }`).
2. Reuses the existing lockfile read path (`core/src/registry.rs`
   `locked_langs` / lock read) and the same sha256 primitive `write_lock_for`
   uses to compute hashes, so a freshly written lock verifies clean.
3. Behaviour when no `grove.lock` is present is a distinct, non-error signal the
   caller can render as "not present" (T07 renders it as a warn/info, not a
   fail).
4. Unit tests: (a) a lock written by `write_lock_for` over the dev stub verifies
   all-`Match`; (b) a tampered/edited expected hash yields `Mismatch` for that
   language; (c) a lock referencing an absent cached wasm yields `Missing`.
   Keep assertions registry-root-agnostic per repo convention (or pin
   `GROVE_REGISTRY`).
5. `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo
   test` green. Files end with a newline.

## Context

Implements the `grove.lock` integrity check row of `SPRINT_REQUIREMENTS.md`
item 6 / the doctor proposal's universal checks. The proposal explicitly notes
this recompute-and-compare step is **not** an existing primitive. Current lock
code: `core/src/registry.rs` `write_lock`/`write_lock_for`/`locked_langs`
(lines ~406–434) and the sha256 usage in `fetch.rs`/`init.rs`. No dependency on
the config work — parallelisable; only **T07** consumes it.

## Artifacts Involved

- `core/src/registry.rs` (or new `core/src/lock.rs`) — `verify_lock` +
  `LockVerifyEntry` type; export from `lib.rs` if new module.

## Operational Impact

- **Version bump:** not required on its own (additive read-only primitive; ships
  with T07's `doctor`).
- **Regeneration:** none.
- **Security scan:** not required.
