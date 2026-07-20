# PLAN_REVIEW — GROVE-S04-T04: `mcp-llm` = register-both (iteration 1 of 3)

**Verdict:** Approved

The plan has been revised since the prior review (PLAN.md 15:44 > PLAN_REVIEW.md 13:38). Both REQUIRED items from the prior review are now addressed:

## Verification of Prior REQUIRED Items

1. **Dead-code / AC7 (RESOLVED):** Plan §2c now explicitly states:
   > "Rename `write_mcp_json_explore` → `write_json_mcp_explore_server`... By renaming (not adding a sibling), `write_mcp_json_explore` is eliminated and the `dead_code` lint is avoided — AC7 remains green."
   
   Confirmed: this repurpose approach eliminates the orphaned function. The old `["serve", "--explore"]` hard-coding is replaced with `args: []` for the `grove-explore` binary.

2. **Matrix gate / AC4 (RESOLVED):** Plan §4a now shows updated `assert_mcp_json_consistent` with all three branches:
   - `Mode::McpLlm`: asserts `grove` args == `["serve"]` AND `grove-explore` key present
   - `Mode::Mcp | Both`: asserts `grove` args == `["serve"]` AND `grove-explore` must NOT be present
   - `Mode::Skill | Grammars`: asserts `grove` absent AND `grove-explore` absent
   
   This closes the strip-half gap — transitions away from McpLlm are now verified to remove `grove-explore`.

## Technical Assessment

The plan is architecturally sound:

- **ADR 0002 compliance (AC2):** The second per-harness loop in §2i lives inside the single `reconcile_harness_for` function, preserving the single-writer invariant.
- **Forward migration (AC3):** §3 correctly relies on `write_json_mcp` overwriting the `grove` key's args, so `["serve","--explore"]` → `["serve"]` converges on first run.
- **Dual-surface steering (AC1):** §2k/2l provide locator-framed steering naming both `mcp__grove__*` and `mcp__grove-explore__explore`, with the recommended flow intact per the memory note.
- **Host content preservation (AC6):** §4i extends the host-content test to verify neither `grove` nor `grove-explore` strip removes third-party `.mcp.json` entries.

## Advisory Notes (non-blocking)

- **Cross-platform binary name:** §2b notes Unix-only assumption with a code comment. Acceptable for T04 scope; Windows support is a follow-up if needed.
- **Doctor scope:** §6a seeds fixtures but adds no new doctor check. Confirmed acceptable — ACs don't require it, and `harness_matrix_clean_fixtures_all_ok` will exercise the both-servers shape via `expected_explore_args(McpLlm)`.

The plan is complete and ready for implementation.
