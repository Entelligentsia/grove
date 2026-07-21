# Sprint Requirements — GROVE-S04

**Captured:** 2026-07-20
**Source:** sprint-intake interview (`/forge:new-sprint`)
**Inputs:** [`docs/adr/0004-explore-split-into-grove-explore.md`](../../../docs/adr/0004-explore-split-into-grove-explore.md)
(amends ADR 0002 §1/§3; related: ADR 0002, VISION §6.4.1,
`grove-explore-model` registry, `is-grep-enough/studies/fastcontext-sidebench/`)

---

## Background & Evidence

`grove serve` is bimodal: at startup, `determine_surface` (`cli/src/mcp.rs`)
resolves declared mode + `--explore`/`--standard` overrides + a provider health
probe into a `Surface` enum, and the server presents **either** the 7-tool
structural surface **or** a single delegating `explore` tool — never both. ADR
0004 documents why this single server identity is the root problem:

1. **`grove-cst` is a published library with a false advertisement.** Every
   consumer of the "structural code-intelligence library" compiles in an LLM
   agent loop, an OpenAI-compatible HTTP client, embedded prompts, `/proc`
   engine discovery, and JSONL trace persistence (`core/src/explore/`, ~3,900
   lines) — none of it feature-gated. `toolset.rs` re-implements Glob/Grep/Read
   with Claude-style schemas.
2. **The explore product's lifecycle already lives elsewhere** — research in
   `grove-explore-model`, weights fronted by `grove-models`, eval bed in
   `is-grep-enough` — while its runtime harness rides grove's full release
   train (tag, 5-platform build, npm, brew) for any prompt or harness change.
3. **The either/or `Surface` is a product limitation**: the best configuration
   (delegate broad "where is X" sweeps to `explore`, use `map`/`source`
   directly for precision) is unreachable.
4. **The seam has already produced bugs**: Bug-1 (GROVE-S03-T03, stale
   `explore.json` booting the wrong surface) and the standing silent
   health-probe fallback, where steering says "use `mcp__grove__explore`" while
   the server quietly serves 7 structural tools because the local model is down.

ADR 0004's decision: split into **`grove-cst` (pure library) → `grove`
(structural CLI + 7-tool MCP server) + `grove-explore` (the locator product:
agent loop, prompts, config/trace TUIs, tap, its own MCP server identity)**,
composable in `.mcp.json`. Stages 1–2 (crate split + surface split) execute
now; stage 3 (repo/release split) is **gated** on explicit triggers and is not
part of this sprint.

## Decisions (resolved from intake)

| # | Question | Decision |
|---|---|---|
| D1 | Acceptance bar for ADR gate test (a) — sidebench parity | **Full sidebench re-run.** The 347-case holdout is re-run on the split `grove-explore` binary and must match the `base-q4-v2-hf` reference (80.6) before the sprint closes. User runs/supervises the local llama.cpp rig. |
| D2 | Release/distribution work in this sprint | **Include packaging.** `release.yml`, `dist/npm`, and `dist/homebrew` ship both binaries; the sprint ends release-ready. |
| D3 | crates.io publishing | **Workspace-only.** The new explore crate stays unpublished; no `grove-cst` or `grove-explore-core` crates.io release this sprint. Publishing rides the next normal release train. |
| D4 | Docs restructure scope | **Must-have.** README delegated-mode section, `docs/setup.md`, `docs/mcp.md`, and the init-written steering templates all land the two-server story this sprint. |

## Goals

1. **`grove-cst` is honest again**: a structural-parsing library with zero LLM
   knowledge — no agent loop, no HTTP-to-LLM client, no embedded prompts, no
   trace persistence, no TUI dependencies in its tree.
2. **Two composable MCP surfaces replace the either/or.** `grove serve` always
   serves the 7 structural tools; `grove-explore` serves only the `explore`
   tool under its own server identity. A project can register either or both,
   and `init --as mcp-llm` writes exactly that composition. The
   steering-vs-served-surface mismatch class is structurally gone — each
   server's surface is a constant.
3. **No inner-loop behavior change**: the reference harness (flat v2 prompt,
   bare location lines, H1/H2 backstops, retry-on-leak) ships byte-identical,
   proven by an unchanged full sidebench run (D1).
4. **The split is release-ready**: both binaries build, test, and package
   through the existing release pipeline (D2), and all user docs + steering
   tell the two-server story (D4).

## In Scope

### 1. Stage 1 — explore crate split [must-have]
Move `core/src/explore/` (11 modules + embedded prompt) into a new workspace
crate (`explore/`; the `grove-explore-core` name is reserved but **not
published**, per D3).

**Acceptance criteria:**
- `grove-cst` contains no explore modules; its dependency tree carries no
  explore-only dependencies (HTTP chat client, prompt embeds, trace writer) —
  verifiable by `cargo tree`/`Cargo.toml` inspection.
- `GroveConfig` stays in `core/` but holds the explore section as a type
  opaque to core (owned by or re-exported from the new crate); `grove-cst`
  compiles with zero LLM types.
- The crate's public surface remains `run_explore[_reporting]`, config, and
  health types (the existing clean `mod.rs` boundary).
- Behavior-bearing harness modules (`agent.rs`, `steering.rs`,
  `grounding.rs`, `toolset.rs`, `wire.rs`, `prompts/explore_v2.system.md`)
  move **byte-identical** apart from mechanical path/import adjustments,
  demonstrated by diff in review.
- Existing unit tests move with the code; `cargo test` and
  `cargo clippy --all-targets --workspace --locked -- -D warnings` are green.

### 2. Stage 2 — `grove-explore` binary with its own MCP identity [must-have]
New bin target serving **only** the `explore` tool under its own MCP server
name, linking `grove-cst` for in-process structural ops (no MCP hop, no
subprocess — the delegate's economics depend on this).

**Acceptance criteria:**
- `grove-explore` starts an MCP server exposing exactly one tool (`explore`);
  tool name and result contract are unchanged.
- An unhealthy provider at startup is a **startup error** with an actionable
  message and non-zero exit — never a silent morph into another surface.
- Mid-session provider loss returns today's recoverable `isError` result.
- The inner explorer still reaches grove ops as direct in-process Rust calls.

### 3. Stage 2 — `grove serve` always structural; Surface machinery deleted [must-have]
`grove serve` unconditionally serves the 7-tool structural surface.

**Acceptance criteria:**
- `Surface`, `determine_surface`, the `serve --explore`/`--standard` flags,
  and the health-probe fallback are deleted; `cli/src/mcp.rs` has no
  `explore::` imports.
- `grove serve` boots the 7 structural tools regardless of declared mode,
  config contents, or provider health.
- ratatui/crossterm are no longer dependencies of the `grove` binary (they
  move with the TUIs to `grove-explore`).

### 4. Mode semantics amendment + `reconcile_harness` both-servers column [must-have]
Per the ADR's amendment to ADR 0002 §1: `mode` no longer selects a serve
surface; `mcp-llm` now means "register **both** servers and write the
delegation steering." `Mode::LEGAL` is unchanged.

**Acceptance criteria:**
- `init --as mcp-llm` registers both server entries in `.mcp.json` and writes
  steering that names both surfaces and when to use each.
- Every other mode's harness output is converged by the same single
  `reconcile_harness` writer (ADR 0002 design intact); leaving `mcp-llm`
  strips the `grove-explore` registration and its steering block.
- **Gate test (b):** the transition-matrix test is extended — every `A → B`
  mode switch leaves both server registrations, steering, and the served
  surfaces mutually consistent. Existing single-registration `mcp-llm`
  projects are migrated forward on the first `init` run (no silently broken
  registration).
- No mode is added or removed; no new tunables.

### 5. TUI + `tap` move with one-release forwarding shims [must-have]
`config_tui/`, `trace_tui/`, and `tap.rs` move wholesale to the
`grove-explore` binary.

**Acceptance criteria:**
- `grove-explore config` and `grove-explore tap` provide today's TUIs
  unchanged in function.
- `grove config` and `grove tap` remain for **one release** as forwarding
  shims that print the new spelling.
- User-facing string sweep lands in the same change: `tap`'s hint reads
  "restart the grove-explore server" (not "restart `grove serve`"), and its
  error path no longer points at the deprecated `.grove/explore.json`.
- ADR 0002 §3's mode-gated inert rendering is removed as moot: the explore
  config is always live for the explore server; any badge reflects whether
  the `grove-explore` server is registered, not the mode.
- **Config ownership (interim):** `config_tui` keeps its read-modify-write of
  grove-owned `.grove/config.json`, preserving `mode` and `harnesses` and
  rewriting only the explore section (guest-writer status is deliberate until
  stage 3).

### 6. `init --as mcp-llm` adoption funnel + graceful degrade [must-have]
The funnel (VISION §6.4.1) survives the split.

**Acceptance criteria:**
- `init --as mcp-llm` registers both servers and writes the two-surface
  steering (per item 4).
- The first-run config TUI is reached by **shelling out to
  `grove-explore config`** — `init` no longer contains the TUI.
- When `grove-explore` is not on PATH, `init` degrades gracefully: it still
  registers the server and prints "run `grove-explore config` to finish
  setup" — it does not fail. This degrade path has a test.

### 7. `doctor` re-keying [must-have]
The explore check group moves behind the product boundary.

**Acceptance criteria:**
- Explore checks run when the harness registers the `grove-explore` server,
  not when `mode == mcp-llm` implies a surface.
- Structural checks (harness drift, lock verify, registry/grammars) are
  unchanged; drift checks account for the two-registration layout.

### 8. Packaging — release-ready split (D2) [must-have]
Both binaries ride the existing release pipeline.

**Acceptance criteria:**
- `release.yml` builds and publishes **both** binaries for all 5 platforms.
- `dist/npm` installs both binaries; `dist/homebrew/update-formula.sh`
  produces a formula that ships both.
- A release dry-run (or CI equivalent) proves the pipeline end-to-end before
  sprint close; no version is actually published (D3).

### 9. Docs restructure — two-server story (D4) [must-have]

**Acceptance criteria:**
- README's delegated-mode section, `docs/setup.md`, and `docs/mcp.md` are
  rewritten to the two-server composition story (structural only / explore
  only / both).
- The init-written steering templates (CLAUDE.md/AGENTS.md blocks) match the
  docs — no references to a mode-switched single server remain.
- `CLAUDE.md` (repo orientation) and ADR 0002's superseded clauses are
  annotated to point at ADR 0004.

### 10. Gate test (a) — full sidebench re-run (D1) [must-have]

**Acceptance criteria:**
- The 347-case holdout is re-run via
  `is-grep-enough/studies/fastcontext-sidebench/` against the split
  `grove-explore` binary on the reference rig (llama.cpp, `base-q4-v2-hf`).
- The score matches the 80.6 reference (no regression attributable to the
  split); the run is recorded in the study's evidence per its conventions.
- User runs/supervises the rig; the sprint provides the split binary and any
  harness-pointing changes the bench needs to target it.

## Out of Scope

- **Stage 3 — repo/release split.** No new repository, versioning, or
  distribution channel; explicitly gated on the ADR's triggers (sid-fork
  graduation, or >1 explore-only release forced onto the grove train in a
  quarter).
- **crates.io publishing** (D3) — neither the slimmed `grove-cst` nor
  `grove-explore-core` is published this sprint.
- **Any inner-loop behavior change.** The reference harness ships
  byte-identical; the sidebench numbers must not move. No prompt, steering,
  backstop, or tool-vocabulary edits.
- **Renames of user-visible contracts.** The tool is still `explore`;
  symbol-id and location-line formats untouched; model branding stays
  `grove-explore-*`.
- **New modes or tunables.** `Mode::LEGAL` unchanged; `mcp-llm` changes
  meaning (register-both) but nothing is added or removed.
- **Config file move.** The explore section stays in `.grove/config.json`
  (ADR 0002 placement); `grove-explore` is a deliberate interim guest writer.
  Config moves only at stage 3.
- **Registry/grammar work** — nothing in the grammar pipeline changes.

## Nice-to-Have *(attempt if must-haves complete)*

- Config-TUI badge showing whether the `grove-explore` server is registered
  in the project's `.mcp.json` (the ADR's suggested replacement for the mode
  badge).
- A doctor hint when the deprecated `grove config`/`grove tap` shims are the
  spelling in any user-facing config or docs the project carries.

## Constraints

- **Workspace altitude:** engine logic stays in crates; `main.rs`/`mcp.rs`
  only dispatch and format. `grove-cst` stays clap-free and, after this
  sprint, LLM-free. `grove-explore` pins `grove-cst` exactly (as `cli` does
  today).
- **Toolchain:** cargo 1.87; crates.io deps only (no tree-sitter workspace
  path-deps). `cargo build` warning-clean;
  `cargo clippy --all-targets --workspace --locked -- -D warnings` clean
  (matches CI).
- **Single harness writer:** all `.mcp.json`/steering mutations go through
  `reconcile_harness`; only sentinel-delimited grove-authored blocks and
  grove's own server entries are touched.
- **Interim couplings are accepted but must be tested:** guest-writer
  read-modify-write of `.grove/config.json`, and `init`'s shell-out to a
  possibly-absent `grove-explore` binary — both need explicit degrade-path
  tests (the ADR names these as the two skew points).
- **Conventions:** conventional commits, no `Co-Authored-By`, files end with
  a newline, branch before committing to `main`.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Sidebench re-run (model-in-the-loop, local rig) is unavailable or noisy at sprint close, blocking gate (a) | Medium | Byte-identity diff of the moved harness modules is the code-level backstop and lands first (item 1 AC); the rig run uses the pinned reference combination (same holdout, same llama.cpp quant) to keep variance out |
| Existing single-registration `mcp-llm` projects break on upgrade (registration/steering points at a surface `grove serve` no longer has) | High | `reconcile_harness` migrates forward on first `init`; transition-matrix gate test (b) covers every `A → B`; doctor flags the stale single-registration layout |
| Silent behavior drift during the crate move (import shuffles touching harness logic) | Medium | Byte-identical move requirement + diff-based review (item 1) + gate test (a) |
| 5-platform × 2-binary packaging matrix breaks the release pipeline | Medium | Release dry-run is an explicit AC (item 8) before sprint close |
| `init` shell-out to `grove-explore` fails on PATH-less installs and takes the funnel down with it | Medium | Graceful-degrade AC with its own test (item 6) |
| Two guest writers skew `.grove/config.json` (grove writes `mode`/`harnesses`, grove-explore rewrites the explore section) | Low | Preserve today's read-modify-write semantics exactly; covered by config round-trip tests |
| Deprecation window confusion (`grove config`/`tap` shims, removed `serve` flags) | Low | Shims print the new spelling; removed flags produce a clear error naming the replacement; docs sweep is must-have (D4) |

## Carry-Over from GROVE-S03

| Item | Status | Notes |
|---|---|---|
| `GROVE-S03-T07` — `grove doctor` | Approved (not yet committed) | S04's doctor re-keying (item 7) builds directly on it; T07 should be committed before S04 planning finalizes task order |
| ADR 0002 declared mode + `reconcile_harness` + transition-matrix test | Committed | S04 amends §1 (mode no longer selects a surface) and extends the transition-matrix test to the two-registration layout; the single-writer design is preserved |
| S03-T05 config-TUI mode badge + inert explore rendering | Committed | Deliberately removed/replaced by S04 item 5 — inert rendering is moot once no surface switches on mode (ADR 0004 amendment to ADR 0002 §3) |
