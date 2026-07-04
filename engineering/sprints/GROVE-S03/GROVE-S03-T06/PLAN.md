# GROVE-S03-T06 Plan: grove.lock wasm sha256 verify primitive (core)

## Objective

Add a `verify_lock` function (plus supporting types) to `core/src/registry.rs`
that reads `grove.lock`, resolves each grammar's cached wasm, recomputes its
sha256, and returns a typed per-language match/mismatch/missing result — the
integrity primitive `grove doctor` (T07) will consume.

---

## Approach

### Why in `registry.rs`, not a new `lock.rs`

All lock-file concerns already live in `registry.rs`:
`write_lock`, `write_lock_for`, `locked_langs`, and the `sha256` helper itself.
Adding `verify_lock` here keeps the surface minimal; a future split can happen if
the file grows further. No new module → no new `lib.rs` `mod` declaration needed.

### Data types (new, public)

```rust
pub enum LockVerifyStatus { Match, Mismatch, Missing }

pub struct LockVerifyEntry {
    pub lang:     String,          // language name from the lock
    pub expected: String,          // "sha256:…" from the lock file
    pub actual:   Option<String>,  // sha256 we computed; None when Missing
    pub status:   LockVerifyStatus,
}
```

`Option<String>` for `actual` conveys `Missing` naturally without a sentinel.

### Function signature

```rust
pub fn verify_lock(path: &Path) -> Result<Option<Vec<LockVerifyEntry>>>
```

* `path` is the lock file path (parallel to `write_lock_for(langs, path)` and
  `locked_langs(path)` conventions — caller passes the exact file, not a project
  root).
* `Ok(None)` → lock file absent (non-error; caller renders as "not present").
* `Ok(Some(vec))` → lock file parsed; every grammar has an entry.
* `Err(_)` → I/O or parse failure (bad JSON, unexpected read error).

### Wasm-file resolution

Each lock entry's grammar wasm is found at:

```
registry_root() / <name> / grammar.wasm
```

This is identical to the path `resolve()` reads. It works for:
* Dev / test: source-tree `registry/<lang>/grammar.wasm` (via `dev_root()`)
* CI / docker: `GROVE_REGISTRY` override
* Production: `cache_root()` (fallback in `registry_root()` when no other
  candidate exists) — where `grove fetch` stores downloaded grammars.

Using this path lets us detect `Missing` without going through `resolve()` (which
would panic/error on unknown languages) and without loading the wasm into the
Grammar cache (unnecessary for a read-only verify).

### Algorithm inside `verify_lock`

```
1. let lock_path = path;
2. If !lock_path.exists() → return Ok(None)
3. Read and parse JSON   (reuse locked_langs read pattern for consistency)
4. For each object in doc["grammars"]:
   a. Extract name (String), expected_hash = entry["wasm"].as_str()
   b. wasm_path = registry_root().join(&name).join("grammar.wasm")
   c. match std::fs::read(&wasm_path):
        Ok(bytes)                      → actual = sha256(&bytes)
                                         status = if actual == expected { Match } else { Mismatch }
        Err(e) if NotFound             → actual = None, status = Missing
        Err(e) other                   → return Err(e).with_context(...)
   d. push LockVerifyEntry { lang: name, expected, actual, status }
5. return Ok(Some(entries))
```

### `lib.rs` re-export

Add `pub use registry::{LockVerifyEntry, LockVerifyStatus};` so T07 (and any
future consumer) can name these types at the crate root without repeating the
module path.

---

## Files to Modify

| File | Change |
|------|--------|
| `core/src/registry.rs` | Add `LockVerifyStatus` enum, `LockVerifyEntry` struct, `verify_lock` function, and four unit tests |
| `core/src/lib.rs` | Add `pub use registry::{LockVerifyEntry, LockVerifyStatus};` to the curated surface |

No new files. No Cargo.toml changes (all primitives — `sha2`, `serde_json`,
`std::fs`, `anyhow` — are already in scope).

---

## Data Model Changes

None. `grove.lock` format is unchanged; this is a read-only consumer.

---

## Testing Strategy

Four unit tests inside `mod tests` in `registry.rs`, following the inline
pattern used throughout the module. **No `GROVE_REGISTRY` env override needed** —
tests use the dev-stub registry (`dev_root()`) which includes the `rust` grammar,
or construct a minimal lock JSON by hand.

| Test | What it proves |
|------|----------------|
| `verify_lock_returns_none_when_file_absent` | `Ok(None)` for a path that does not exist |
| `verify_lock_matches_after_write_lock_for` | Fresh lock written by `write_lock_for(&["rust"], …)` verifies all `Match` |
| `verify_lock_detects_tampered_hash` | Mutate the `wasm` field in the written lock JSON → entry status is `Mismatch` |
| `verify_lock_detects_missing_wasm` | Hand-craft a lock JSON referencing `"grove_nonexistent_lang_xyz"` → entry status is `Missing` |

All tests write to `std::env::temp_dir()` with process-id-qualified paths to
avoid collisions between parallel test workers.

---

## Acceptance Criteria

1. ✅ `verify_lock(path)` returns `Ok(None)` when the lock file is absent.
2. ✅ `verify_lock(path)` returns per-language `LockVerifyEntry` with `Match`,
   `Mismatch`, or `Missing` status.
3. ✅ Reuses `sha256()` primitive (same function `write_lock_for` uses), ensuring
   a freshly written lock verifies clean.
4. ✅ Four unit tests covering all status variants plus the absent-lock case.
5. ✅ `cargo build` warning-clean, `cargo clippy -- -D warnings` clean,
   `cargo test` green. All files end with a newline.

---

## Operational Impact

* **Version bump:** Not required (additive read-only primitive; ships inside an
  existing crate, no CLI surface change).
* **Regeneration:** None.
* **Security scan:** Not required.
* **Breaking changes:** None — new public types + function, no existing signatures
  changed.
* **T07 dependency:** T07 (`grove doctor` wasm integrity row) depends on this
  task and consumes `registry::verify_lock` directly.
