# Architect Approval — GROVE-S04-T01

**Verdict:** Approved

## Scope

Stage-1 extraction of `core/src/explore/` (11 modules + embedded prompt, ~3,900
lines) into a new `grove-explore-core` workspace crate, making `grove-cst` a pure
structural library by storing the explore section of `GroveConfig` as an opaque
`serde_json::Value`.

## Approval Rationale

- **Architecturally coherent.** The opaque `Option<serde_json::Value>` seam for
  `GroveConfig.explore` is the correct choice: it keeps `GroveConfig` in
  `grove-cst` while deferring typed Provider/Steering deserialization to the CLI,
  breaking what would otherwise be a circular `grove-cst ↔ grove-explore-core`
  dependency. This matches the ADR's Stage-1 design.
- **Structural boundary verified.** `cargo tree -p grove-cst` carries no
  explore-only dependencies (HTTP chat client, prompt embeds, trace writer); the
  new crate's `lib.rs` re-export surface mirrors the old `mod.rs` boundary exactly,
  so no consumer loses a symbol.
- **Behavior preserved.** Behavior-bearing harness modules (`agent.rs`,
  `steering.rs`, `grounding.rs`, `toolset.rs`, `wire.rs`,
  `prompts/explore_v2.system.md`) moved byte-identical apart from mechanical
  import/path edits — `toolset.rs`'s `crate::ops → grove_core::ops` is the sole
  non-path content change, confirmed by diff vs git HEAD in code review.
- **In-process semantics intact.** Grove ops are called in-process (no MCP hop, no
  subprocess), exactly as before.
- **Green across the board.** 335 tests pass, `clippy --all-targets --workspace
  -D warnings` clean, `cargo build --release --locked` passes. All 6 acceptance
  criteria validated PASS. Upstream plan/review-plan/code-review/validation all
  verdicted approved.

## Deployment Notes

- **Version bump:** rides the next normal release train (D3) — no crates.io
  publish this sprint.
- **Regeneration:** none — no user-facing behavior change.
- **Security scan:** not required.
- **Migration:** config load/save and the S03-T02 legacy `explore.json` migration
  behavior are unchanged; `migrate_from_legacy_explore` is now a pure JSON
  key-rename (`mode → steering`).

## Follow-up Items (future sprints)

1. **CLI call-site relocation (T02/T03/T05):** the CLI's temporary consumption of
   `grove-explore-core` at the four `serde_json::from_value`/`to_value` boundaries
   is the intended bridge; the follow-on tasks relocate those call sites.
2. **Advisory (non-blocking):** stale intra-doc link at
   `cli/src/config_tui/model.rs:107` — `grove_core::explore::discover_engines`
   should be `grove_explore_core`. Not caught by build/clippy; `cargo doc` warns.
   Fix opportunistically.
3. **Advisory (non-blocking):** `migrate_from_legacy_explore` defers
   Provider/Steering validation to CLI deserialize, so a malformed legacy file
   migrates silently. Documented trade-off; revisit if it surfaces in the field.
