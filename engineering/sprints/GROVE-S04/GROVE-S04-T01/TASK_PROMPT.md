# GROVE-S04-T01: Stage 1 — move `core/src/explore/` into a new `explore/` workspace crate

**Sprint:** GROVE-S04
**Estimate:** L
**Pipeline:** default

---

## Objective

Return `grove-cst` to what its crates.io description claims — a pure structural
code-intelligence library — by moving the entire explore subsystem
(`core/src/explore/`, 11 modules + embedded prompt, ~3,900 lines) into a new
workspace crate `explore/` (name `grove-explore-core` reserved, **not
published** per D3). This is ADR 0004 Stage 1: a mechanical boundary move with
zero behavior change.

## Acceptance Criteria

1. A new workspace member `explore/` holds everything currently under
   `core/src/explore/` (`mod.rs`, `config.rs`, `wire.rs`, `client.rs`,
   `health.rs`, `discovery.rs`, `agent.rs`, `trace.rs`, `toolset.rs`,
   `steering.rs`, `grounding.rs`, `prompts/explore_v2.system.md`); its public
   surface remains `run_explore[_reporting]`, config, and health types (the
   existing `mod.rs` boundary).
2. `grove-cst` contains no explore modules and zero LLM types; its dependency
   tree carries no explore-only dependencies (HTTP chat client, prompt embeds,
   trace writer) — verified via `cargo tree -p grove-cst` / `Cargo.toml`
   inspection in review.
3. `GroveConfig` stays in `core/src/config.rs` and holds the explore section as
   a type opaque to core (owned by or re-exported from the new crate), per the
   ADR's Stage-1 design. Config load/save/migration behavior (incl. the legacy
   `explore.json` migration from S03-T02) is unchanged.
4. Behavior-bearing harness modules (`agent.rs`, `steering.rs`, `grounding.rs`,
   `toolset.rs`, `wire.rs`, `prompts/explore_v2.system.md`) move
   **byte-identical** apart from mechanical path/import adjustments —
   demonstrated by a diff summary in PROGRESS.md for the reviewer.
5. The new crate compiles in-process calls to grove ops exactly as today (no
   MCP hop, no subprocess); `cli/` consumes the new crate wherever it imported
   `core::explore` (temporarily — T02/T03/T05 relocate those call sites).
6. All existing explore unit tests move with the code and pass; the full
   workspace is green: `cargo test`,
   `cargo clippy --all-targets --workspace --locked -- -D warnings`,
   warning-clean `cargo build`. Files end with a newline.

## Context

Implements item 1 of `SPRINT_REQUIREMENTS.md`; ADR 0004 Stage 1 (see
`docs/adr/0004-explore-split-into-grove-explore.md` §Decision). The boundary is
already clean — `core/src/explore/mod.rs` exports only `run_explore[_reporting]`,
config, and health types. The byte-identical requirement is the code-level
backstop for gate test (a) (T10's sidebench re-run); review of this task is
where that diff is inspected. Toolchain constraint: cargo 1.87, crates.io deps
only. Every other code task in this sprint depends on T01 landing first.

## Artifacts Involved

- `explore/` — new workspace crate (Cargo.toml + moved modules)
- `Cargo.toml` (workspace root) — new member
- `core/src/explore/` — removed; `core/src/lib.rs`, `core/src/config.rs` —
  explore section becomes opaque/re-exported
- `core/Cargo.toml` — drop explore-only deps (HTTP client etc.)
- `cli/src/*` — import-path updates only (`core::explore` → new crate)

## Operational Impact

- **Version bump:** rides the next normal release train (D3 — no crates.io
  publish this sprint).
- **Regeneration:** none — no user-facing behavior change.
- **Security scan:** not required.
