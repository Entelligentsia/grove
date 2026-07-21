# VALIDATION REPORT — GROVE-S04-T09: Docs restructure — two-server story

(standalone review)

**Verdict:** Approved

## Acceptance Criteria — pass/fail with evidence

### AC1 — README/setup/mcp describe the two-server composition story
**PASS.** Independently read all three:
- `README.md` Advanced section ("grove-explore — a second, composable MCP
  server"): explicit "Composition, not a mode switch" callout; documents
  register-`grove`-only / `grove-explore`-only / both; setup block shows
  `grove init --as mcp-llm` registering both; health semantics section states
  unhealthy startup = hard error, "never silently swapping in a different
  tool surface."
- `docs/setup.md` "Explore-mode — `grove init --as mcp-llm`" section: "Opt-in,
  and a second server, not a mode"; "registers **both servers** in
  `.mcp.json`"; explicit no-fallback statement; "What it writes" table lists
  both servers.
- `docs/mcp.md` new "Two servers, composable" section: table of both server
  identities/MCP keys/what each serves; "the two servers are composable, not
  exclusive."

### AC2 — docs match `cli/src/init.rs` ground truth (server identities, verb spellings, delegation flow)
**PASS.** Re-read `claude_section`/`agents_section` McpLlm branches
(L847–1000) directly. Ground truth: `mcp__grove__*` (7 named tools),
`mcp__grove-explore__explore`, 3-step flow (narrow `explore` question →
`source`/`map` on cited `file:line` → synthesize). Grepped README.md,
docs/setup.md, docs/mcp.md, docs/reference.md, CLAUDE.md — all use identical
identities, identical flow wording, and identical deprecated-shim framing
(`grove config`/`grove tap` → `grove-explore config`/`grove-explore tap`).

### AC3 — no remaining stale mode-switch language outside excluded categories
**PASS.** Independent greps (not trusting PROGRESS.md's reproduction):
- `serve --explore|serve --standard`: zero hits in any in-scope prose file;
  remaining hits confined to `docs/adr/*.md` history, `docs/assets/*.svg`
  (unlinked binary assets), and `docs/doctor-command-proposal.md` (explicit
  out-of-scope planning doc) — exactly the three excluded categories AC3
  names.
- `mode badge|inert`: zero hits outside `docs/adr/0002` and `docs/adr/0004`
  (both ADR history — 0002's hit is the ADR-0004-annotated §3 block itself).
- `health.probe|falls? back to|swap.*server`: CLAUDE.md's two hits describe
  the real `health_probe()` function and the startup health *gate* (hard
  `exit(1)`, "never falls back") — this is accurate architecture, not stale
  mode-switch language.
- `skills/grove/SKILL.md` and `dist/npm/README.md`: independently reconfirmed
  clean (zero hits) — PROGRESS.md's "no edit needed" finding holds.

### AC4 — CLAUDE.md architecture/Commands + ADR 0002/0004 annotations
**PASS.**
- CLAUDE.md "Architecture — one engine, two binaries" section lists the real
  four-crate workspace (`cli/`, `core/`, `explore/` → `grove-explore-core`,
  `grove-explore/`) — verified against actual `ls` of the repo root.
- Commands section: `grove serve [path]` with no mode flags, comment
  documenting `--explore`/`--standard` as removed/`bail!`ing; separate
  `grove-explore serve`/`config`/`tap` block with "canonical verb spellings";
  `grove config`/`grove tap` under a "deprecated shims" heading.
- ADR 0002 §1 carries a blockquote: "Superseded by ADR 0004 §2... nothing
  left for a mode to select." §3 carries a blockquote: "Moot per ADR 0004
  §2... no inert/greyed state to render." Historical body text left intact
  in both cases (verified original prose still present below each
  annotation).
- ADR 0004: `Status: Accepted` (line 3). "Status of implementation" section
  states Stages 1–2 implemented, GROVE-S04-T01–T08 committed with itemized
  detail; Gate test (a) = GROVE-S04-T10, open; Gate test (b) shipped with
  T04; Stage 3 not scheduled. Matches AC4's required wording precisely.

### AC5 — SUMMARY.md/book.toml consistency + mdbook build
**PASS.** `docs/book.toml` description now reads "...a Rust library, a CLI,
and two composable MCP servers over one tree-sitter engine." `docs/SUMMARY.md`
sidebar entries match content (including the ADR 0004 entry, which is new —
see Advisory below). Independently ran `cd docs && mdbook build`: clean,
`INFO HTML book written to .../site/docs`, no warnings.

### AC6 — `cargo test --release --locked` green
**PASS.** Independently reproduced: all workspace crates pass — 139 + 65 +
43 + 42 + 54 + 1 + 0 = matches PROGRESS.md's reported counts exactly, 0
failed. Also independently ran `cargo clippy --all-targets --workspace -- -D
warnings`: clean, zero warnings. `git diff cli/src/init.rs` confirms the only
`.rs` change is a 5-line rustdoc comment on `Target::McpLlm` — no logic
touched, consistent with "no behavior change."

## Regression check
`git status --short` shows modifications confined to the plan's declared
file list (README.md, CLAUDE.md, cli/src/init.rs, docs/{SUMMARY.md,
adr/0002-*.md, adr/0004-*.md (new), book.toml, introduction.md, mcp.md,
reference.md, setup.md}) plus forge-store/engineering-KB bookkeeping. No
unexpected source or test file touched. Full workspace test suite green
(AC6) — no regression.

## Advisory notes carried forward (non-blocking, already logged in CODE_REVIEW.md)
- PROGRESS.md's "Files reviewed, no edit needed" framing for `docs/SUMMARY.md`
  undercounts: T09 added the ADR 0004 sidebar entry (file was untracked
  before this task). The resulting content is correct and required by AC5 —
  this is a provenance-labeling nit in PROGRESS.md's prose, not a doc defect.
- `docs/setup.md` line 25's "three faces" phrasing (describing the `--as
  mcp|skill|both` trio) reads slightly imprecise now that there are two MCP
  server binaries, but is contextually scoped correctly and not in conflict
  with any acceptance criterion.

Both advisories were already surfaced by CODE_REVIEW.md and independently
reconfirmed here; neither blocks approval — no acceptance criterion requires
correcting either.

## Conclusion
All 6 acceptance criteria independently verified with fresh evidence (not
merely re-reading PROGRESS.md's claims): fresh `cargo test`, fresh `cargo
clippy`, fresh `mdbook build`, fresh grep sweeps, fresh re-read of
`cli/src/init.rs` ground truth, fresh diff of the only `.rs` change, and
fresh `git status` file-list cross-check against the plan. No gaps, no
regressions, no unmet criteria found.
