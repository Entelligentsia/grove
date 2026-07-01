# CODE_REVIEW — GROVE-S01-T06: crates.io publication readiness for grove-core + grove

**Verdict:** Approved

**Iteration:** 1 (orchestrated review)

---

## Spec Compliance

All plan requirements were implemented correctly:

| Requirement | Status | Evidence |
|---|---|---|
| `cli/Cargo.toml` grove-core version pin | DONE | Line 19: `grove-core = { path = "../core", version = "=0.1.11" }` |
| `bump-version.sh` step for pin update | DONE | Step 3/6 (lines 42-46) updates the exact-version pin atomically |
| `RELEASING.md` cargo publish steps | DONE | Step 5 (lines 51-61) with ordering constraint (lines 91-93) |

## Verification Results

### Dry-run Gates

- `cargo publish --dry-run -p grove-core`: **PASS** — exits 0 with "aborting upload due to dry run"
- `cargo publish --dry-run -p grove`: **EXPECTED FAILURE** — fails with `grove-core = "=0.1.11"` not found on crates.io. This is correct behavior: the exact-version dependency requires grove-core to be published first. The error confirms the pin is active and the ordering constraint documented in RELEASING.md is necessary.

### Regression Gates

- `cargo test --release --locked --workspace`: **PASS** — 87 unit tests + 1 doc-test (grove-core) plus CLI tests
- `cargo clippy --workspace -- -D warnings`: **PASS** — exits 0, no warnings

## Code Quality

1. **`cli/Cargo.toml` change:** Minimal and correct. The exact pin (`=0.1.11`) ensures the published grove binary binds the exact matching grove-core version.

2. **`bump-version.sh` additions:**
   - Step 3/6 correctly updates the pin with a perl regex that handles in-place substitution
   - Header comment updated to document the new step
   - Done message updated to reflect the change
   - Step numbering correctly adjusted to 1-6

3. **`RELEASING.md` additions:**
   - Step 5 clearly documents the publish order (grove-core then grove)
   - Includes 30s wait guidance for crates.io indexing
   - Ordering constraint bullet explicitly explains why core must precede cli
   - Steps renumbered correctly (old 5-8 became 6-9)

## Advisory Notes

- The perl regex `s/(grove-core\s*=\s*\{[^}]*version\s*=\s*)"=[^"]*"/$1"='"$VERSION"'"/` assumes the version pin is already present (it updates, not inserts). This is correct for the current state where T06 added the pin, and future bumps will update it.
- When actually publishing, ensure you wait for crates.io indexing between the two cargo publish commands as documented.

## Conclusion

The implementation is complete, correct, and minimal. All acceptance criteria are met. The "expected failure" on grove dry-run is actually evidence the implementation is working as designed — it proves the exact-version dependency is enforced and validates the ordering constraint.
