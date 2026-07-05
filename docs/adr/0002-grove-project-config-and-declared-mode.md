# ADR 0002 — `.grove/config.json`: a declared project mode

- **Status:** Proposed
- **Date:** 2026-07-04
- **Deciders:** Boni Gopalan
- **Supersedes:** —
- **Related:** `cli/src/init.rs` (`Target`, `write_harness`), `cli/src/mcp.rs`
  (`determine_surface`), `core/src/explore/config.rs` (`ExploreConfig`),
  `cli/src/config_tui/`, VISION §6.4.1 (availability ≠ adoption)

## Context

`grove init --as <mode>` wires a project for one of five integration modes —
`mcp`, `skill`, `both`, `mcp-llm`, `grammars`. Each mode writes a **different
subset** of on-disk state:

| Mode | `.mcp.json` grove args | `CLAUDE.md` block | `AGENTS.md` block | `.grove/explore.json` |
|---|---|---|---|---|
| `mcp` | `["serve"]` | MCP steering | *untouched* | *untouched* |
| `skill` | *untouched* | skill steering | *untouched* | *untouched* |
| `both` | `["serve"]` | MCP steering | *untouched* | *untouched* |
| `mcp-llm` | `["serve","--explore"]` | explore steering | explore steering (written) | created via TUI |
| `grammars` | *untouched* | *untouched* | *untouched* | *untouched* |

The mode itself is **never persisted** — `Target` is an in-memory CLI enum. A
project's "mode" is only *inferred* from the residue of which files a given run
happened to write, and the two consumers infer it from **different,
un-synchronized signals**:

- **`grove serve`** picks its surface from `ExploreConfig::config_path(root).exists()`
  (`mcp.rs:49`) — the mere presence of `.grove/explore.json`, not the `.mcp.json`
  args. In explore surface, `tools/list` returns **only** `explore`; the seven
  structural tools are hidden (unless the provider health-probe fails, which
  silently falls back to Standard).
- **The agent** reads whichever steering block is in `CLAUDE.md` / `AGENTS.md`.
- **A re-run of `init`** is stateless: it does not read the prior mode, it only
  overwrites the subset of files its new `--as` targets.

Because nothing removes `.grove/explore.json` (grepped: no `remove_*` call targets
it) and `serve` keys off its existence, **mode is sticky and emergent, not
declared.** This produces routing bugs on any `A → B` transition that touches
some files but not all:

1. **`mcp-llm → mcp` (or `both`).** `init` rewrites `.mcp.json` to `["serve"]` and
   `CLAUDE.md` to standard MCP steering (naming `outline`/`symbols`/…), but
   `.grove/explore.json` survives, so `serve` **still boots the explore surface
   exposing only `explore`.** `CLAUDE.md` now steers the agent to seven tools the
   live server does not expose. The only thing that "fixes" it is the accidental
   fallback when the local model happens to be down — behaviour that depends on
   whether a background LLM is running.
2. **Orphaned `AGENTS.md`.** Only `mcp-llm` ever writes `AGENTS.md`; no other mode
   rewrites or removes its block. Every transition out of `mcp-llm` strands a
   stale explore-mode `AGENTS.md`, misdirecting any `AGENTS.md`-reading harness.
3. **`skill` / `grammars` never rewrite `.mcp.json`,** so a prior `--explore`
   registration persists under steering that no longer mentions explore.

The root cause is singular: **there is no single source of truth for the mode**,
so the transition logic can't clean up and the runtime can't agree with the
steering. `.grove/` is already grove's sovereign, project-local directory (it
holds `grammars/`, `explore.json`, `traces/`) — the natural home for a declared
mode.

## Decision

Introduce **`.grove/config.json`** as the single declared source of truth for a
project's grove integration, with the explore backend demoted to a section:

```json
{
  "version": 1,
  "mode": "mcp-llm",
  "explore": {
    "provider": "ollama",
    "base_url": "http://localhost:11434/v1",
    "model": "qwen2.5-coder:7b",
    "steering": "standard",
    "allowed_tools": ["grove", "rg", "grep", "find"],
    "tap": false,
    "trace_retain": 50
  }
}
```

- **`mode`** records the init `Target` verbatim (`mcp` | `skill` | `both` |
  `mcp-llm` | `grammars`). It is written by `grove init` on every run.
- **`explore`** is today's `ExploreConfig`, present only when relevant. Its inner
  `mode` field (steering level: standard/balanced/strict) is renamed to
  **`steering`** to remove the `config.mode` / `config.explore.mode` ambiguity —
  optional polish, not load-bearing (see Scope boundary).

Three independent changes, each keyed off the declared `mode`:

### 1 — `serve` reads the declared mode

`determine_surface` (`mcp.rs`) changes its trigger from
`explore.json.exists()` to `config.mode == mcp-llm`. The `--explore` / `--standard`
flags remain as runtime overrides. This one change ends the stickiness: after
`init --as mcp` sets `mode: "mcp"`, `serve` boots Standard immediately.

### 2 — `init` reconciles all harness files on a mode switch

Extract a single function keyed off the declared mode:

```rust
fn reconcile_harness(root, old_mode: Option<Mode>, new_mode: Mode) -> Result<Vec<String>>
```

It writes, rewrites, or **strips** `.mcp.json`, `CLAUDE.md`, and `AGENTS.md` so the
on-disk harness matches `new_mode` exactly, cleaning up `old_mode`'s residue
(e.g. removing the `AGENTS.md` explore block when leaving `mcp-llm`, dropping the
`--explore` arg when leaving explore mode). `init` reads the prior `mode` from
`config.json`, calls `reconcile_harness`, then persists the new `mode`. This is
the one place that maps `mode → on-disk harness state`.

### 3 — the `config` TUI becomes display-consistent (read-only on mode)

The TUI does **not** gain a mode selector — changing mode stays the job of
`grove init --as`. It only gains read awareness so it never implies explore
settings drive a project that isn't in explore mode:

- Shows the active `mode` (read from `config.json`) as a header/badge.
- When `mode == mcp-llm`: the explore section is live and editable, as today.
- When `mode != mcp-llm`: the explore fields render inert/greyed with a one-line
  note that they are dormant until `grove init --as mcp-llm`.

Because the TUI never mutates `mode`, it cannot become a second, uncoordinated
harness writer — the reason mode-editing-in-TUI was rejected (see Alternatives).

### Migration

`GroveConfig::load`: read `config.json` if present; else if legacy
`.grove/explore.json` exists, load it, synthesize `mode: mcp-llm`, and rewrite
forward to `config.json`. Without this, every deployed `mcp-llm` project silently
drops to Standard on upgrade (its `explore.json` would no longer be the trigger).

## Scope boundary — what this deliberately does NOT do

- **No mode changes from the TUI.** `grove init --as` remains the only way to
  switch mode; the TUI is display-only on that axis. This keeps a single writer
  of harness files (`init`) and avoids a second divergence source.
- **The `explore.mode → steering` rename is optional.** The only remaining
  collision is a human reading the JSON (`config.mode` vs `config.explore.mode`);
  both are unambiguous by path. Worth doing for clarity, but it can ship
  separately from the mode work.
- **No new config surface beyond `mode`.** This ADR consolidates existing state;
  it does not add tunables.
- **`grammars` mode still touches no project-owned files.** Writing
  `.grove/config.json` is consistent with the `grammars` contract because `.grove/`
  is grove's own directory, not a host file like `CLAUDE.md` / `.mcp.json`.

## Alternatives considered

- **Keep mode emergent; special-case the cleanup in `init`.** Rejected: without a
  persisted mode, `init` cannot know the *prior* mode, so it can only guess at
  cleanup from file residue — brittle and exactly the ambiguity that produced the
  bugs.
- **Thin `config.json` holding only `{ mode }`, leave `explore.json` separate.**
  Smaller diff and no TUI churn, but keeps two files and defers "explore as a
  section." Rejected as a half-measure — the point is one source of truth.
- **Make `serve` authoritative on the `.mcp.json` `--explore` arg instead of a
  file.** Fixes the runtime half but leaves `init` with no persisted prior mode to
  reconcile, and `.mcp.json` is a host-owned file grove shouldn't treat as its own
  state store. Rejected.
- **Let the `config` TUI change mode + call `reconcile_harness`.** Rejected as
  unnecessary complication: it makes the TUI a second harness writer and forces
  mode-selector UI. Switching mode is a rare, deliberate act well served by
  `grove init --as`.

## Consequences

**Positive**

- Both original bugs are fixed structurally: the sticky explore surface (bug 1)
  and the orphaned `AGENTS.md` (bug 2) both stem from emergent mode, which is now
  declared.
- One function (`reconcile_harness`) owns the `mode → files` mapping, so a mode
  switch converges every file from a single code path.
- `serve`, the agent's steering, and `init`'s transition logic all read the same
  signal.
- `.grove/` gains a documented, versioned config — a natural extension point.

**Negative / costs**

- A real refactor spanning `config.rs` (new `GroveConfig` wrapper, `mode` enum,
  migration), `mcp.rs` (surface check), `init.rs` (extract `reconcile_harness`),
  and `config_tui/` (mode badge + inert-state rendering).
- Migration code must carry the legacy `explore.json` path indefinitely (or until
  a deprecation window closes).
- `reconcile_harness` must be careful to only strip grove-authored blocks
  (sentinel-delimited) and grove's own `.mcp.json` entry, never host content.

## Status of implementation

- Proposed — not yet implemented. Next: a plan/task decomposing the four
  touch-points, with a transition-matrix test asserting that every `A → B` mode
  switch leaves `.mcp.json`, `CLAUDE.md`, `AGENTS.md`, and the `serve` surface
  mutually consistent.
