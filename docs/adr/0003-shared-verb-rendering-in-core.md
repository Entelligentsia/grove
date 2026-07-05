# ADR 0003 — Shared structural-verb rendering in `core`

- **Status:** Accepted
- **Date:** 2026-07-05
- **Deciders:** Boni Gopalan
- **Supersedes:** —
- **Related:** `core/src/explore/toolset.rs` (the explore inner toolset),
  `cli/src/main.rs` (verb dispatch + formatting), the 2026-07-03 code-quality
  review (item 2), [release 0.3.0 plan](../release-0.3.0-plan.md) Phase 0.

## Context

The explore inner toolset (`mcp-llm` mode) exposes a `Grove` tool that runs
read-only structural verbs (`outline`, `symbols`, `source`, `callers`,
`definition`, `map`) on behalf of the local LLM. Today it **shells out to grove's
own binary** per call — `grove_tool` builds a CLI arg vector and
`run_capture(grove_binary(), parts, root)` spawns `grove <verb> …` and captures
stdout (`toolset.rs:405-411`, `grove_binary()` resolves `current_exe()`).

The code-quality review flagged this: it loses the in-process grammar cache and
pays a subprocess spawn + full reparse on every tool call — *"acceptable while
experimental, but must be revisited before mcp-llm stabilizes."* The 0.3.0 release
graduates `mcp-llm` out of experimental, so this must land first.

The obstacle is an altitude mismatch. The toolset lives in `core`, but the human
**text** it needs to reproduce is produced by the CLI's formatting, which lives
inline in `cli/src/main.rs`'s `match` arms (`println!` blocks). `core` cannot
depend on `cli`, so "just call the formatter" isn't available — hence the
subprocess. Two facts make an in-process port clean:

1. The toolset captures **stdout only** (`run_capture` returns stdout on success),
   so only the `println!` bodies must be reproduced — not the `eprintln!` summary
   lines, which the toolset never saw.
2. `core` is already **clap-free** by design (see `core/src/init.rs`), and the six
   verbs take a small, stable, documented flag set — so parsing them without clap
   is tractable.

## Decision

Move the human text rendering of the six read-only structural verbs from
`cli/src/main.rs` into a new **`core::render`** module, and have the explore
toolset call `ops` + `core::render` **in-process** instead of shelling out.

### `core::render`

Pure typed→`String` formatters, one per verb, each returning exactly the bytes the
CLI's `println!` block emits today (trailing newline per line):

```rust
pub fn outline(syms: &[Symbol]) -> String;
pub fn symbols(syms: &[Symbol]) -> String;
pub fn source(res: &SourceResult) -> String;
pub fn callers(sites: &[CallSite]) -> String;
pub fn definition(defs: &[Symbol]) -> String;
pub fn map(maps: &[FileMap]) -> String;
```

- **`cli/src/main.rs`** replaces its inline `println!` loops with
  `print!("{}", render::outline(&syms))` etc. It keeps clap parsing, the `--json`
  branch, and its `eprintln!` summaries — those stay CLI concerns. No output
  change.
- **`core::explore::toolset`** parses the verb + documented flags from the
  `command` string (a small hand-rolled parser — `core` stays clap-free), calls
  the matching `ops` function, and renders with `core::render`. The per-call
  subprocess (`grove_binary` + `run_capture` for the Grove verb) is removed.
  `run_capture` remains for the `rg`/`find`-backed Grep/Glob tools.

The verb allow-list (`ALLOWED_VERBS` / `RECON_VERBS`) and the workspace-relative
path sandbox are unchanged — they gate the in-process path exactly as before.

## Scope boundary — what this deliberately does NOT do

- **`core` does not gain clap.** The toolset hand-parses the six verbs' small flag
  set; the `Cli`/`Cmd` clap structs stay in `cli`. This preserves the clap-free
  core.
- **The CLI's `--json` output and stderr summaries stay in `cli`.** `core::render`
  is human-text only; JSON is `serde` on the typed `ops` results, already shared.
- **No behaviour or format change.** `render` reproduces the current CLI stdout
  byte-for-byte (a parity test enforces this). This is a refactor, not a redesign.
- **Only the six read-only verbs move.** `check`, `init`, `doctor`, registry
  verbs, etc. keep their CLI-side formatting; they are not part of the explore
  toolset.

## Alternatives considered

- **Keep the subprocess.** Rejected: the review's stabilization blocker; a spawn +
  reparse per tool call is the wrong cost for a graduated surface.
- **Move clap `Cmd` dispatch into `core` and have both faces share it.** Rejected:
  pulls clap into `core`, breaking the clap-free-core principle for a larger
  surface than the six verbs need.
- **Capture the CLI's stdout in-process via a buffer without extracting the
  formatters.** Rejected: still couples `core` to `cli`'s dispatch, and there is no
  clean in-process handle to it from `core`.

## Consequences

**Positive**
- Explore tool calls use the in-process grammar cache — no spawn, no reparse per
  call. Removes the review's one stabilization blocker for `mcp-llm`.
- The verb text format now has a single owner (`core::render`) shared by both
  faces, so CLI and explore output cannot drift.
- Keeps the "faces only format" rule honest: the *shared* formatting moves down to
  `core`; `cli` retains only CLI-specific concerns (clap, `--json`, stderr).

**Negative / costs**
- A hand-rolled arg parser in the toolset must track the six verbs' documented
  flags; a new CLI flag the toolset should honour must be added in two places. Low
  risk (the set is small and stable) and covered by tests.
- A small altitude shift: `core` now owns human text rendering for these verbs,
  which previously lived entirely in `cli`.

## Status of implementation

- Accepted; implemented in the 0.3.0 prep branch (Phase 0): `core::render` +
  in-process toolset dispatch, with a parity test asserting `render` output equals
  the prior CLI stdout for representative calls.
