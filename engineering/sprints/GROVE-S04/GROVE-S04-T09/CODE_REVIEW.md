# CODE REVIEW — GROVE-S04-T09: Docs restructure — two-server story

🌿 *grove Supervisor*

**Task:** GROVE-S04-T09
**Review:** standalone review (iteration 1, no limit)

---

**Verdict:** Approved

---

## Review Summary

The implementation faithfully executes the approved plan. Every must-have doc
was rewritten to the two-server composition story; the ADR trail carries the
right annotations and status; the one nice-to-have `init.rs` rustdoc fix landed
cleanly; and all four testing commands (`cargo test`, `cargo clippy`,
`mdbook build`, manual grep sweep) reproduce green from a clean working tree.
Spec compliance was verified independently — I read the actual diffs and the
actual `cli/src/init.rs` steering source, not the PROGRESS.md report.

## Spec Compliance (all 6 ACs verified)

| AC | Verified | How |
|---|---|---|
| 1 — README/setup/mcp two-server story | ✓ | `git diff` of all three; composition-not-mode-switch framing, `grove-explore` as own server, register-both via `--as mcp-llm` |
| 2 — docs match init.rs ground truth | ✓ | Re-read `claude_section` (L924) and `agents_section` (L847) McpLlm branches: server identities (`mcp__grove__*` 7 tools, `mcp__grove-explore__explore`), verb spellings (`grove-explore config`/`tap`, deprecated shims noted), and 3-step flow (narrow `explore` → `source`/`map` on `file:line` → synthesize) all match substance-for-substance |
| 3 — no stale mode-switch language | ✓ | Independent grep sweep: zero `serve --explore`/`serve --standard` hits in any in-scope prose file; zero `mode badge`/`inert` hits outside ADRs (which are excluded by design). Only `--explore`/`--standard` references are in CLAUDE.md describing them as *removed/`bail!`-ing* flags — correct, not stale. `doctor-command-proposal.md` hits are in the explicitly out-of-scope planning doc. |
| 4 — CLAUDE.md + ADRs | ✓ | CLAUDE.md architecture block rewritten to real 4-crate/two-binary layout; Commands section splits `grove serve` (no flags) from `grove-explore serve`/`config`/`tap` with deprecated-shim notes. ADR 0002 §1 and §3 carry blockquote annotations pointing at ADR 0004. ADR 0004 Status → Accepted with detailed implementation state (T01–T08 committed, T10 open, stage 3 gated). |
| 5 — SUMMARY.md/book.toml consistency | ✓ | `book.toml` description "two MCP modes" → "two composable MCP servers". `mdbook build` from `docs/` succeeds with no warnings (reproduced independently). |
| 6 — cargo test green | ✓ | Reproduced: 139+65+43+42+54+1+0 passed, 0 failed across all crates. `cargo clippy --all-targets --workspace -- -D warnings` clean. |

## Correctness

- The `init.rs` rustdoc edit (the only `.rs` change) is a 4-line doc-comment
  rewrite on `Target::McpLlm` describing register-both + `grove-explore config`
  shell-out. No behavior change — confirmed by the green test/clippy run and the
  `git diff --stat` (9 lines, all in the doc-comment block).
- ADR 0002 annotations are additive blockquotes; the historical Decision body
  (§1's `determine_surface` prose, §3's inert-rendering prose) is left intact as
  record, exactly as the plan and ADR convention require.
- ADR 0004's "Status of implementation" section accurately itemizes what shipped
  (crate split, binary + own server identity + hard startup health gate, mcp.rs
  surface deletion, reconcile_harness both-servers column, TUI/tap move + shims,
  init funnel, doctor re-keying, packaging) and what remains (T10 gate test,
  stage 3 not scheduled). This matches the actual committed state of T01–T08.

## Architecture / Conventions

- Doc-only task; no architectural surface changed. The docs now describe the
  shipped architecture (verified during plan review: 4-crate workspace, `cli/src`
  has only main/mcp/init, `grove-explore/src` has main + both TUIs + tap,
  `core/src` has no `explore/` subdir). CLAUDE.md's file tree matches `ls` reality.
- The deprecated-shim framing (`grove config`/`grove tap` forward to
  `grove-explore config`/`grove-explore tap`) is documented consistently across
  README, setup.md, reference.md, and CLAUDE.md — matching the actual shim
  behavior in `cli/src/main.rs`.

## Testing Evidence (independently reproduced)

- `cargo test --release --locked` → all green (139+65+43+42+54+1+0 passed).
- `cargo clippy --all-targets --workspace -- -D warnings` → clean.
- `mdbook build` (from `docs/`) → clean, no warnings.
- Manual grep sweep for `serve --explore`/`serve --standard`/`mode badge`/`inert`
  across in-scope files → clean (only excluded categories hit).

---

## Advisory Notes (non-blocking)

1. **PROGRESS.md misreports the `docs/SUMMARY.md` change.** PROGRESS.md states
   "the ADR 0004 sidebar entry was already present from a prior commit. No edit
   made by this task." This is inaccurate: `docs/adr/0004-*.md` was never
   committed before this task (it is an untracked file), and `git diff` on
   `docs/SUMMARY.md` shows this task **added** the
   `- [ADR 0004 — Explore split into grove-explore](...)` sidebar line. The
   actual edit is **correct and necessary** — ADR 0004 is now Accepted and is
   referenced from the ADR 0002 annotations, so it must appear in the built
   book's sidebar; without it the sidebar would be incomplete. Only the
   PROGRESS.md *narrative* is wrong, not the change. No fix needed in the docs;
   the record should simply not be relied upon for this one file.

2. **skills/grove/SKILL.md sweep detail slightly off.** PROGRESS.md cites "one
   unrelated hit: 'the model reads the few relevant symbols'" as the lone grep
   result, but an independent `grep -in "explore" skills/grove/SKILL.md` returns
   zero hits. The conclusion (clean, no edit needed) is correct; the specific
   hit description is inaccurate. Harmless — the file is verified clean.

3. **`docs/setup.md` line 28 "one engine behind three faces"** — still reads
   "the CLI, the MCP server, and a cross-harness skill". With two MCP servers
   now, this is mildly imprecise, but in context it describes the `--as
   mcp|skill|both` trio (the non-`mcp-llm` targets), and the `--as mcp-llm`
   line immediately below clarifies the both-servers story. Not a blocker; a
   future tightening to "one engine behind four integration targets" would be
   cleaner but is not required by any AC.

4. **`docs/mcp.md` section placement.** The plan offered "after 'Same engine as
   the CLI' or as a new 'Two servers, composable' section". The engineer placed
   it after "Availability vs. adoption" (early), before "Tool schemas". This is
   arguably better for reader orientation than the plan's fallback placement
   and is within the plan's latitude. No issue.

---

## If Approved

No revision items — all advisories are non-blocking. The implementation is
correct, the docs match the shipped code, and the testing evidence reproduces.