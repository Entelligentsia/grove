# Sprint Requirements — GROVE-S03

**Captured:** 2026-07-04
**Source:** sprint-intake interview (`/forge:new-sprint`)
**Inputs:** [`docs/adr/0002-grove-project-config-and-declared-mode.md`](../../../docs/adr/0002-grove-project-config-and-declared-mode.md),
[`docs/doctor-command-proposal.md`](../../../docs/doctor-command-proposal.md)

---

## Background & Evidence

Two coupled pieces of work, captured from a proposed ADR and a command proposal:

1. **`.grove/config.json` — a declared project mode (ADR 0002).** Today grove's
   integration mode (`mcp` | `skill` | `both` | `mcp-llm` | `grammars`) is
   **never persisted** — `Target` is an in-memory CLI enum, and the mode is only
   *inferred* from the residue of which files a given `init` run wrote. The two
   consumers infer it from **different, un-synchronized signals**: `grove serve`
   keys its surface off the mere existence of `.grove/explore.json`
   (`mcp.rs:49`), while the agent reads whichever steering block landed in
   `CLAUDE.md` / `AGENTS.md`. Because nothing removes `.grove/explore.json`,
   **mode is sticky and emergent, not declared**, producing routing bugs on any
   `A → B` transition that touches some files but not all:
   - **Bug 1 — sticky explore surface.** `mcp-llm → mcp` rewrites `.mcp.json`
     and `CLAUDE.md`, but the surviving `.grove/explore.json` makes `serve`
     still boot the explore-only surface — steering now names seven tools the
     live server does not expose. The only thing that "fixes" it is the
     accidental fallback when the local model happens to be down.
   - **Bug 2 — orphaned `AGENTS.md`.** Only `mcp-llm` writes `AGENTS.md`; no
     other mode rewrites or removes its block, so every transition out of
     `mcp-llm` strands a stale explore-mode `AGENTS.md`.

   Root cause is singular: **no single source of truth for the mode.** ADR 0002
   consolidates it into `.grove/config.json` (with the explore backend demoted
   to a section), makes `serve` read the declared mode, and gives `init` one
   `reconcile_harness` function that converges every harness file from a single
   code path.

2. **`grove doctor` — config-mode health check (proposal).** A pre-flight
   `brew doctor` / `flutter doctor` analogue that answers "is my setup healthy?"
   It **depends on ADR 0002**: it reads the declared `mode` from
   `GroveConfig` (not `explore.json`'s existence), and its headline check
   verifies the declared mode agrees with the on-disk harness that
   `reconcile_harness` is supposed to have produced. It also surfaces the two
   failure modes that are silent today — the provider-down fallback (`serve`
   logs one stderr line the harness swallows) and declared-mode-vs-harness
   drift.

The current surface (post GROVE-S01 workspace split): engine logic in `core/`
(`core/src/explore/config.rs` = `ExploreConfig`, `core/src/registry.rs`), the
two faces in `cli/` (`cli/src/mcp.rs` = `determine_surface`, `cli/src/init.rs`
= `Target` / `write_harness`, `cli/src/config_tui/`).

## Decisions (resolved from intake)

| # | Question | Decision |
|---|---|---|
| D1 | Sprint composition | **Both docs in one sprint.** ADR 0002 (config consolidation) is the foundation; `grove doctor` tasks depend on it and are **sequenced after** the config work. |
| D2 | `explore.mode → steering` rename | **Included in this sprint.** Done now while `config.rs` is already being refactored, folded into the migration — avoids a second migration pass and removes the naming ambiguity `doctor`'s output would otherwise inherit. |
| D3 | `grove doctor` MVP gates | **All four capabilities are must-have:** harness-drift check (the headline), explore provider probe, `grove.lock` sha256 verify, and `--json` + CI exit code. |
| D4 | Legacy `.grove/explore.json` migration | **Deprecate immediately.** The one-time auto-migrate-on-read (load legacy → synthesize `mode: mcp-llm` → rewrite forward to `config.json`) is retained so no deployed `mcp-llm` project silently drops to Standard on upgrade, but the legacy path is **deprecated as of S03**: it emits a deprecation warning on use and `doctor` warns when a legacy file is present without `config.json`. It is **not** carried indefinitely. |

## Goals

1. A project's grove integration mode is a **single declared fact** in
   `.grove/config.json`, read identically by `grove serve`, the agent's
   steering, and `init`'s transition logic — ending the sticky/emergent-mode
   class of bugs.
2. Switching modes with `grove init --as <mode>` **converges every harness file**
   (`.mcp.json`, `CLAUDE.md`, `AGENTS.md`, and the surface `serve` boots) to the
   new mode from a single `reconcile_harness` code path, stripping the prior
   mode's residue.
3. `grove doctor` gives operators a **deliberate, pre-flight, read-only health
   check** that catches declared-mode-vs-harness drift and provider-down
   conditions **before** they manifest as a silently-wrong surface, and is
   usable as a CI/setup gate via its exit code.

## In Scope

### 1. `GroveConfig` — declared mode as the source of truth [must-have]
Introduce `.grove/config.json` (schema `version: 1`) holding a top-level
`mode` (`mcp` | `skill` | `both` | `mcp-llm` | `grammars`) with today's
`ExploreConfig` demoted to an `explore` section. Engine-side type lives in
`core/` (clap-free).

**Acceptance criteria:**
- A `GroveConfig` type in `core/` serialises/deserialises `{ version, mode,
  explore? }` to/from `.grove/config.json`, with fail-fast, field-named errors
  (matching today's `ExploreConfig::load` behaviour) and atomic save.
- `mode` is a legal-value enum; an illegal `mode` string is a descriptive load
  error, not a silent default.
- The `explore` section is present only when relevant (`mode == mcp-llm`) and
  round-trips the existing provider/base_url/model/allowed_tools/tap/
  trace_retain fields.
- `GroveConfig::load` reads `config.json` when present.

### 2. Legacy migration + `explore.mode → steering` rename [must-have]
One-time forward migration off the legacy `.grove/explore.json`, and the
`explore.mode` (steering level) → `steering` field rename, applied together.

**Acceptance criteria:**
- When `config.json` is absent but legacy `.grove/explore.json` exists,
  `GroveConfig::load` loads it, synthesizes `mode: mcp-llm`, and rewrites
  forward to `config.json` (so an upgraded `mcp-llm` project does **not** drop
  to Standard).
- The migration emits a **deprecation warning** that the legacy file is
  deprecated as of this release; the legacy read path is documented as slated
  for removal (not carried indefinitely).
- The inner steering-level field (standard / balanced / aggressive), previously
  `explore.mode`, is renamed to `explore.steering`; migration maps the old key
  forward. No remaining `config.mode` vs `config.explore.mode` ambiguity.
- `Mode::LEGAL` (steering levels) enforcement is preserved under the new field
  name.

### 3. `serve` reads the declared mode [must-have]
`determine_surface` (`cli/src/mcp.rs`) changes its trigger from
`explore.json.exists()` to `config.mode == mcp-llm`.

**Acceptance criteria:**
- With `config.mode == mcp-llm` and a healthy provider, `serve` surfaces
  explore-only; with any other `mode`, `serve` surfaces the standard 7
  structural tools.
- After `init --as mcp` sets `mode: "mcp"`, a subsequent `serve` boots the
  standard surface **immediately** (bug 1 fixed: the presence of a leftover
  `explore.json` no longer forces explore surface).
- The `--explore` / `--standard` runtime override flags still work and retain
  their existing precedence (`--standard` wins).
- The health-gated startup fallback (mcp-llm + unhealthy provider → standard
  surface) from GROVE-S02 is preserved.
- A shared `active_mode(root, force) -> Mode` resolution is used so `serve` and
  `doctor` agree by construction.

### 4. `reconcile_harness` — one writer converges all harness files [must-have]
Extract a single function keyed off the declared mode that writes, rewrites, or
**strips** `.mcp.json`, `CLAUDE.md`, and `AGENTS.md` to match the new mode
exactly, cleaning up the prior mode's residue. `init` reads the prior `mode`
from `config.json`, calls `reconcile_harness`, then persists the new `mode`.

**Acceptance criteria:**
- `init --as <mode>` records `mode` verbatim in `config.json` on every run.
- Leaving `mcp-llm` **removes** the grove-authored explore block from
  `AGENTS.md` (bug 2 fixed) and drops the `--explore` arg from `.mcp.json`.
- Only grove-authored, sentinel-delimited blocks and grove's own `.mcp.json`
  entry are ever modified or stripped — host content is never touched.
- A **transition-matrix test** asserts that for every `A → B` mode switch, the
  resulting `.mcp.json`, `CLAUDE.md`, `AGENTS.md`, and the surface `serve`
  would boot are mutually consistent.
- Existing `init --as mcp|skill|both` behaviour is otherwise unchanged
  (regression: current `tests/cli.rs` init assertions still pass, updated only
  where they must reflect the new `config.json` write).

### 5. `config` TUI — display-consistent, read-only on mode [must-have]
The TUI gains read awareness of the declared mode but **does not** gain a mode
selector (switching mode stays `grove init --as`, preserving a single harness
writer).

**Acceptance criteria:**
- The TUI shows the active `mode` (read from `config.json`) as a header/badge.
- When `mode == mcp-llm`: the explore section is live and editable, as today.
- When `mode != mcp-llm`: the explore fields render inert/greyed with a
  one-line note that they are dormant until `grove init --as mcp-llm`.
- The TUI never mutates `mode` (it cannot become a second harness writer).

### 6. `grove doctor` — read-only config-mode health check [must-have]
New CLI verb `grove doctor [path] [--explore] [--standard] [--json]`, backed by
a new `core::doctor` module returning a typed `Report`. Pure / read-only — no
writes. Composes existing primitives plus the new harness-consistency and
lock-verify checks.

**Acceptance criteria:**
- `core::doctor::diagnose(root, force) -> Report` returns typed
  `Check { group, name, status: Ok|Warn|Fail|Info, detail, hint }` values and a
  declared `Mode` tag (the `core` `Mode`, not the CLI `Surface`).
- No mode flag → auto-detects the active mode exactly as `serve` does
  (`config.mode == mcp-llm` ⇒ explore; else standard), via the shared
  `active_mode` helper; `--explore` / `--standard` force a mode (same
  precedence as `serve`).
- **Universal checks** run for every project: grove version; `config.json`
  present + loads + legal `mode`; **harness-consistency (the headline)** —
  `config.mode` agrees with `.mcp.json` grove args, the steering blocks in
  `CLAUDE.md` / `AGENTS.md`, and the surface `serve` would boot, **failing on
  drift and naming the mismatched file**; legacy `explore.json` present without
  `config.json` → **warn** (pending migration); registry root resolution +
  search order; grammar cache presence; detected project languages vs available
  grammars.
- **`grove.lock` sha256 verify (new primitive):** when a lockfile is present,
  recompute each cached wasm's sha256 and compare to the lock's `wasm` field;
  **fail on mismatch**.
- **Explore-mode checks** (resolved/forced explore): explore section validates;
  `steering` legal; provider **reachable** (`health_probe` →
  `HealthError::Unreachable`) and configured **model served**
  (`HealthError::ModelMissing`, enriched via `list_models`), surfacing the
  built-in fix hints **verbatim**; `allowed_tools` recognised (warn on unknown);
  tap / trace_retain reported as info.
- **`--json`** emits the machine-readable report (`checks[]` + `summary`);
  `--json` changes the format, not the gate.
- **Exit code:** 0 when all checks pass (warnings allowed), **non-zero** on any
  hard failure — under both human and `--json` output, so `doctor` is a usable
  CI/setup gate.
- Engine logic stays in `core::doctor`; `cli/src/main.rs` only dispatches and
  formats (human table or `--json`) and sets the exit code from `Report::ok()`.

## Out of Scope

- **Mode changes from the `config` TUI.** `grove init --as` remains the only way
  to switch mode (single harness writer; avoids a second divergence source).
- **New config tunables beyond `mode`.** This sprint consolidates existing
  state; it does not add new knobs.
- **Auto-remediation in `doctor`** (pulling a missing model, running
  `grove fetch`, rewriting drifted harness files). `doctor` reports and hints;
  it does not mutate. (Reconciliation is `init`'s job.)
- **Changes to the seven structural tools' contracts** or the `check` verb
  (which checks file *syntax*, not config — distinct from `doctor`).
- **Windows-specific cache-path checks** beyond what `dirs::cache_dir()`
  abstracts.
- **Probing multiple explore configs at once** — `doctor` inspects one
  project's resolved (or forced) mode.
- **Indefinite legacy `explore.json` support** — deprecated immediately (D4).

## Nice-to-Have *(attempt if must-haves complete)*

- Surface the `Report` as an MCP tool and/or reuse it in `config`
  (validate-on-save) and `serve` (replace the silent-fallback stderr line with a
  structured warning) — the typed report makes this rework-free later.
- A concrete removal target version for the legacy `explore.json` read path,
  with the deprecation warning naming it.

## Constraints

- **Workspace altitude:** engine logic (`GroveConfig`, migration, `core::doctor`,
  lock verify) lives in `core/` (grove-core, clap-free); `cli/` stays thin
  (`main.rs` / `mcp.rs` / TUI presentation only). `core` must **not** depend on
  the CLI-side `Surface` type.
- **Single harness writer:** `reconcile_harness` (called only from `init`) is the
  one place mapping `mode → on-disk harness state`; it strips only
  sentinel-delimited grove-authored blocks and grove's own `.mcp.json` entry.
- **`.grove/` is grove's own directory:** writing `.grove/config.json` is
  consistent with the `grammars` contract (which touches no host files) because
  `.grove/` is grove-owned, unlike `CLAUDE.md` / `.mcp.json`.
- **Toolchain:** cargo 1.87; crates.io deps only (no tree-sitter workspace path
  deps). `cargo build` warning-clean; `cargo clippy -- -D warnings` clean.
- **Conventions:** conventional commits, files end with a newline, no
  `Co-Authored-By`.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Migration bug drops a deployed `mcp-llm` project to Standard on upgrade (the exact failure ADR 0002 warns of) | High | Migration is an explicit must-have with its own acceptance test; auto-migrate-on-read synthesizes `mode: mcp-llm` from legacy `explore.json` before any surface decision |
| `reconcile_harness` strips or corrupts host-authored content in `CLAUDE.md` / `AGENTS.md` / `.mcp.json` | High | Operate only on sentinel-delimited grove blocks and grove's own `.mcp.json` entry; never touch host content; cover with transition-matrix + host-content-preserved tests |
| Transition matrix is combinatorial (5 modes → 20 ordered switches) and easy to under-test | Medium | One table-driven test enumerating every `A → B`, asserting all four artifacts consistent after a clean `init` and drift-detected on hand-induced mismatch (shared fixtures with `doctor`'s harness-consistency test) |
| `doctor` and `serve` drift in how they resolve the active mode | Medium | Shared `active_mode(root, force)` helper is the single resolver both call |
| New `grove.lock` sha256 verify is a fresh primitive (not reusing an existing one) | Medium | Isolate as a small, unit-tested function; only runs when a lockfile is present |
| Rename `explore.mode → steering` breaks existing `.grove/explore.json` on read | Medium | Migration maps the old key forward as part of the one-time rewrite; covered by a migration test |
| `config` TUI inert-state rendering interacts with existing model-list dropdown / tap toggle | Low | TUI change is display-only (badge + greyed fields); no new mutation paths |

## Carry-Over from GROVE-S02

| Item | Status | Notes |
|---|---|---|
| mcp-llm mode / `ExploreConfig` / explore surface / health-gated fallback | Active (built) | S03 consolidates `ExploreConfig` into `GroveConfig.explore`, replaces the `explore.json.exists()` surface trigger with declared `mode`, and preserves the S02 startup-probe fallback semantics |
| `grove config` TUI (model-list dropdown, tap toggle) | Active (built) | S03 adds a mode badge + inert-state rendering; does not add a mode selector |
| mcp-llm still experimental / unreleased | Active | S03 hardens its configuration story (declared mode, migration, doctor) but does not change its experimental status |
