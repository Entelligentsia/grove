# PROGRESS — GROVE-S04-T09: Docs restructure — two-server story

## Summary

Rewrote every doc describing grove's MCP surface to the two-server
composition story shipped by GROVE-S04-T01–T08: `grove` (always the 7-tool
structural MCP server) and `grove-explore` (its own MCP server identity, the
LLM-delegating locator) are **composable**, not a single server that
mode-switches its surface. Ground truth for server names, verb spellings, and
the recommended delegation flow was `cli/src/init.rs::claude_section`/
`agents_section` (McpLlm branch), independently re-verified before writing
each doc section, exactly as PLAN_REVIEW.md's testing strategy specified.

### Files rewritten (must-haves)

- **README.md** — replaced the `Advanced → Delegated local-LLM mode`
  `<details>` block: new title framing `grove-explore` as "a second,
  composable MCP server"; composition-not-mode-switch section; `grove-explore
  config`/`grove-explore tap` verbs with the deprecated-shim note (`grove
  config`/`grove tap`); corrected health semantics (unhealthy provider =
  startup error, not silent fallback). Dropped the stale
  `docs/assets/explore_delegation_flow.svg` embed, replaced by the prose/table
  content already in the section.
- **docs/setup.md** — rewrote `## Explore-mode — grove init --as mcp-llm`:
  `--as mcp-llm` registers **both** servers in `.mcp.json`; no health
  fallback; updated "What it writes" bullets (two `.mcp.json` entries, dual
  CLAUDE.md/AGENTS.md steering naming both server identities); first-run TUI
  note corrected to "`init` shells out to `grove-explore config`" with the
  PATH-absent graceful-degrade behavior (T06) documented. Dropped the stale
  `assets/mcp_explore_comparison.svg` embed. Verified: no `--as mcp` bullet
  (default target's "What init writes" table below it) needed a change — it
  describes the default `mcp` target's writes, not `mcp-llm`'s, and was
  already accurate.
- **docs/mcp.md** — added a new `## Two servers, composable` section (after
  Availability vs. adoption) naming both server identities
  (`grove`/`mcp__grove__*`, `grove-explore`/`mcp__grove-explore__explore`),
  what each is for, and that a project can register either or both, linking
  to `setup.md`.
- **docs/introduction.md** — corrected the "One engine, four surfaces" table:
  MCP row was `grove serve --explore` (a flag that doesn't exist on a surface
  that's actually a separate binary) → split into "MCP: structural" (`grove
  serve`) and "MCP: explore" (`grove-explore serve`), four surfaces still,
  correctly named. Corrected the "single Rust binary" prose (advisory from
  PLAN_REVIEW.md) to "a library shipped as two binaries." Dropped the stale
  `assets/grove_architecture_surfaces.svg` embed (baked in `serve --explore`
  as literal SVG text).
- **docs/reference.md** — `.grove/config.json` section: `mode` no longer
  selects a `serve` surface (it's unconditionally structural); `mode:
  mcp-llm` now means "`init` registers both servers"; `explore` section
  documented as read by `grove-explore`; `tap`/`config` cross-references
  updated to `grove-explore tap`/`grove-explore config` with a deprecated-shim
  note for the old `grove config`/`grove tap` spellings.
- **docs/book.toml** — `description`: "two MCP modes" → "two composable MCP
  servers."
- **docs/SUMMARY.md** — verified: no page adds/removes needed (no new page
  content was created — `mcp.md` grew a section rather than splitting into a
  new page); the ADR 0004 sidebar entry was already present from a prior
  commit. No edit made by this task. `mdbook build` confirmed clean (see
  Testing below).
- **CLAUDE.md** — heaviest rewrite:
  - `## Architecture` header/prose: "one engine, two faces" → "one engine, two
    binaries"; file tree replaced with the real four-crate layout:
    `core/src/*` (no `explore/` subdirectory), `explore/src/*`
    (`grove-explore-core`, moved wholesale, same module list), `cli/src/*`
    (only `main.rs`/`mcp.rs`/`init.rs` — no TUIs/`tap.rs`), and
    `grove-explore/src/*` (`main.rs` + both TUIs + `tap.rs`).
  - "mcp-llm mode is opt-in" paragraph: reframed to "register-both, not a
    surface switch," with the ADR 0002 §1 / ADR 0004 §2 amendment noted
    inline, and `grove-explore`'s hard-fail-on-unhealthy-provider startup gate
    replacing the old health-fallback framing.
  - `## Commands`: split `grove serve [path]` (always structural, flags
    removed/`bail!`) from a new `grove-explore serve [path]` block; moved
    `grove config`/`grove tap` into an explicit "deprecated shims" group;
    added the `grove-explore config`/`grove-explore tap` canonical verbs.
  - Everything else (grammar system, registry/cache, build/test/run,
    conventions, design decisions, roadmap, gotchas) left untouched per plan.
- **docs/adr/0002-grove-project-config-and-declared-mode.md** — annotated
  (not rewritten) §1 ("`serve` reads the declared mode") and §3 (config TUI
  display-consistency) with blockquote notes pointing at ADR 0004 §2 as the
  superseding/mooting change. Rest of the document (Context, Decision body,
  Alternatives, Consequences) left as historical record, unchanged.
- **docs/adr/0004-explore-split-into-grove-explore.md** — `Status: Proposed` →
  `Status: Accepted`; `## Status of implementation` rewritten from "Proposed
  — not yet implemented" to the actual state: stages 1–2 implemented
  (GROVE-S04-T01–T08, all committed, itemized); gate test (a) — the full
  sidebench re-run — is GROVE-S04-T10 and remains open; gate test (b) — the
  extended transition-matrix test — shipped with GROVE-S04-T04; stage 3
  remains not scheduled, gated on the stated triggers.

### Files reviewed, no edit needed (sweep evidence)

- **skills/grove/SKILL.md** — re-confirmed clean: documents only
  `mcp__grove__*` structural tools for the coding-agent skill; no mention of
  explore mode, `serve --explore`, or a mode-switched surface anywhere in the
  file (`grep -in "explore|mode"` found one unrelated hit: "the model reads
  the few relevant symbols" — "model" as in LLM sizing, not `config.mode`).
- **dist/npm/README.md** — re-confirmed clean: describes the `grove`
  package/binary generically (`grove init`, `grove serve`, seven tools); zero
  mentions of explore mode. `grep -in "explore|mode"` on this file: no hits.

### Explicitly out of scope (per plan, reasoning recorded)

- `docs/doctor-command-proposal.md`, `docs/code-quality-review-2026-07-03.md`,
  `docs/release-0.3.0-plan.md` — dated planning artifacts, not in
  `docs/SUMMARY.md`, not part of the built book, not documentation of current
  behavior. Confirmed by grep sweep: `doctor-command-proposal.md` still has
  several `serve --explore`/`--explore`/`--standard` references — these are
  proposal-stage design text for a `grove doctor` variant that was never
  shipped that way; intentionally untouched.
- `docs/assets/*.svg` — the three stale diagrams
  (`explore_delegation_flow.svg`, `mcp_explore_comparison.svg`,
  `grove_architecture_surfaces.svg`) were **not regenerated** (out of scope
  for a prose-only task); their embeds were **dropped** from README.md,
  setup.md, and introduction.md respectively and replaced with equivalent
  prose/table content, so no doc renders a picture of the old single-surface
  design. The SVG files themselves remain on disk, unlinked from any page.

### Nice-to-have taken

- **`cli/src/init.rs`** — one rustdoc comment on `Target::McpLlm` still
  described `.mcp.json` with `serve --explore`; corrected to describe the
  actual register-both behavior and the `grove-explore config` shell-out. No
  behavior change — doc comment only. This is the only `.rs` file this task
  touched, and only that one doc comment (verified via `git diff --stat`: 9
  lines changed, all within the doc comment block).

## Test Evidence

### `cargo test --release --locked`

```
$ cargo test --release --locked 2>&1 | grep -E "FAILED|error\[|test result:"
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
All green across all workspace crates (core, explore, cli, grove-explore) —
no behavior change slipped in through the one-line `init.rs` doc-comment edit.

### `cargo clippy --all-targets --workspace -- -D warnings`

```
$ cargo clippy --all-targets --workspace -- -D warnings
    Checking grove-cst-cli v0.4.1 (/home/boni/src/grove-engineering/grove/cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
```
Clean, zero warnings.

### `mdbook build` (from `docs/`)

```
$ cd docs && mdbook build
 INFO Book building has started
 INFO Running the html backend
 INFO HTML book written to `/home/boni/src/grove-engineering/grove/docs/../site/docs`
```
Clean build, no warnings — confirms `book.toml`/`SUMMARY.md` are consistent
and no internal link broke.

### Manual grep sweep (AC3 evidence)

```
$ grep -rn "serve --explore\|serve --standard" docs/ README.md CLAUDE.md skills/grove/SKILL.md dist/npm/README.md
docs/adr/0004-explore-split-into-grove-explore.md:240:  (historical Consequences prose — expected)
docs/assets/*.svg                                     (unlinked binary assets, out of scope)
docs/doctor-command-proposal.md                        (explicitly out-of-scope planning doc)
```
No hits in any in-scope prose file (README.md, CLAUDE.md, docs/setup.md,
docs/mcp.md, docs/introduction.md, docs/reference.md, skills/grove/SKILL.md,
dist/npm/README.md). The only remaining hits are the three excluded
categories from AC3 (ADR historical text, unlinked SVGs, the out-of-scope
doctor proposal).

```
$ grep -rin "mode badge\|inert" docs/ README.md CLAUDE.md skills/grove/SKILL.md dist/npm/README.md
(no output — zero hits anywhere, including ADRs, after the §3 annotation)
```

### Cross-check against `cli/src/init.rs` ground truth (AC2 parity)

Re-read `claude_section`/`agents_section` (McpLlm branch, L846–1000) after
the docs rewrite: server names (`mcp__grove__*`, 7 tools listed;
`mcp__grove-explore__explore`), the 3-step recommended flow (narrow `explore`
question → `source`/`map` on cited `file:line` → synthesize), and verb
spellings all match what README.md, docs/setup.md, docs/mcp.md, and CLAUDE.md
now describe, substance-for-substance.

## Files Changed

- `README.md`
- `docs/setup.md`
- `docs/mcp.md`
- `docs/introduction.md`
- `docs/reference.md`
- `docs/book.toml`
- `CLAUDE.md`
- `docs/adr/0002-grove-project-config-and-declared-mode.md`
- `docs/adr/0004-explore-split-into-grove-explore.md`
- `cli/src/init.rs` (one rustdoc comment, nice-to-have per plan §13)

No files outside this list were modified. `docs/SUMMARY.md`,
`skills/grove/SKILL.md`, and `dist/npm/README.md` were reviewed and
confirmed to need no changes (findings recorded above).
