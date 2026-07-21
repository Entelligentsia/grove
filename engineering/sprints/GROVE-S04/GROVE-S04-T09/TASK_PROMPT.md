# GROVE-S04-T09: Docs restructure — two-server story (README, setup, mcp, steering templates, ADR annotations)

**Sprint:** GROVE-S04
**Estimate:** M
**Pipeline:** default

---

## Objective

Land the two-server story everywhere users and agents read about grove
(intake decision D4): README's delegated-mode section, `docs/setup.md`,
`docs/mcp.md`, the init-written steering templates, the repo orientation
`CLAUDE.md`, and the ADR trail — no reference to a mode-switched single server
survives.

## Acceptance Criteria

1. README's delegated-mode section, `docs/setup.md`, and `docs/mcp.md` are
   rewritten to the two-server composition story: structural only, explore
   only, or both; when to use each surface; `grove-explore` as the locator
   product's own server.
2. The init-written steering templates (CLAUDE.md/AGENTS.md sentinel blocks —
   T04's content) match the docs exactly; docs and steering name the same
   server identities, verbs (`grove-explore config`/`tap` with the old
   spellings noted as deprecated shims), and recommended delegation flow.
3. No user-facing doc still describes: `serve --explore`/`--standard`, the
   health fallback, mode-selected surfaces, or the mode badge/inert TUI
   rendering. A repo-wide sweep (docs/, README, skills/grove/SKILL.md,
   dist/npm README if present) is evidenced in PROGRESS.md.
4. This repo's `CLAUDE.md` (developer orientation: architecture block,
   commands, mcp-llm paragraphs) reflects the workspace layout and two-server
   surface; ADR 0002's superseded clauses (§1 surface selection, §3 inert
   rendering) carry an annotation pointing at ADR 0004; ADR 0004's status
   moves to Accepted with the implementation state noted.
5. `docs/SUMMARY.md`/book structure updated if pages are added/renamed;
   internal links resolve (book builds if the docs pipeline is exercised in
   review).
6. Prose-only task: no behavior changes; `cargo test` remains green (guards
   against accidental code edits, e.g. doc-tests or `include_str!` templates).

## Context

Implements item 9 of `SPRINT_REQUIREMENTS.md` (D4). Depends on **T04** and
**T06** (the steering content and init flow it documents are final). The
steering templates are load-bearing product surface (VISION §6.4.1 and the
grove-explore-steering memory: locator-framed + recommended flow, or the outer
agent bypasses grove) — docs must not drift from what `reconcile_harness`
writes. Nice-to-have from requirements (only if all must-haves are done):
doctor hint for deprecated spellings in user docs.

## Artifacts Involved

- `README.md`, `docs/setup.md`, `docs/mcp.md`, `docs/SUMMARY.md`
- `CLAUDE.md` (repo orientation), `docs/adr/0002-*.md` (annotations),
  `docs/adr/0004-*.md` (status)
- `skills/grove/SKILL.md` + any steering-template sources not already updated
  in T04

## Operational Impact

- **Version bump:** none (docs ride the same release).
- **Regeneration:** none.
- **Security scan:** not required.
