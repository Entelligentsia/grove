# Proposal: `grove doctor` — config-mode health check

**Status:** proposed (not implemented)
**Date:** 2026-07-04
**Author:** (design draft)
**Depends on:** [ADR 0002](adr/0002-grove-project-config-and-declared-mode.md) —
`.grove/config.json` and the declared `mode`. `doctor` reads the mode from that
config (not from `.grove/explore.json`'s existence) and should be sequenced
**after** the config consolidation lands.

## Motivation

grove runs in more than one configuration, and the surfaces have very different
failure modes:

- **Standard structural mode** — the 7-tool MCP surface (`grove serve`). Health =
  the registry resolves and grammars exist for the project's languages.
- **Explore / mcp-llm mode** — `grove serve --explore`, backed by a local LLM
  (Ollama / llama.cpp) configured in `.grove/explore.json`. Health additionally
  requires the provider to be reachable and to actually serve the configured
  model.

Today the only health check lives **inside `serve` startup** and fails
**silently**: if the explore provider is down, `determine_surface`
(`cli/src/mcp.rs:44`) logs one line to stderr and quietly degrades to the
standard surface (`mcp.rs:65`). An agent harness swallows that stderr, so the
operator sees "it works" while getting a different surface than they configured.

A second, structural failure mode motivates `doctor` just as much: the
**declared mode can drift from the on-disk harness**. Under ADR 0002 the mode is
declared in `.grove/config.json`, but the harness files it implies
(`.mcp.json` args, the `CLAUDE.md` / `AGENTS.md` steering blocks) are written
separately by `init`; a partial edit, a hand-tweak, or a project mid-migration
from the legacy `.grove/explore.json` can leave them inconsistent. Nothing today
surfaces that mismatch.

There is **no deliberate, pre-flight way to ask "is my setup healthy?"** — and no
way to validate a provider *before* writing a config with `grove init --as
mcp-llm`. `grove doctor` fills that gap, in the spirit of `brew doctor` /
`flutter doctor`.

Naming note: distinct from the existing **`check`** verb, which checks *file
syntax* (tree-sitter ERROR/MISSING nodes), not configuration.

## Command surface

```
grove doctor [path] [--explore] [--standard]
```

- `path` — project directory (default `.`), used to locate `.grove/config.json`
  and to detect source languages. Mirrors `serve` / `config`.
- No mode flag → **auto-detect** the active mode exactly as `serve` does under
  ADR 0002 (`config.mode == mcp-llm` ⇒ explore; else standard). During the
  migration window (legacy `.grove/explore.json`, no `config.json`) the same
  fallback `serve` uses applies — treat it as `mcp-llm`.
- `--explore` / `--standard` → **force** checking that mode, even if no config
  exists. `--explore` lets an operator validate a provider *before* `init --as
  mcp-llm`. Same precedence semantics as `serve` (`--standard` wins).
- `--json` → machine-readable report (grove already has a global `--json` flag).

Exit code: **0** if all checks pass (warnings allowed), **non-zero** if any
check is a hard failure — so `grove doctor` is usable as a setup gate or in CI.

## Checks

Layered: universal checks always run; mode-specific checks run for the
resolved/forced surface.

### Universal (every project)

| Check | Reuses | Fail vs warn |
| --- | --- | --- |
| grove version / binary path | `env!("CARGO_PKG_VERSION")` | info |
| `.grove/config.json` present + loads + `mode` is legal | `GroveConfig::load` (ADR 0002) — fail-fast, field-named errors | fail; info if absent (unconfigured project) |
| **Declared mode vs on-disk harness consistent** — `config.mode` agrees with the `.mcp.json` grove args, the steering blocks present in `CLAUDE.md` / `AGENTS.md`, and the surface `serve` would boot | `GroveConfig::mode` + the sentinel/args conventions in `init.rs` (the `reconcile_harness` target state, ADR 0002) | **fail on drift**, naming the mismatched file — the headline check |
| Legacy `.grove/explore.json` present without `config.json` (pending migration) | filesystem check | warn — "run `grove init` to migrate to `.grove/config.json`" |
| Registry root resolution + full search order (which candidate won) | `registry::search_path()` → `Vec<RootCandidate{source,path,exists}>` (`registry.rs:141`) | fail if none exist |
| Grammar cache present + location | `registry::cache_root()` (`registry.rs:115`) | warn if absent |
| Project source languages detected vs grammars available | existing language detection used by `init` (`core/src/init.rs`) + registry index | warn per missing language |
| `grove.lock` integrity (wasm sha256 matches) — if a lockfile is present | lockfile read (`registry.rs`) + a **new** verify step: recompute each cached wasm's sha256 and compare to the lock's `wasm` field (not an existing primitive) | fail on mismatch |

### Standard mode

| Check | Reuses | Fail vs warn |
| --- | --- | --- |
| The 7-tool surface can resolve a grammar for each detected language | `registry::for_path` / index | warn per language with no grammar |

(Standard mode has no network or config dependency; its health is a subset of the
universal checks. Listed explicitly so `--standard` output is self-describing.)

### Explore / mcp-llm mode

The explore config lives in the `explore` section of `.grove/config.json`
(ADR 0002); its presence + validity is already covered by the universal
`GroveConfig::load` check above. These checks add the runtime/provider layer.

| Check | Reuses | Fail vs warn |
| --- | --- | --- |
| Explore section present + validates (provider/steering/base_url/model) | the `explore` section of `GroveConfig` — same fail-fast, field-named errors as `ExploreConfig::load` today | fail |
| `steering` is legal (standard / balanced / aggressive) | `Mode::LEGAL` (`config.rs:66`) — the steering level, renamed from `mode` per ADR 0002 | fail (load already enforces) |
| Provider **reachable** | `health_probe` → `HealthError::Unreachable{url,detail}` (variant `client.rs:480`, fn `client.rs:627`) | fail, with the built-in hint |
| Configured **model served** | `health_probe` → `HealthError::ModelMissing{model,url,available}` (`client.rs:487`); enrich with `list_models` (`client.rs:672`) to show what *is* available | fail, with the built-in hint |
| `allowed_tools` are recognized tool names | `toolset` tool-name constants | warn on unknown entries |
| tap / trace_retain settings | `explore` section fields (`config.rs:101,105`) | info |

The two `HealthError` variants already carry operator-facing fix hints
(`client.rs:500-511`, e.g. "is the server running? check `base_url` in
.grove/config.json") — `doctor` should surface those verbatim.

## Output

**Human (default):** a grouped ✓ / ⚠ / ✗ checklist.

```
grove doctor · /path/to/project · mode: mcp-llm (explore)

  Universal
    ✓ grove 0.3.0
    ✓ config: .grove/config.json · mode=mcp-llm
    ✓ harness consistent: .mcp.json args, CLAUDE.md + AGENTS.md steering match mode
    ✓ registry: user cache (~/.cache/grove/grammars)   [4 more candidates searched]
    ✓ grammar cache: 27 languages
    ⚠ grove.lock: not present (run `grove lock` to pin grammars)
    ✓ project languages: rust ✓

  Explore mode (.grove/config.json → explore)
    ✓ config valid · provider=ollama · model=llama3 · steering=balanced
    ✗ provider unreachable at http://localhost:11434/v1/models: connection refused
        → is the server running? check `base_url` in .grove/config.json

  1 failure, 1 warning — explore mode is DOWN (serve would fall back to standard)
```

A drift example — the class of bug ADR 0002 targets, caught pre-flight:

```
grove doctor · /path/to/project · mode: mcp

  Universal
    ✓ config: .grove/config.json · mode=mcp
    ✗ harness drift: config.mode=mcp but .mcp.json registers `serve --explore`
        → run `grove init --as mcp` to reconcile the harness to the declared mode
    ⚠ stale AGENTS.md explore steering block present (not written by mode=mcp)
```

**JSON (`--json`):**

```json
{
  "path": "/path/to/project",
  "mode": "mcp-llm",
  "ok": false,
  "checks": [
    {"group": "universal", "name": "harness_consistent", "status": "ok",
     "detail": "config.mode=mcp-llm matches .mcp.json + steering", "hint": null},
    {"group": "universal", "name": "registry", "status": "ok",
     "detail": "user cache", "hint": null},
    {"group": "explore", "name": "provider_reachable", "status": "fail",
     "detail": "connection refused at http://localhost:11434/v1/models",
     "hint": "is the server running? check base_url in .grove/config.json"}
  ],
  "summary": {"ok": 5, "warn": 1, "fail": 1}
}
```

## Implementation altitude

Keep engine logic out of the faces (the repo's "main/mcp only format" rule):

- New **`core::doctor`** module returning a typed report:
  ```rust
  pub enum Status { Ok, Warn, Fail, Info }
  pub struct Check { pub group: &'static str, pub name: &'static str,
                     pub status: Status, pub detail: String, pub hint: Option<String> }
  pub struct Report { pub mode: Mode, pub checks: Vec<Check> }  // Mode = GroveConfig's declared mode (ADR 0002)
  pub fn diagnose(root: &Path, force: ModeChoice) -> Report;
  ```
  The report's `mode` is the core `Mode` enum from `GroveConfig` (ADR 0002) — a
  plain declared-mode tag, **not** the serve-time `cli::mcp::Surface` (which
  carries a live `ExploreConfig` + root and is a CLI-side runtime type; `core`
  must not depend on it). `diagnose` composes existing primitives
  (`registry::search_path`, `cache_root`, lockfile verify, `GroveConfig::load`,
  `health_probe`, `list_models`) plus the new harness-consistency check. Pure /
  read-only — no writes.
- **`cli/src/main.rs`** gains a `Cmd::Doctor { path, explore, standard }` arm that
  calls `core::doctor::diagnose` and formats the report (human table or `--json`),
  then sets the process exit code from `Report::ok()`. `--json` still sets the
  non-zero exit on hard failure (it changes the *format*, not the gate), so CI can
  branch on the exit code without parsing the payload.
- **Single source of truth for the active mode.** ADR 0002 already makes
  `GroveConfig::mode` that source (replacing `serve`'s `.grove/explore.json`
  existence check). `doctor` reads the same `GroveConfig`; if ADR 0002 extracts a
  shared `active_mode(root, force) -> Mode` helper, `serve` and `doctor` call it
  identically. The harness-consistency check compares that declared mode against
  the on-disk files (`.mcp.json` args + sentinel steering blocks) — i.e. it
  verifies the state `init`'s `reconcile_harness` is supposed to have produced.

MVP is a CLI-only verb. Because the report is a typed value, it can later be
surfaced as an MCP tool or reused by `config` (validate-on-save) and `serve`
(replace the silent-fallback stderr line with a structured warning) without
rework.

## Testing

- Unit-test `core::doctor::diagnose` with seams that already exist: the explore
  suite has a `DownClient` and `health_probe_against_unreachable_url_is_unreachable`
  (`client.rs:910`); the CLI suite already covers the silent-fallback path
  (`cli/tests/cli.rs:422`, `explore_mode_unhealthy_provider_falls_back_to_standard_surface`).
- Unit-test the **harness-consistency** check directly against ADR 0002's
  transition matrix: for each `A → B` mode switch, assert `diagnose` reports
  `Ok` after a clean `init` and `Fail`/`Warn` on a hand-induced drift (e.g.
  `config.mode=mcp` but `.mcp.json` still carries `--explore`, or a stale
  `AGENTS.md` explore block). This is the same matrix ADR 0002 uses to validate
  `reconcile_harness`, so the two share fixtures.
- Integration-test the verb against the dev stub for the universal + standard
  checks (no network), and against a dead `base_url` for the explore failure path
  (same pattern the fallback test uses).

## Out of scope (for the MVP)

- Auto-remediation (pulling a missing model, running `grove fetch`). `doctor`
  reports and hints; it does not mutate.
- Windows-specific cache-path checks beyond what `dirs::cache_dir()` already
  abstracts.
- Probing multiple explore configs at once — `doctor` inspects one project's
  resolved (or forced) mode.
