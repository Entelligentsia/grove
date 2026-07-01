# ARCHITECT_APPROVAL — GROVE-S01-T06: crates.io publication readiness for grove-core + grove

🗻 *grove Architect*

**Task:** GROVE-S01-T06
**Sprint:** GROVE-S01
**Date:** 2026-07-01

---

**Verdict:** Approved

---

## Architectural Assessment

### Alignment with Project Architecture

This task correctly completes the crates.io publication pipeline for the grove workspace. The implementation is minimal and precisely scoped:

1. **Dependency pin** — The exact version pin (`=0.1.11`) in `cli/Cargo.toml` ensures deterministic resolution when published. The published `grove` binary will bind exactly the `grove-core` version it was developed against.

2. **Version script maintenance** — The `bump-version.sh` step 3/6 keeps the pin synchronized with workspace versions. This prevents silent drift that would cause publish failures on future releases.

3. **Documentation** — The `RELEASING.md` updates correctly document the publish order constraint (core before cli) with clear rationale.

### Cross-Cutting Concerns

- **No impact** on existing distribution channels (npm, Homebrew, GitHub Releases)
- **Additive only** — enables `cargo install grove` as a new distribution path
- **No API changes** — grove-core's public API is unchanged from T04

### Operational Impact

| Impact | Assessment |
|--------|------------|
| Backwards compatibility | None — purely additive |
| Migration required | No |
| Deployment changes | No |
| Security implications | No — metadata only |

### Verification Notes

The "expected failure" on `cargo publish --dry-run -p grove` (version not found on crates.io) is correct behavior:

1. It proves the exact-version dependency (`=0.1.11`) is being enforced
2. It validates the ordering constraint: grove-core must be published first
3. The dry-run will succeed once grove-core is actually published

This is the intended design — the test validates the invariant.

## Deployment Notes

When the actual `cargo publish` is executed (maintainer-driven, not automated):

1. Publish `grove-core` first: `cargo publish -p grove-core`
2. Wait ~30s for crates.io indexing
3. Publish `grove`: `cargo publish -p grove`

This sequence is now documented in `RELEASING.md` step 5.

## Follow-up Items

None for this sprint. Future considerations:

- Consider CI automation for crates.io publish (requires crates.io API token in secrets)
- The 12 minimal-profile languages (bash/julia/haskell) still need tags.scm coverage (tracked in ROADMAP)
