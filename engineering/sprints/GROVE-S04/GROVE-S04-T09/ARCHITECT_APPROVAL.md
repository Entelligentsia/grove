# ARCHITECT APPROVAL — GROVE-S04-T09: Docs restructure — two-server story

🧭 **grove Architect** — I hold the shape of the whole.

**Task:** GROVE-S04-T09 — Docs restructure — two-server story (README, setup, mcp, steering templates, ADR annotations)
**Sprint:** GROVE-S04
**Dependencies:** GROVE-S04-T04, GROVE-S04-T06

---

**Verdict:** Approved

---

## Architectural Review

GROVE-S04-T09 is a documentation-only task whose purpose is to bring every
prose surface that describes grove's MCP offering into alignment with the
**two-server composition story** that ADR 0004 established and that
GROVE-S04-T01–T08 shipped: `grove` (structural, seven `mcp__grove__*` tools) and
`grove-explore` (locator, `mcp__grove-explore__explore`) are **composable, not
mode-switched** — a client registers either, both, or neither, and an unhealthy
`grove-explore` startup is a hard error, never a silent surface swap.

### Alignment with shipped architecture

I independently verified the load-bearing architectural claims rather than
trusting the review chain's reports:

- **Workspace layout.** `Cargo.toml` declares `members = ["core", "explore",
  "cli", "grove-explore"]` with `resolver = "2"`. CLAUDE.md's "Architecture —
  one engine, two binaries" block enumerates exactly these four crates with
  the correct responsibilities (`core` = library, `explore` = inner
  `grove-explore-core`, `cli` = the `grove` binary with `main`/`mcp`/`init`
  only, `grove-explore` = the second binary with both TUIs + `tap`). The docs
  now describe the code that actually exists.
- **ADR trail.** ADR 0004 line 3 reads `**Status:** Accepted`; its
  "Status of implementation" section itemizes T01–T08 as committed, T10 (the
  gate test) as open, and Stage 3 as not scheduled — matching the sprint's
  real state. ADR 0002 carries additive blockquote annotations at §1 (L95)
  and §3 (L122) pointing at ADR 0004 §2, with the historical Decision body
  left intact as record. This is the correct ADR treatment: annotations and
  a status change, not historical rewrites.
- **Deprecated-shim framing.** `grove config` / `grove tap` are documented
  consistently across README, setup.md, reference.md, and CLAUDE.md as
  forwarding shims to `grove-explore config` / `grove-explore tap`. This
  matches the actual shim behavior and the `init.rs` steering text
  (verified during plan review and re-read in validation).
- **Ground-truth parity (AC2).** The rewritten docs name the same server
  identities (`mcp__grove__*`, `mcp__grove-explore__explore`), the same verb
  spellings (`grove-explore serve`/`config`/`tap`), and the same 3-step
  delegation flow as `cli/src/init.rs`'s `claude_section`/`agents_section`
  McpLlm branches. The docs cannot drift from what `grove init` writes.
- **Stale-language sweep (AC3).** My own grep for `serve --explore` /
  `serve --standard` across the in-scope prose files (README, CLAUDE.md,
  docs/setup.md, mcp.md, introduction.md, reference.md, book.toml) returned
  zero hits outside the three excluded categories (ADR history, unlinked
  SVG assets, the out-of-scope doctor-command-proposal planning doc).

### Cross-cutting concerns

None. This is a doc-only task. The sole `.rs` change is a 5-line rustdoc
comment on `Target::McpLlm` in `cli/src/init.rs` — no behavior change, no
ABI change, no schema change. Both review phases independently reproduced
`cargo test --release --locked` (139+65+43+42+54+1+0 passed, 0 failed) and
`cargo clippy --all-targets --workspace -- -D warnings` (clean) from a clean
working tree. `mdbook build` from `docs/` completes with no warnings.

No other module, crate, or runtime surface is touched. The docs now describe
the architecture the rest of GROVE-S04 already shipped, so they cannot
introduce a new cross-cutting concern — they retire one (the stale
mode-switch narrative that contradicted ADR 0004).

### Operational impact

- **Deployment:** none. No binary behavior, flag set, config schema, or
  health-probe semantics changed. The docs now accurately describe the
  hard-startup-health-gate behavior that T04 already shipped.
- **Version bump:** none — the docs ride the same release as the rest of
  GROVE-S04.
- **Migrations / regeneration:** none. No schema, no generated artifact.
  The stale `docs/assets/*.svg` files are explicitly out of scope (they are
  unlinked after this task dropped their embeds) and are left untouched.
- **Security scan:** not required — no code-path change.
- **Backwards compatibility:** none affected.

## Advisory notes (non-blocking, carried forward)

Two non-blocking advisories were surfaced by CODE_REVIEW.md and
independently reconfirmed by VALIDATION_REPORT.md. Neither violates any
acceptance criterion and I concur that neither blocks approval:

1. **`docs/SUMMARY.md` provenance.** PROGRESS.md's narrative undercounts the
   SUMMARY.md change — T09 added the ADR 0004 sidebar entry (the file was
   untracked before this task). The resulting sidebar content is **correct
   and required** (AC5): ADR 0004 is now Accepted and is referenced from the
   ADR 0002 annotations, so it must appear in the built book's sidebar. This
   is a provenance-labeling nit in PROGRESS.md's prose, not a doc defect.
2. **`docs/setup.md` "three faces" phrasing.** Line 25's "one engine behind
   three faces" reads mildly imprecise now that there are two MCP server
   binaries, but it is contextually scoped to the `--as mcp|skill|both` trio
   (the non-`mcp-llm` targets), and the immediately-following `--as mcp-llm`
   line clarifies the both-servers story. Not in conflict with any AC.

## Deployment notes

No deployment action required. The documentation now matches the deployed
binaries; operators reading README.md, docs/setup.md, or CLAUDE.md will see
the same two-server composition that `grove init --as mcp-llm` actually
writes to `.mcp.json`.

## Follow-up items for future sprints

1. **Stale SVG assets.** `docs/assets/*.svg` (the dropped
   `explore_delegation_flow.svg`, `mcp_explore_comparison.svg`,
   `grove_architecture_surfaces.svg`) are now unlinked. A future task should
   either regenerate them to the two-server story or delete them from the
   repo to avoid bit-rot. Explicitly deferred by this task's scope.
2. **`docs/setup.md` "three faces" tightening.** A one-line future tightening
   to "one engine behind four integration targets" would be cleaner, but is
   not required by any current AC. Optional polish.
3. **GROVE-S04-T10 gate test.** ADR 0004's gate test (a) remains the open
   task that finally closes the explore-split's acceptance. This task's
   docs correctly document T10 as the open gate; T10 itself is a separate
   task.
4. **`docs/doctor-command-proposal.md` and sibling planning docs.** Carry
   stale mode-switch language and are explicitly out of scope here. A future
   pass should either update or archive them.

## Sign-off

The implementation is consistent with the project's architecture, the ADR
trail is correct, the deployment posture is unchanged, and all six
acceptance criteria are independently verified by both the code-review and
validation phases with freshly reproduced evidence. I am signing off.

🧭 grove Architect