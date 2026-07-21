# PLAN REVIEW — GROVE-S04-T09: Docs restructure — two-server story

🌿 *grove Supervisor*

**Task:** GROVE-S04-T09
**Review:** standalone review (iteration 1, no limit)

---

**Verdict:** Approved

---

## Review Summary

The plan is thorough, well-grounded in verified codebase reality, and correctly
scoped for a prose-only documentation task. Every ground-truth claim (workspace
layout, binary surface, init.rs steering text, serve flag removal, shim
behavior, ADR statuses, sprint task states) was independently verified against
the actual source and store. All six acceptance criteria are addressed with
specific, file-level actions. The testing strategy (grep sweep + init.rs
parity cross-check + mdbook build + cargo test guard) is appropriate for a
docs-only task.

## Feasibility

The approach is realistic and correctly scoped. The plan identifies 10 files
to modify (plus 2 reviewed-clean files and 1 optional nice-to-have) and
explicitly calls out 3 categories of out-of-scope items with reasoning. Each
doc-by-doc rewrite (items 1–10) is anchored to specific sections, line-level
stale content, and the ground-truth source (`cli/src/init.rs` steering blocks).

**Independent verification of ground-truth claims:**

| Claim | Verified |
|---|---|
| 4-crate workspace: core, explore, cli, grove-explore | ✓ `Cargo.toml [workspace] members` |
| cli/src has only main.rs, mcp.rs, init.rs (no TUIs/tap) | ✓ `ls cli/src/` |
| grove-explore/src has main.rs, tap.rs, config_tui/, trace_tui/ | ✓ `ls grove-explore/src/` |
| core/src has no explore/ subdirectory | ✓ `ls core/src/` |
| serve --explore/--standard → bail! naming grove-explore serve | ✓ `cli/src/main.rs:407-409` |
| grove config/tap are forwarding shims with deprecation notes | ✓ `cli/src/main.rs:358,165,188` |
| init.rs claude_section McpLlm: two servers, 7 tools, 3-step flow | ✓ `cli/src/init.rs:923` |
| init.rs agents_section McpLlm: same structure | ✓ `cli/src/init.rs:846` |
| Stale `serve --explore` in README, setup.md, introduction.md | ✓ grep confirmed |
| Stale `two MCP modes` in book.toml | ✓ `docs/book.toml:4` |
| Stale mode semantics in reference.md | ✓ `docs/reference.md:51` |
| ADR 0002: Status Proposed, §1 and §3 exist | ✓ confirmed |
| ADR 0004: Status Proposed, "not yet implemented" | ✓ confirmed |
| skills/grove/SKILL.md, dist/npm/README.md: clean | ✓ no stale refs |
| T01–T08 committed, T10 draft | ✓ store query confirmed |

## Plugin Impact Assessment

- **Version bump declared correctly?** Yes — "none (docs ride the same release)."
- **Migration entry targets correct?** N/A — no schema or store changes.
- **Security scan requirement acknowledged?** Yes — "not required — no code-path change."

## Security

No security risks. This is a prose-only documentation task. No new Markdown
files are added to plugin/agent surfaces (the init-written steering templates
are T04's already-shipped content; this task only documents them). No hooks,
no data paths, no input handling changes.

## Architecture Alignment

- The plan correctly follows the ADR convention: ADR 0002 gets **annotations**
  (not rewrites) on superseded clauses; ADR 0004 gets a status change and
  implementation-state note. ADR text is treated as historical record.
- The plan's ground-truth-first approach (read `init.rs` steering before
  writing docs, then cross-check after) ensures docs match the shipped
  artifact, not the ADR's proposal-stage design.
- The plan correctly identifies `cli/src/init.rs::Target::McpLlm` doc comment
  as stale (says `serve --explore` and `mcp__grove__explore`) but scopes it
  as a nice-to-have only — appropriate for a prose-only task whose Artifacts
  Involved list doesn't include `.rs` files.

## Testing Strategy

Adequate for a prose-only task:

1. **`cargo test --release --locked`** — guards against accidental code edits
   (especially the optional init.rs rustdoc fix). Correct.
2. **`cargo clippy --all-targets --workspace -- -D warnings`** — only relevant
   if the optional .rs edit is made; plan correctly runs it regardless as a
   cheap drift check. Correct.
3. **`mdbook build`** from `docs/` — confirms book compiles after
   SUMMARY.md/book.toml edits. `mdbook` is installed (`~/.cargo/bin/mdbook`).
   Correct.
4. **Manual grep sweep** — scoped to the right files, with correct exclusions
   (ADRs are historical; three planning docs are explicitly out-of-scope).
   Evidence goes in PROGRESS.md. Correct.
5. **init.rs parity cross-check** — re-read `claude_section`/`agents_section`
   after rewrite and confirm server names, verb spellings, and 3-step flow
   match. This is AC2's verification mechanism. Correct.

---

## If Approved

### Advisory Notes

1. **docs/introduction.md "single Rust binary" prose** — The plan says to
   "restate" the four-surfaces table and drop the SVG embed, but the
   surrounding prose ("grove is a single Rust binary and a library over one
   engine") also needs correction (it's now two binaries). The plan's
   "Restate as: CLI · MCP structural · MCP explore · Library" implicitly
   covers this, but the engineer should be explicit about correcting the
   "single binary" phrase, not just the table row.

2. **docs/SUMMARY.md sidebar labels** — The plan says to adjust labels "if
   page structure changes" and otherwise "leave structure as-is." But the
   labels themselves ("MCP: standard server" for mcp.md, "MCP: explore mode
   (mcp-llm)" for setup.md) use the old mode framing. If mcp.md gains a
   "Two servers, composable" section and setup.md is rewritten to the
   composition story, these labels may become misleading even without
   structural page changes. The engineer should evaluate whether the labels
   need updating for honesty (AC5), not just for structural changes.

3. **docs/setup.md line 33 code-block comment** — The one-liner
   `grove init --as mcp-llm  # explore-mode: .mcp.json (serve --explore) + ...`
   is inside a code block, not prose. The plan's "rewrite the Explore-mode
   section" should cover this, but the engineer should verify code-block
   comments are updated too, not just prose paragraphs.