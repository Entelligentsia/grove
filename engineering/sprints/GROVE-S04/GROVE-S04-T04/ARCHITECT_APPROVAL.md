# ARCHITECT_APPROVAL — GROVE-S04-T04

**Verdict:** Approved

## Architectural Assessment

The implementation correctly re-keys `Mode::McpLlm` from single-explore to both-servers:

1. **Harness shape constants** (`core/src/harness.rs`): `EXPLORE_SERVER_KEY`, `expected_explore_args`, and updated `expected_claude_marker` define the contract for the second server. The single-source-of-truth pattern is preserved.

2. **Reconcile loop extension** (`cli/src/init.rs:334-345`): The second loop writes/strips the grove-explore entry across all harnesses without duplicating the first loop's structure — clean separation.

3. **Dual-surface steering**: The new `claude_section(McpLlm)` content is locator-framed, carries the recommended flow (`explore` → `source`/`map` → synthesise), and names both tool prefixes. This satisfies the memory note about outer-agent bypass.

4. **ADR 0002 invariant intact**: All MCP-mode writes still flow through the single `reconcile_harness_for` function. No new writer introduced.

## Cross-cutting Concerns

- **Forward migration**: Pre-split `["serve", "--explore"]` layouts converge on first `init` run — no dangling registrations. Test confirms this path.
- **Host content**: The strip functions only touch grove-owned entries (`grove` / `grove-explore` keys, sentinel-delimited markdown blocks).
- **Unix-only scope**: `find_explore_binary()` is documented as Unix-only for T04; T07 doctor and future Windows support are out of scope.

## Operational Impact

- **Version bump**: Material — `mcp-llm` mode behaviour changes (two registrations instead of one).
- **User action**: Existing `mcp-llm` users run any `grove init` to converge; no config migration script needed.
- **No new crates or dependencies**.

## Deployment Notes

None — this is a code change to the init and harness subsystems. The binary ships the new behaviour on next release train.

## Follow-up Items

- T07 (`doctor` re-keying) will add a check for the grove-explore registration in mcp-llm mode.
- Windows `.exe` suffix support in `find_explore_binary()` is deferred to a future sprint.

---

*Approved for commit.*
