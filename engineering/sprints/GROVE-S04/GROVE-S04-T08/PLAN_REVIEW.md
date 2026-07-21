# PLAN_REVIEW — GROVE-S04-T08 (Revision): Packaging — release.yml/npm/brew ship both binaries

(iteration 1 of 3)

## Verdict: Approved

---

## Context

This is a **revision plan** addressing the critical code-review finding that `scripts/bump-version.sh 9.9.9` was run against the real working tree and not reverted, corrupting all workspace version files. The packaging implementation itself (release.yml, npm, Homebrew, install.sh, bump-version.sh, RELEASING.md, deployment.md) was assessed as correct in the prior code review.

**Current repo state verified:** All version files are at `0.4.1` (confirmed via `grep` on all four `Cargo.toml` files and `dist/npm/package.json`). The corruption has been remediated.

---

## Plan Assessment

### Correctness

The plan correctly identifies that:
1. Version files are already restored — verification is needed, not mutation.
2. The `--version` evidence in PROGRESS.md was stale and needs fresh capture after a forced rebuild.
3. The bump-version.sh script itself is correct — the failure was procedural (running against real files without revert).

The six-step approach (verify → forced rebuild → test/lint → bump-script exercise → syntax checks → document procedure) is complete and addresses all code-review findings.

### Feasibility

All steps are straightforward:
- Step 1: Verification via grep/head commands — no risk.
- Step 2: Touch + rebuild + capture — standard.
- Step 3: Existing test/lint commands — standard.
- Step 4: The run→verify→restore→checksum protocol is well-designed and provides both mutation evidence and restoration proof.
- Step 5: Existing syntax check commands — standard.
- Step 6: RELEASING.md edit — minimal scope.

### Completeness

The plan covers all code-review findings:
- [x] Version corruption addressed (already fixed, plan verifies).
- [x] Stale `--version` evidence addressed (forced rebuild + fresh capture).
- [x] False PROGRESS.md claim addressed (fresh evidence with explicit protocol).
- [x] Recurrence prevention addressed (RELEASING.md "Testing the bump script" section).

### Security

No security concerns — this is documentation and verification work, not code changes.

### Architecture Alignment

The plan correctly preserves all approved implementation work without modification. Only RELEASING.md (documentation) and PROGRESS.md (evidence) are modified.

---

## Minor Advisory Notes (non-blocking)

1. **Step 1 table typo:** The table lists `Cargo.toml | [package] version` as a verification target, but the workspace root `Cargo.toml` has no `[package]` section — it's a workspace manifest only. The intended target is `core/Cargo.toml`. The plan's subsequent rows correctly cover all four crate Cargo.toml files, so this doesn't affect correctness.

2. **Step 4 implementation note:** The plan correctly observes that bump-version.sh operates in-place and proposes a run→verify→restore→checksum protocol rather than a temp-copy approach. This is the right call — the script's in-place behavior is intentional for release workflows; the fix is procedural discipline, not script modification.

---

## Acceptance Criteria Alignment

| Plan Step | Acceptance Criterion |
|---|---|
| Step 1 | All twelve version-bearing locations confirm 0.4.1 |
| Step 2 | grove 0.4.1 and grove-explore 0.4.1 from forced rebuild |
| Step 3 | cargo test and cargo clippy pass |
| Step 4 | bump-version.sh mutation + restoration + checksum verification |
| Step 5 | JS/shell/YAML/JSON syntax checks pass |
| Step 6 | RELEASING.md "Testing the bump script" section |
| All | PROGRESS.md contains fresh, non-stale evidence |

All acceptance criteria are traceable to plan steps.

---

## Verdict Rationale

The revision plan is **Approved** because:
1. It correctly diagnoses the failure mode (procedural, not code).
2. It provides a complete verification and evidence-capture protocol.
3. It adds a sustainable prevention mechanism (documented testing procedure).
4. It limits scope to documentation artifacts, preserving the already-verified implementation.

Proceed to implementation.
