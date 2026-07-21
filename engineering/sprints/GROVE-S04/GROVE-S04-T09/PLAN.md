# PLAN — GROVE-S04-T09: Docs restructure — two-server story (README, setup, mcp, steering templates, ADR annotations)

🗻 *grove Architect*

**Task:** GROVE-S04-T09
**Sprint:** GROVE-S04
**Estimate:** M

---

## Objective

Every user-facing and developer-facing document that describes how an agent
reaches grove must tell the **two-server** story that T01–T08 already shipped
in code — `grove` (always the 7-tool structural MCP server) and `grove-explore`
(its own MCP identity, the LLM-delegating locator) are **composable**, not a
single server that switches surface on a declared mode. No surviving prose may
describe `serve --explore`/`--standard`, a health-probe fallback that morphs
the server identity, a mode badge, or inert TUI rendering. The init-written
steering blocks (T04's `claude_section`/`agents_section` in `cli/src/init.rs`)
are the ground truth for server names, verb spellings, and the recommended
delegation flow — every doc that describes the same territory must name the
same things. ADR 0002 gets pointer annotations on its now-superseded clauses;
ADR 0004 moves to Accepted with its implementation state noted (stages 1–2
shipped via T01–T08; stage 3 remains gated; gate test (a) is T10, still open).

## Approach

**Ground truth gathered before writing docs** (so the prose is checked against
the shipped artifact, not against the ADR's proposal-stage design):

- `cli/src/init.rs::claude_section`/`agents_section` (McpLlm branch, ~L923–1000
  and L846–867) is the canonical steering text: two servers —
  `mcp__grove__*` (7 tools: `outline`/`symbols`/`source`/`callers`/`definition`/
  `map`/`check`) and `mcp__grove-explore__explore` — with a 3-step recommended
  flow (narrow `explore` question → `source`/`map` on the cited `file:line` →
  synthesize). Every doc section describing "when to use each surface" must
  match this, not invent a different framing.
- `cli/src/main.rs` `Cmd::Serve` — `--explore`/`--standard` are still parsed
  (kept `hide = true` for a clear error) but **both now `bail!` with a message
  naming `grove-explore serve` as the replacement** — no fallback, no silent
  mode resolution.
- `cli/src/main.rs` `Cmd::Config`/`Cmd::Tap` on the `grove` binary are
  **forwarding shims**: they print `` note: `grove config` is deprecated; use
  `grove-explore config` instead `` (same pattern for `tap`), shell out to the
  sibling binary (`find_explore_binary()`), and forward the exit code.
  `grove-explore`'s own `Cmd::Config`/`Cmd::Tap` are the canonical verbs.
- Workspace layout is now four crates (`Cargo.toml` `[workspace] members`):
  `core` (`grove-cst`, pure structural engine, no LLM code), `explore`
  (`grove-explore-core`, the inner agent loop — moved wholesale out of
  `core/src/explore/`), `cli` (the `grove` binary — `main.rs`/`mcp.rs`/`init.rs`
  only; `config_tui`/`trace_tui`/`tap.rs` are **gone** from `cli/src`), and
  `grove-explore` (the `grove-explore` binary — MCP server + the two TUIs +
  `tap`, at `grove-explore/src/{main,config_tui,trace_tui,tap}.rs`).
- All ten GROVE-S04 tasks except this one (T09) and the sidebench gate (T10)
  are `committed` — stages 1–2 of ADR 0004 are implemented, not proposed.

**Doc-by-doc rewrite**, each checked against the ground truth above:

1. **README.md** — replace the entire `<details><summary>Delegated local-LLM
   mode</summary>` block under `## Advanced`. New content: what `grove-explore`
   is (its own MCP server, not a mode of `grove serve`), the composition story
   (register structural only / explore only / both — `grove init --as
   mcp-llm` gives you both), when to reach for `explore` vs. the structural
   tools directly (mirrors the steering flow), the `grove-explore
   config`/`grove-explore tap` verbs with the deprecated-shim note, and
   corrected health semantics (unhealthy provider at `grove-explore` startup
   = a startup error naming the fix, not a silent surface swap; mid-session
   loss keeps the recoverable `isError`). Drop the `docs/assets/
   explore_delegation_flow.svg` embed (it renders the old single-surface
   flow) rather than editing a binary asset out of scope for a prose task —
   replace it with the equivalent explanation in prose/table form.
2. **docs/setup.md** — rewrite `## Explore-mode — grove init --as mcp-llm`:
   `--as mcp-llm` registers **both** servers in `.mcp.json` and writes
   dual-surface steering to `CLAUDE.md` **and** `AGENTS.md`; no health
   fallback — an unhealthy provider fails `grove-explore serve` at startup
   with an actionable message. Update the "What init writes" bullets (two
   `.mcp.json` entries, not `serve --explore`) and the `--as` one-line
   summary table. First-run TUI note stays accurate (`init` shells out to
   `grove-explore config` when the sibling binary is present; degrades
   gracefully — registers the server anyway and prints the follow-up command
   — when it isn't, per T06). Drop the `assets/mcp_explore_comparison.svg`
   embed for the same reason as README's.
3. **docs/mcp.md** — add a section (after "Same engine as the CLI" or as a new
   "Two servers, composable" section) naming both server identities
   (`grove`/`mcp__grove__*` and `grove-explore`/`mcp__grove-explore__explore`),
   what each is for, and that a project can register either or both — link to
   `setup.md` for the `--as mcp-llm` wiring flow. Keep the existing
   protocol/schema/error-model content (still accurate for `grove serve`,
   which is unconditionally the 7-tool surface now).
4. **docs/introduction.md** — the "One engine, four surfaces" table's MCP row
   (`grove serve --explore`) is wrong twice over (there's no such flag, and
   explore is a separate binary, not a flag on `grove serve`). Restate as:
   CLI (`grove <verb>`) · MCP structural (`grove serve`) · MCP explore
   (`grove-explore serve`) · Library (`grove-cst`) — four surfaces, still, but
   correctly named — and that MCP structural + MCP explore compose. Drop the
   `assets/grove_architecture_surfaces.svg` embed (bakes in `serve --explore`
   as literal SVG text) for the same reason as the other two; replace with
   the corrected table/prose.
5. **docs/reference.md** — `.grove/config.json` section: `mode: mcp-llm` no
   longer means "`grove serve` runs the explore surface" — it means "`init`
   registers both `grove` and `grove-explore` in the harness"; `grove serve`
   is unconditionally structural. Update the `tap`/`config` cross-reference
   to `grove-explore tap`/`grove-explore config` (old spellings noted as
   deprecated shims, matching the steering text).
6. **docs/SUMMARY.md** — no page adds/removes are planned; if any section
   above grows enough to warrant a new page (e.g., splitting the two-server
   explanation out of `mcp.md`), update the sidebar accordingly and keep the
   entry labels honest about what's actually on `mcp.md` vs `setup.md`.
   Otherwise leave structure as-is. Either way, run `mdbook build` from
   `docs/` afterward (baseline today: builds clean) and fix any broken
   internal link the rewrite introduces.
7. **docs/book.toml** — `description` says "two MCP modes"; correct to "two
   MCP servers" to match the composition story (it's rendered book metadata,
   in scope for the sweep).
8. **CLAUDE.md** (repo dev orientation — NOT the init-written steering block;
   this is the hand-authored file living at the repo root) — this is the
   heaviest rewrite:
   - `## Architecture — one engine, two faces` header and file-tree: rename/
     reframe to reflect one engine (`grove-cst`) behind **two binaries**
     (`grove`: CLI + always-structural MCP; `grove-explore`: its own MCP
     identity + the two TUIs + `tap`). Replace the file tree with the real
     layout: `core/src/*` (no `explore/` subdirectory anymore), the new
     top-level `explore/src/*` crate (`grove-explore-core` — same module list
     as today's tree, just moved and re-homed), `cli/src/{main,mcp,init}.rs`
     only (no `config_tui`/`trace_tui`/`tap.rs`), and
     `grove-explore/src/{main,config_tui/,trace_tui/,tap.rs}`.
   - The "mcp-llm mode is opt-in" paragraph: replace "single surface + health
     fallback" framing with "register-both + `grove-explore` startup fails
     hard on an unhealthy provider."
   - `## Commands`: `grove serve [path] [--explore] [--standard]` line is
     wrong — split into `grove serve [path]` (always structural; the two
     flags are removed and error) and a new `grove-explore serve [path]`
     line; `grove config`/`grove tap` become one-line deprecated-shim notes
     forwarding to the new `grove-explore config [path]`/`grove-explore tap
     [path] [--no-enable]` lines.
   - Everything else in CLAUDE.md (grammar system, registry/cache, build/test/
     run, conventions, design decisions, roadmap, gotchas) is unaffected —
     leave untouched.
9. **docs/adr/0002-grove-project-config-and-declared-mode.md** — add a short
   annotation (not a rewrite; ADR text is a historical record) at:
   - **§1 "`serve` reads the declared mode"** — a one-line note that this
     clause is superseded: `serve` no longer selects a surface at all (there
     is nothing left to select); see ADR 0004.
   - **§3 "the `config` TUI becomes display-consistent"** (mode badge /
     inert-field rendering) — a one-line note that this is moot post-split:
     the explore config is always live for the `grove-explore` server; any
     badge reflects whether `grove-explore` is registered, not the declared
     mode; see ADR 0004.
   Leave the rest of the document (context, alternatives, consequences)
   as-is — it's the accurate history of why `.grove/config.json` exists.
10. **docs/adr/0004-explore-split-into-grove-explore.md** —
    - `Status: Proposed` → `Status: Accepted`.
    - `## Status of implementation` — replace "Proposed — not yet
      implemented" with the actual state: stages 1–2 implemented (crate
      split, `grove-explore` binary + own server identity, `mcp.rs` surface
      deletion, `reconcile_harness` both-servers column + forward migration,
      TUI/`tap` move + shims, `init --as mcp-llm` funnel, `doctor`
      re-keying, packaging — GROVE-S04-T01 through T08, all committed);
      gate test (a), the full sidebench re-run, is GROVE-S04-T10 and remains
      open; gate test (b), the extended transition-matrix test, shipped with
      T04. Stage 3 (repo/release split) remains not scheduled, gated on the
      triggers already stated.
11. **skills/grove/SKILL.md** — reviewed: it documents grove's *structural*
    code-navigation tools (`mcp__grove__*`) for the coding-agent skill, and
    never mentions explore mode, `serve --explore`, or any mode-switched
    surface. No stale content found — no edit needed. Record this finding
    (checked, clean) in PROGRESS.md as evidence of the sweep.
12. **dist/npm/README.md** — reviewed: describes the `grove` package/binary
    generically (`grove init`, `grove serve`, the seven tools); no mention of
    explore mode at all. No stale content found — no edit needed. Record this
    finding in PROGRESS.md as evidence of the sweep.
13. **Explicitly out of scope** (reviewed, deliberately not touched — record
    the reasoning in PROGRESS.md):
    - `docs/doctor-command-proposal.md`, `docs/code-quality-review-2026-07-03.md`,
      `docs/release-0.3.0-plan.md` — dated, historical planning artifacts, not
      linked from `docs/SUMMARY.md` (not part of the built book), not
      user-facing documentation of current behavior. `doctor-command-proposal.md`
      in particular describes a `doctor --explore/--standard` design that was
      never shipped that way (today's `grove doctor` takes no such flags) —
      it's a proposal doc, not a description of current behavior; leaving it
      untouched doesn't violate AC3 (which targets docs describing *current*
      usage).
    - `docs/assets/*.svg` — regenerating diagrams is out of scope for a
      prose-only task (Operational Impact: "Regeneration: none"); the three
      stale embeds (`explore_delegation_flow.svg`, `mcp_explore_comparison.svg`,
      `grove_architecture_surfaces.svg`) are **dropped from the rewritten
      prose** (items 1, 2, 4 above) rather than left showing an inaccurate
      picture, and replaced with equivalent text/table content.
    - `cli/src/init.rs` `Target::McpLlm` doc comment (rustdoc, ~L38–42) still
      describes `.mcp.json` with `serve --explore` — a genuine discrepancy,
      but it's a code file, not listed in the task's Artifacts Involved, and
      out of a prose-only task's scope. **Nice-to-have, only if the must-haves
      above are done and there's time**: a one-line rustdoc correction (no
      behavior change, `cargo test`/`clippy` unaffected). Do not let this
      expand into a code-review-worthy change; if touched, it is the only
      `.rs` file this task edits, and only that one doc comment.

## Files to Modify

| File | Change | Rationale |
|---|---|---|
| `README.md` | Rewrite `Advanced → Delegated local-LLM mode` section; drop stale svg embed | AC1, AC3 |
| `docs/setup.md` | Rewrite `Explore-mode — grove init --as mcp-llm` section + "What init writes"; drop stale svg embed | AC1, AC3 |
| `docs/mcp.md` | Add two-servers/composability section | AC1 |
| `docs/introduction.md` | Correct "four surfaces" table row; drop stale svg embed | AC3 |
| `docs/reference.md` | Correct `.grove/config.json` `mode` semantics; `grove-explore tap`/`config` spellings | AC3 |
| `docs/SUMMARY.md` | Verify/adjust sidebar labels if page structure changes | AC5 |
| `docs/book.toml` | Correct "two MCP modes" → "two MCP servers" in description | AC3 |
| `CLAUDE.md` | Rewrite architecture block (file tree, two binaries), Commands section (serve flags removed, grove-explore verbs, deprecated shims) | AC4 |
| `docs/adr/0002-grove-project-config-and-declared-mode.md` | Annotate §1 and §3 pointing at ADR 0004 | AC4 |
| `docs/adr/0004-explore-split-into-grove-explore.md` | Status → Accepted; implementation-state note | AC4 |
| `skills/grove/SKILL.md` | Reviewed, no changes — record finding | AC3 (sweep evidence) |
| `dist/npm/README.md` | Reviewed, no changes — record finding | AC3 (sweep evidence) |
| `cli/src/init.rs` (nice-to-have only) | One-line rustdoc fix on `Target::McpLlm` | discovered discrepancy, optional |

## Data Model Changes

None. No `.forge/store/` or config schema touched.

## Testing Strategy

- `cargo test --release --locked` — the project's standard test command;
  must remain green, confirming no accidental behavior change slipped in
  through the (at most one, optional) `.rs` doc-comment edit.
- `cargo clippy --all-targets --workspace -- -D warnings` — only relevant if
  the optional `init.rs` rustdoc line is touched; run it regardless as a
  cheap confirmation nothing else drifted.
- `mdbook build` (run from `docs/`) — confirms the book still compiles after
  `SUMMARY.md`/`book.toml` edits; baseline today is a clean build with no
  warnings, so any new warning is a regression to fix before this task closes.
- Manual grep sweep (evidence goes in PROGRESS.md), scoped to files this task
  claims are clean of stale mode-switch language — `docs/`, `README.md`,
  `CLAUDE.md`, `skills/grove/SKILL.md`, `dist/npm/README.md` — for
  `serve --explore`, `serve --standard`, an unqualified `--explore`/
  `--standard` flag reference, "mode badge", and "inert" rendering, **excluding**
  `docs/adr/*.md` (ADRs are historical decision records — they are *expected*
  to describe the old design in their Context/Alternatives sections; only
  their Status / annotation lines need to be current) and the three
  explicitly out-of-scope planning docs (item 13 above).
- Cross-check: read `cli/src/init.rs::claude_section`/`agents_section` (McpLlm
  branch) again after the docs rewrite and confirm the server names, verb
  spellings (`grove-explore config`/`tap`), and 3-step recommended flow the
  docs now describe match that source verbatim in substance (not necessarily
  word-for-word) — this is AC2's parity check.

## Acceptance Criteria

- [ ] README's delegated-mode section, `docs/setup.md`, and `docs/mcp.md`
      describe the two-server composition story: structural only / explore
      only / both, when to use each, `grove-explore` as its own server.
- [ ] The rewritten docs name the same server identities
      (`mcp__grove__*`, `mcp__grove-explore__explore`), the same verb
      spellings (`grove-explore config`/`tap`, with `grove config`/`grove tap`
      noted as deprecated shims), and the same recommended delegation flow as
      `cli/src/init.rs`'s `claude_section`/`agents_section` (McpLlm branch).
- [ ] No remaining doc (outside `docs/adr/*.md` history and the three
      explicitly out-of-scope planning docs) describes `serve --explore`/
      `serve --standard`, a health-probe fallback that swaps server surface,
      a mode badge, or inert TUI rendering. Sweep evidence (including the
      "reviewed, no change needed" files) is in PROGRESS.md.
- [ ] `CLAUDE.md`'s architecture block reflects the real four-crate workspace
      layout and the two-binary/composable-surface model; its Commands
      section shows `grove serve` with no mode flags, plus the
      `grove-explore serve`/`config`/`tap` verbs and the deprecated-shim
      notes. ADR 0002 §1 and §3 carry annotations pointing at ADR 0004. ADR
      0004's status is Accepted with implementation state noted (T01–T08
      committed, T10 gate test open, stage 3 not scheduled).
- [ ] `docs/SUMMARY.md` and `docs/book.toml` are consistent with any
      structural change; `mdbook build` from `docs/` succeeds with no new
      warnings.
- [ ] `cargo test --release --locked` is green (no behavior change).

## Operational Impact

- **Version bump:** none — docs ride the same release as the rest of GROVE-S04.
- **Regeneration:** none — no schema, no generated artifact; the stale SVG
  diagrams are dropped from the rewritten prose rather than regenerated.
- **Security scan:** not required — no code-path change (the one optional
  `.rs` edit, if made, is a rustdoc comment with zero behavioral effect).
- **Backwards compatibility:** none affected — this task changes documentation
  only; the deprecated-shim behavior it documents (`grove config`/`grove tap`
  forwarding) already shipped in T05.
