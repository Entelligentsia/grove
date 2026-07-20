# ARCHITECT_APPROVAL — GROVE-S04-T02

## grove-explore binary — dedicated single-tool MCP server with hard startup health gate

**Verdict:** Approved

## Approval Rationale

The implementation is consistent with grove's established architecture and I sign off for commit.

- **Fits the stack unchanged.** A second `[[bin]]` (`grove-explore`) in `cli/` reuses the project's hand-rolled stdio JSON-RPC 2.0 MCP surface (no async runtime, no external MCP SDK), the same `SUPPORTED_PROTOCOLS` set as `grove serve`, and clap/serde as everywhere else. No new dependencies, no build-profile changes. `CARGO_BIN_EXE_grove-explore` resolves for integration tests; a self-contained bin inheriting package deps is valid Cargo.
- **Config source respects ADR 0002.** `GroveConfig::load(root)` → `grove_cfg.explore` → `ExploreConfig` with no config-file move (Stage 3 concern intentionally deferred). The `.grove/config.json` explore-section placement is preserved.
- **The startup health gate is the right architectural posture.** Hard-fail before the loop (config load → explore deser → `health_probe`, each `process::exit(1)`) means an unhealthy provider surfaces as a non-zero startup exit with a fix hint — never a silent fallback to a degraded surface. This is exactly the "never fall back" invariant the plan set out, and it is verified by the empty-stdout / non-zero-exit integration test.
- **In-process explorer call, no MCP hop.** `run_explore_reporting` is called directly; mid-session `ProviderDown` returns an `isError:true` tool result identical in structure to `mcp.rs`. Cross-module behaviour of the existing `grove` binary is unchanged.
- **Independent verification is clean.** Code review and validation both ran build + `cargo test` + `clippy --all-targets --workspace --locked -D warnings` green (337 workspace tests, 0 failures). AC1–AC7 all satisfied.

## Deployment Notes

- **New release artifact.** grove now produces a second binary. The distribution channels (GitHub Releases cross-compiles, the `@entelligentsia/grove` npm wrapper, the Homebrew formula, the install script) currently ship a single `grove` binary. Packaging/registration of `grove-explore` is explicitly out of scope here (owned by T04/T06) — but it MUST land before any release that intends to expose `grove-explore` to end users, or the binary will build in CI yet never reach a distribution channel. Flagging so it is not lost between tasks.
- No config-file migration, no version bump, no schema change — deployment impact for the `grove` binary itself is nil.

## Follow-up Items (future work / sibling tasks)

1. **T03 dedup (tight ordering).** ~130 LOC (`explore_tool_spec`, protocol constants, the ProviderDown arm) are duplicated from `mcp.rs`. Keep the T02→T03 sequence close so the copy does not drift. Reconcile the trimmed `explore_tool_spec` description and the Step-2 config diagnostic that hard-codes "mode is mcp-llm" without verifying mode.
2. **T04/T06 registration + distribution.** Wire `grove-explore` into release packaging and any client/registration manifests before it is user-visible (see Deployment Notes).
3. **Stage 3 config move** remains deferred by design — no action now, tracked for the config-relocation slice.
