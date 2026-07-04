# GROVE-S03-T07: `grove doctor` — `core::doctor` report + CLI verb (checks, `--json`, exit code)

**Sprint:** GROVE-S03
**Estimate:** L
**Pipeline:** default

---

## Objective

Ship `grove doctor` — a read-only, pre-flight health check (`brew doctor` /
`flutter doctor` analogue) that answers "is my setup healthy?". Its headline
value is catching **declared-mode-vs-harness drift** and the silent
**provider-down** condition before they manifest as a wrong surface, and being
usable as a CI/setup gate via its exit code. Engine logic lives in a new typed
`core::doctor`; the CLI verb only dispatches and formats.

## Acceptance Criteria

1. `core::doctor` defines `Status { Ok, Warn, Fail, Info }`,
   `Check { group, name, status, detail, hint }`, `Report { mode: Mode, checks }`
   (`mode` is the core declared `Mode`, **not** the CLI `Surface`), and
   `diagnose(root, force) -> Report`. Pure / read-only — no writes.
2. Mode resolution uses the shared `active_mode(root, force)` (T03): no flag →
   auto-detect (`config.mode == mcp-llm` ⇒ explore; else standard);
   `--explore`/`--standard` force, same precedence as `serve`.
3. **Universal checks** run for every project: grove version;
   `config.json` present + loads + legal `mode`; **harness-consistency (headline)**
   — declared `mode` vs `.mcp.json` grove args, the steering blocks in
   `CLAUDE.md`/`AGENTS.md`, and the surface `serve` would boot; **fail on drift,
   naming the mismatched file**; legacy `explore.json` present without
   `config.json` → **warn** (pending migration); registry root + search order;
   grammar cache presence; detected project languages vs available grammars.
4. **`grove.lock` sha256 verify** via T06's `verify_lock`: present → per-language
   Match/Mismatch/Missing, **fail on mismatch**; absent → warn/info (not fail).
5. **Explore-mode checks** (resolved/forced explore): explore section validates;
   `steering` legal; provider **reachable** (`health_probe` →
   `HealthError::Unreachable`) and configured **model served**
   (`HealthError::ModelMissing`, enriched via `list_models`), surfacing the
   built-in fix hints **verbatim**; `allowed_tools` recognised (warn on unknown);
   tap / trace_retain reported as info.
6. `cli/src/main.rs` gains `Cmd::Doctor { path, explore, standard }` (+ the
   global `--json`): renders a grouped ✓/⚠/✗ human table by default or the
   machine-readable `{ path, mode, ok, checks[], summary }` under `--json`, and
   sets the process **exit code** from `Report::ok()` — **0** when all checks
   pass (warnings allowed), **non-zero** on any hard failure, under **both**
   output formats. Engine stays in `core`; the verb only formats.
7. Tests: unit-test `diagnose` using existing seams (`DownClient` /
   `health_probe_against_unreachable_url_is_unreachable`, `client.rs`); unit-test
   the **harness-consistency** check against the T04 transition matrix (shared
   fixtures) — `Ok` after a clean `init`, `Fail`/`Warn` on hand-induced drift
   (e.g. `config.mode=mcp` but `.mcp.json` carries `--explore`, or a stale
   `AGENTS.md` explore block); integration-test the verb against the dev stub
   (universal + standard, no network) and a dead `base_url` (explore failure).
8. `cargo build` warning-clean, `cargo clippy -- -D warnings` clean, `cargo
   test` green; `--help` documents `doctor`. Files end with a newline.

## Context

Implements item 6 of `SPRINT_REQUIREMENTS.md` and the whole
`docs/doctor-command-proposal.md`. Composes existing primitives:
`registry::search_path`/`cache_root`/`for_path` (`core/src/registry.rs`),
`GroveConfig::load`/`active_mode` (T01/T03), `health_probe`/`list_models`
(`core/src/explore/client.rs`), and T06's `verify_lock`. The harness-consistency
check compares the declared mode against the state **T04**'s `reconcile_harness`
is supposed to produce — reuse those fixtures. Distinct from the existing
`check` verb (file syntax, not config). Depends on **T01, T03, T04, T06** — the
capstone task.

## Artifacts Involved

- `core/src/doctor.rs` (new) — `Status`/`Check`/`Report`/`diagnose` +
  harness-consistency logic; export from `lib.rs`.
- `cli/src/main.rs` — `Cmd::Doctor` arm, human + `--json` formatting, exit code.
- Reuses `core/src/{config,registry,lock}.rs` and `core/src/explore/client.rs`.

## Operational Impact

- **Version bump:** required at release (new user-facing verb).
- **Regeneration:** none; additive command.
- **Security scan:** not required.
