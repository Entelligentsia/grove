# PLAN_REVIEW — GROVE-S01-T06 (iteration 1 of 3)

**Verdict:** Approved

## Review Summary

The plan correctly identifies the single blocking gap for crates.io publication readiness: the `grove-core` dependency in `cli/Cargo.toml` lacks a version pin. The proposed fix is correct and complete.

## Verification Performed

1. **cli/Cargo.toml line 19** confirms `grove-core = { path = "../core" }` — path-only, no version. This is indeed rejected by crates.io on publish.

2. **bump-version.sh** has 5 numbered steps; adding a version-pin update step between steps 2 and 3 is correctly positioned.

3. **RELEASING.md** has no `cargo publish` documentation — the proposed addition is a real gap fill.

## Findings

- Problem analysis is accurate
- Proposed solution (exact version pin `=0.1.11`) is the correct fix for workspace crates
- Testing strategy covers the right gates (dry-run, build, test, clippy)
- Acceptance criteria are testable and complete
- Files to modify list is accurate

## Advisory Notes

1. **bump-version.sh implementation detail:** The perl/sed pattern must handle both the initial case (adding `version = "=X.Y.Z"` to an existing path-only dependency) and subsequent bumps (updating an existing version pin). The plan mentions this step but doesn't specify the exact regex — implementation should handle both cases:
   - First run: `grove-core = { path = "../core" }` → `grove-core = { path = "../core", version = "=X.Y.Z" }`
   - Subsequent runs: `grove-core = { path = "../core", version = "=OLD" }` → `grove-core = { path = "../core", version = "=NEW" }`

2. **Ordering is critical:** The plan correctly documents that `grove-core` must publish before `grove`. This ordering must be enforced in RELEASING.md documentation.

## Conclusion

The plan addresses the correct problem with the correct solution. No revision required.
