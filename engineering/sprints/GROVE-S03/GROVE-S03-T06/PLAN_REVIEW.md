# PLAN_REVIEW.md — GROVE-S03-T06 (standalone review)

**Verdict:** Approved

## Scope reviewed
`grove.lock` wasm sha256 verify primitive in `core/src/registry.rs`: new
`LockVerifyStatus` enum, `LockVerifyEntry` struct, `verify_lock(path) ->
Result<Option<Vec<LockVerifyEntry>>>`, a `lib.rs` re-export, and four unit tests.
Verified independently against the actual code in `core/src/registry.rs` (not the
plan's self-description).

## Independent verification against the codebase

| Plan claim | Verified fact | Result |
| --- | --- | --- |
| Lock file is `{version, grammars:[{name,version,wasm}]}` and `wasm` holds the expected hash | `write_lock_for@412` writes exactly that; `wasm` = `g.wasm_sha256()` | ✓ matches |
| A fresh lock verifies all-`Match` | `wasm_sha256@106` = `sha256(self.wasm.as_slice())`; `verify_lock` recomputes `sha256(&fs::read(path))` over the same file bytes → identical `"sha256:…"` string (`sha256@316`) | ✓ AC#2 holds |
| Wasm resolvable at `registry_root()/<name>/grammar.wasm` | `resolve@219` reads `idx.root.join(lang)/grammar.wasm`; `index@188` seeds `root` from `registry_root@169` | ✓ same path in steady state |
| `NotFound` → `Missing`, other IO err → `Err` | Mirrors the existing `read_optional` idiom in `resolve@219` | ✓ idiomatic |
| Absent lock → `Ok(None)` non-error signal | Distinct from `Ok(Some)`; satisfies AC#3 | ✓ |

All five acceptance criteria are addressed; the change is purely additive
(new public types + fn, no signature or lock-format changes), which matches the
stated "no version bump" operational impact.

## Advisory notes (non-blocking)

1. **`OnceLock` root memoization vs. re-evaluated `registry_root()`.** `index()`
   caches its `root` once per process via `OnceLock`, while `verify_lock` calls
   `registry_root()` fresh each time. In the normal case these are the same path.
   They can *diverge inside a single test binary* if a test mutates
   `GROVE_REGISTRY` after `index()` was already initialized by an earlier test
   (tests share one process): `write_lock_for` → `resolve` → stale `index().root`,
   but `verify_lock` → new env root. Keep the "Match after `write_lock_for`" test
   registry-root-agnostic — follow the existing `write_lock_for_pins_versions_and_hashes@653`
   pattern (ambient dev-stub `rust`, no `GROVE_REGISTRY` manipulation) rather than
   setting the env var mid-process. The `Missing` test (nonexistent lang) is
   immune to this since the dir never exists under any root.

2. **Malformed-entry robustness.** The algorithm reads `entry["wasm"].as_str()`
   and the entry `name` optimistically. The lock is machine-written so this is
   low-risk, but decide deliberately whether a lock entry missing `wasm`/`name`
   should be skipped, surfaced as a distinct status, or produce `Err` — avoid a
   silent `unwrap`.

3. **Re-export surface.** The plan re-exports the two *types* but not the
   `verify_lock` *function*; T07 will call it as `registry::verify_lock(...)`,
   which is fine (consistent with `write_lock_for`/`locked_langs` not being
   re-exported). Optionally re-export the fn too for symmetry — not required.

4. **Signature note for T07.** `verify_lock` takes the lock *file* path (parallel
   to `locked_langs(path)`), not a project root as the AC's illustrative example
   suggested. This is the better, convention-consistent choice — just ensure T07
   passes the resolved `grove.lock` path.

No changes required to proceed to implementation.
