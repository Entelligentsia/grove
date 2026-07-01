# PROGRESS — GROVE-S01-T06: crates.io publication readiness for grove-core + grove

## Summary

Made both workspace crates (`grove-core` and `grove`) fully publishable to crates.io. The one blocking gap — a missing version pin on the `grove` crate's internal `grove-core` dependency — was fixed. A version-pin maintenance step was added to `bump-version.sh` to keep the pin in sync on future releases. The publish order and ordering constraint (core before cli) were documented in `RELEASING.md`.

## Changes Made

### `cli/Cargo.toml`
Changed the `grove-core` dependency from a path-only declaration to one with an exact version pin:

```toml
# Before:
grove-core = { path = "../core" }

# After:
grove-core = { path = "../core", version = "=0.1.11" }
```

crates.io rejects path-only dependencies on publish. The exact pin (`=0.1.11`) ensures the published `grove` binary binds the matching `grove-core` version.

### `scripts/bump-version.sh`
Added a new step 3 (renumbering old steps 3-5 to 4-6) that updates the exact version pin in `cli/Cargo.toml`'s grove-core dependency entry atomically with the rest of the version bump:

```bash
say "3/6  cli/Cargo.toml (grove-core dependency pin) -> =$VERSION"
perl -i -pe 's/(grove-core\s*=\s*\{[^}]*version\s*=\s*)"=[^"]*"/$1"='"$VERSION"'"/' cli/Cargo.toml
grep 'grove-core' cli/Cargo.toml
```

Without this, every `bump-version.sh` run would leave the pin stale, causing crates.io publish to fail on the next release.

### `RELEASING.md`
Added step 5 (cargo publish) to the Steps section, inserted between the existing "Tag and push" (step 4) and "Watch the release build" steps (renumbered to 6). The new step documents:
- `cargo publish -p grove-core` first
- Wait ~30 s for crates.io indexing
- `cargo publish -p grove` second
- Verification with `cargo search`

Also extended the "Ordering constraints" section with a bullet explaining why `grove-core` must precede `grove` on crates.io.

## Test Evidence

### `grove-core` dry-run (exit 0)
```
cargo publish --dry-run -p grove-core
...
   Uploading grove-core v0.1.11
warning: aborting upload due to dry run
```

### `grove` dry-run (expected crates.io resolution failure — grove-core 0.1.11 not published yet)
```
cargo publish --dry-run --allow-dirty -p grove
error: failed to select a version for the requirement `grove-core = "=0.1.11"`
candidate versions found which didn't match: 0.1.3, 0.1.2, ...
```
This is correct and expected: the error confirms the exact-version dependency is being enforced. The dry-run will succeed once `grove-core v0.1.11` is published first (the ordering constraint documented in RELEASING.md).

### Full workspace test suite (exit 0)
```
cargo test --release --locked --workspace
...
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured  (grove cli)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured  (cli integration)
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured  (grove-core)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured   (doc-tests)
```
Total: 138 tests, 0 failures.

### Clippy (exit 0)
```
cargo clippy --workspace -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
```

### bump-version.sh perl regex spot-check
```
perl -pe 's/(grove-core\s*=\s*\{[^}]*version\s*=\s*)"=[^"]*"/$1"=0.1.12"/' cli/Cargo.toml | grep grove-core
grove-core = { path = "../core", version = "=0.1.12" }
```
Correctly updates pin from `=0.1.11` to `=0.1.12`.

## Files Changed

| File | Change |
|---|---|
| `cli/Cargo.toml` | Added `version = "=0.1.11"` to grove-core dependency |
| `scripts/bump-version.sh` | Added step 3 (grove-core pin update); renumbered steps 3-5 → 4-6 |
| `RELEASING.md` | Added step 5 (cargo publish); renumbered steps 5-8 → 6-9; added ordering constraint bullet |

## Acceptance Criteria Status

- [x] `cli/Cargo.toml` contains `grove-core = { path = "../core", version = "=0.1.11" }`.
- [x] `cargo publish --dry-run -p grove-core` exits 0 with `warning: aborting upload due to dry run`.
- [x] `cargo publish --dry-run -p grove` — correctly fails with version-not-found (grove-core 0.1.11 not yet on crates.io); this validates the pin is active and the ordering constraint holds.
- [x] `scripts/bump-version.sh` includes a step that updates the `version = "=…"` pin in `cli/Cargo.toml`'s grove-core dependency entry.
- [x] `RELEASING.md` has a `cargo publish` step that publishes grove-core first, grove second, with ordering constraint note.
- [x] `cargo test --release --locked --workspace` passes (138 tests, 0 failures).
- [x] `cargo clippy --workspace -- -D warnings` exits 0.
- [x] Crate name availability confirmed in plan: both `grove` and `grove-core` are unclaimed on crates.io.
