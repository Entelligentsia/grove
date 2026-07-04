# PLAN — GROVE-S03-T07: `grove doctor` — `core::doctor` report + CLI verb

## Objective

Implement `grove doctor`: a `core::doctor` module with `Status`, `Check`, `Report`,
and `diagnose(root, force)` that runs universal + mode-scoped health checks; a
new `Cmd::Doctor` CLI verb with `--json` output and exit-code semantics; and a
`core::harness` module that holds the sentinel constants and mode→expected-content
tables so both `init` and `doctor` share a single source of truth.

---

## Approach

### 0. Pre-requisite: Plan review resolution

The review identified two blocking issues; this revised plan resolves both before
any code is written.

**Blocker 1 — Harness constant ownership**: `CLAUDE_START`, `CLAUDE_END`, and
`MCP_SERVER_KEY` are private constants in `cli/src/init.rs`. Core cannot import
them (cli→core dependency runs that direction; adding a core→cli edge would
create a cycle). The per-mode expected-content markers (`mcp__grove__explore`,
`mcp__grove__outline`, `grove skill`) and `.mcp.json` args are embedded in
private `claude_section` / `write_mcp_json_with` functions in `cli`. Therefore
`core::doctor` cannot reference them without duplication, which defeats the
purpose of the check.

**Fix:** introduce `core/src/harness.rs` as the single source of truth for all
harness-shape knowledge: sentinel constants, expected `.mcp.json` args per mode,
and expected content-markers per mode. Both `cli/src/init.rs` (for writing) and
`core/src/doctor.rs` (for reading/checking) import from this module.

**Blocker 2 — Unit test strategy**: The original plan's harness-consistency
tests referenced `reconcile_harness` and `seed_lock` — private helpers in
`cli/src/init.rs`. Inline tests in `core/src/doctor.rs` cannot call cross-crate
private items.

**Fix:** unit tests in `core/src/doctor.rs` build every fixture by hand into a
`tempdir`: write `.grove/config.json`, `.mcp.json`, `CLAUDE.md`, `AGENTS.md`
directly using `std::fs::write`, then call `diagnose`. No private cli imports.

---

### 1. New `core/src/harness.rs` — single source of truth for harness shape

This module defines the constants and query functions that `init` uses to write
the harness and `doctor` uses to verify it. It has **no external dependencies**
other than `core::config::Mode`.

```rust
use crate::config::Mode;

// ── Sentinel markers (single source of truth) ──────────────────────────────

pub const GROVE_START: &str = "<!-- grove:start -->";
pub const GROVE_END:   &str = "<!-- grove:end -->";

// ── MCP server registry key ─────────────────────────────────────────────────

pub const MCP_SERVER_KEY: &str = "grove";

// ── Expected .mcp.json args per mode ───────────────────────────────────────
//
// Returns `Some(&["serve"])` / `Some(&["serve", "--explore"])` when a grove
// entry is expected, `None` when the mode must have NO grove entry.

pub fn expected_mcp_args(mode: Mode) -> Option<&'static [&'static str]> {
    match mode {
        Mode::Mcp | Mode::Both => Some(&["serve"]),
        Mode::McpLlm           => Some(&["serve", "--explore"]),
        Mode::Skill            => None,
        Mode::Grammars         => None,
    }
}

// ── Expected CLAUDE.md content marker per mode ─────────────────────────────
//
// Returns `None` for `Grammars` (file may exist but must have no grove block).
// Returns `Some(marker)` — a substring that MUST appear inside the grove block.

pub fn expected_claude_marker(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::McpLlm           => Some("mcp__grove__explore"),
        Mode::Mcp | Mode::Both => Some("mcp__grove__outline"),
        Mode::Skill            => Some("grove skill"),
        Mode::Grammars         => None,          // block must be absent
    }
}

// ── AGENTS.md expectation per mode ─────────────────────────────────────────
//
// Returns `true` when the mode requires a grove block in AGENTS.md.

pub fn agents_md_expected(mode: Mode) -> bool {
    mode == Mode::McpLlm
}
```

**Migration in `cli/src/init.rs`**: replace the three `const` definitions
(`CLAUDE_START`, `CLAUDE_END`, `MCP_SERVER_KEY`) with `use grove_core::harness`
imports. All downstream call-sites that reference the old constants now reference
`harness::GROVE_START` / `harness::GROVE_END` / `harness::MCP_SERVER_KEY`.
The existing `assert_mcp_json_consistent`, `assert_claude_md_consistent`, and
`assert_agents_md_consistent` test helpers in `cli/src/init.rs` can additionally
adopt `harness::expected_mcp_args` / `harness::expected_claude_marker` /
`harness::agents_md_expected` so they share the same truth source as doctor.

---

### 2. New `core/src/doctor.rs` — types and `diagnose`

**Core types:**

```rust
pub enum Status { Ok, Warn, Fail, Info }

pub struct Check {
    pub group:  &'static str,   // "universal" | "explore"
    pub name:   &'static str,
    pub status: Status,
    pub detail: String,
    pub hint:   Option<String>,
}

pub struct Report {
    pub mode:   Mode,            // declared mode from GroveConfig; not the CLI Surface
    pub checks: Vec<Check>,
}

impl Report {
    /// True when no check has `Status::Fail`.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| !matches!(c.status, Status::Fail))
    }
}

pub fn diagnose(root: &Path, force: ModeChoice) -> Report { … }
```

`diagnose` is **pure and read-only**. It composes only existing `core` primitives;
it never writes to disk.

**Mode resolution**: call `active_mode(root, force)` (T03 shared helper). The
report's `mode` field is `GroveConfig::mode` (the declared tag). If no config
exists, mode falls back to `Mode::Mcp` and that is recorded via an Info check.

---

### 3. Universal checks (run for every project)

Appended in order to `report.checks` with `group: "universal"`.

| Check name | Logic | Status |
|---|---|---|
| `grove_version` | `env!("CARGO_PKG_VERSION")` | Info |
| `config_present` | `GroveConfig::load(root)` | Ok / Fail; Info if absent (unconfigured) |
| `legacy_explore_json` | `.grove/explore.json` present without `.grove/config.json` | Warn |
| `harness_mcp_json` | compare `.mcp.json` grove args vs `harness::expected_mcp_args(mode)` | Ok / Fail |
| `harness_claude_md` | detect sentinel; check for `harness::expected_claude_marker(mode)` | Ok / Fail / Warn |
| `harness_agents_md` | detect sentinel; check vs `harness::agents_md_expected(mode)` | Ok / Warn |
| `harness_serve_surface` | simulate `determine_surface` for the declared mode | Info |
| `registry_root` | `registry::search_path()` — fail if no candidate `exists` | Ok / Fail |
| `grammar_cache` | `registry::cache_root()` | Ok / Warn if None |
| `project_languages` | walk source extensions via `ignore` against `registry::available()` | Ok / Warn per missing lang |
| `lock_integrity` | `registry::verify_lock(&root.join("grove.lock"))` | Match→Ok, Mismatch→Fail, Missing→Warn; absent→Warn |

#### 3a. Harness check detail

Each of the four harness sub-checks (`harness_mcp_json`, `harness_claude_md`,
`harness_agents_md`, `harness_serve_surface`) is emitted as a separate `Check`
entry so each file's drift is independently addressable in `--json` output.

**`.mcp.json` consistency** — parse file if present; extract
`doc["mcpServers"]["grove"]["args"]` as a `Vec<String>`. Compare to
`harness::expected_mcp_args(mode)`:
- Mode expects a grove entry (`Some(args)`) but args mismatch or file absent → `Fail`;
  detail names file + expected vs actual; hint: `grove init --as <mode>`.
- Mode expects no grove entry (`None`) but a grove entry exists → `Fail`.
- Correct → `Ok`.

**`CLAUDE.md` consistency** — read file if present:
- `harness::expected_claude_marker(mode) == None` (Grammars): block must be
  absent; if `GROVE_START` found → `Warn`.
- `harness::expected_claude_marker(mode) == Some(marker)`: file must exist, must
  contain `GROVE_START`, and must contain `marker` inside the block; any deviation
  → `Fail`.

**`AGENTS.md` consistency** — read file if present:
- `harness::agents_md_expected(mode)` is `true`: file must exist, must contain
  `GROVE_START`; absent → `Warn`.
- `harness::agents_md_expected(mode)` is `false`: if file exists and contains
  `GROVE_START` → `Warn`.

**`harness_serve_surface`** — compute what `Cmd::Serve` would pick (`Standard` or
`Explore`) given the declared mode and whether an `explore` config section is
present. Emitted as `Info`.

All four use `harness::GROVE_START` / `harness::GROVE_END` / `harness::MCP_SERVER_KEY`
from `core/src/harness.rs` — no constants duplicated, no cli cross-dependency.

---

### 4. Explore-mode checks (mode == McpLlm only)

Run after universal checks; appended with `group: "explore"`.

| Check name | Logic | Status |
|---|---|---|
| `explore_config_valid` | `explore` section must be present and pass `ExploreConfig::validate()` | Fail if absent/invalid |
| `provider_reachable` | `health_probe(&cfg)` → `HealthError::Unreachable { url, detail }` | Fail; surface hint verbatim |
| `model_served` | `health_probe` → `HealthError::ModelMissing { model, url, available }` | Fail; enrich with `list_models` |
| `allowed_tools_known` | compare `cfg.allowed_tools` vs `toolset::{READ, GLOB, GREP, GROVE}` | Warn per unrecognized |
| `tap_config` | `cfg.tap` / `cfg.trace_retain` values | Info |

---

### 5. CLI dispatch in `cli/src/main.rs`

Add to `Cmd`:

```rust
/// Run a pre-flight health check (brew doctor / flutter doctor analogue).
Doctor {
    /// Project directory (default: current).
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Force explore-mode checks even if config declares standard mode.
    #[arg(long = "explore")]
    explore: bool,
    /// Force standard mode checks.
    #[arg(long = "standard")]
    standard: bool,
},
```

**Human output** — grouped ✓ / ⚠ / ✗ table per `group`:

```
grove doctor · <path> · mode: <mode>

  Universal
    ✓ grove_version · <version>
    ✓ config_present · .grove/config.json · mode=<mode>
    ✗ harness_mcp_json · config.mode=mcp but .mcp.json registers `serve --explore`
        → run `grove init --as mcp` to reconcile the harness to the declared mode
    …

  1 failure, 1 warning
```

**JSON output** (`--json`):

```json
{
  "path": "<absolute path>",
  "mode": "<mode string>",
  "ok": <bool>,
  "checks": [
    { "group": "…", "name": "…", "status": "ok|warn|fail|info",
      "detail": "…", "hint": null }
  ],
  "summary": { "ok": N, "warn": N, "fail": N, "info": N }
}
```

**Exit code** — `std::process::exit(1)` when `!report.ok()`, under **both**
output formats. `Cmd::Doctor` dispatch arm returns `Ok(())` before the exit call
so `?` propagation works normally.

---

## Files to Modify

| File | Change |
|---|---|
| `core/src/harness.rs` | **New** — `GROVE_START`, `GROVE_END`, `MCP_SERVER_KEY`, `expected_mcp_args`, `expected_claude_marker`, `agents_md_expected` |
| `core/src/lib.rs` | Add `pub mod harness;` and `pub mod doctor;` |
| `core/src/doctor.rs` | **New** — `Status`, `Check`, `Report`, `diagnose` |
| `cli/src/init.rs` | Replace local `CLAUDE_START`/`CLAUDE_END`/`MCP_SERVER_KEY` consts with imports from `grove_core::harness`; update all references |
| `cli/src/main.rs` | Add `Cmd::Doctor` variant; dispatch arm; import `grove_core::doctor` |

Reuses (read-only, no changes required):

- `core/src/config.rs` — `GroveConfig::load`, `active_mode`, `ModeChoice`, `Mode`
- `core/src/registry.rs` — `search_path`, `cache_root`, `verify_lock`, `available`
- `core/src/explore/client.rs` — `health_probe`, `list_models`, `HealthError`
- `core/src/explore/config.rs` — `ExploreConfig::validate`
- `core/src/explore/toolset.rs` — `READ`, `GLOB`, `GREP`, `GROVE` constants

---

## Data Model Changes

None. No `.forge/store/`, `config.json`, lock-file, or wire-format changes.
`core::harness` is an additive module with no serialised state.

---

## Testing Strategy

### Unit tests in `core/src/harness.rs`

- `expected_mcp_args_coverage` — assert every `Mode` variant returns a value;
  verify Mcp/Both → `["serve"]`, McpLlm → `["serve","--explore"]`,
  Skill/Grammars → `None`.
- `expected_claude_marker_coverage` — assert all variants; McpLlm has
  `mcp__grove__explore`, Mcp/Both have `mcp__grove__outline`, Skill has
  `grove skill`, Grammars is `None`.
- `agents_md_expected_only_mcp_llm` — `true` exactly for `Mode::McpLlm`.

### Unit tests in `core/src/doctor.rs`

All tests build fixtures **by hand** in a `tempdir` (`std::fs::write`) and call
`diagnose`. No cross-crate private helpers.

**Harness-consistency matrix** (replaces the private `reconcile_harness` fixtures):

For each mode (Mcp, Skill, Both, McpLlm, Grammars), write a minimal but
complete fixture set (config.json + correct harness files), call `diagnose`, and
assert all four harness checks are `Status::Ok`.

Then for selected drift scenarios:
- `config.mode=mcp` + `.mcp.json` has `["serve","--explore"]` → `harness_mcp_json` is `Fail`.
- `config.mode=mcp` + `CLAUDE.md` contains `mcp__grove__explore` → `harness_claude_md` is `Fail`.
- `config.mode=mcp-llm` + `AGENTS.md` absent → `harness_agents_md` is `Warn`.
- `config.mode=grammars` + `CLAUDE.md` has `GROVE_START` → `harness_claude_md` is `Warn`.

**Lock integrity** (using the `DownClient` stub pattern from T06):

- `lock_integrity_absent_lockfile_is_warn` — no `grove.lock` present; assert
  `lock_integrity` check is `Warn`.
- `lock_integrity_match_is_ok` — write a minimal valid `grove.lock` matching a
  seeded grammar wasm; assert `lock_integrity` is `Ok` for that grammar.

**Explore-mode checks** (no network):

- `explore_config_absent_is_fail` — config declares McpLlm, explore section absent;
  `explore_config_valid` → `Fail`.
- `provider_unreachable_is_fail` — config has a dead `base_url`; `provider_reachable`
  → `Fail`.

**Exit-code boundary** (advisory from review):

- `warn_only_report_exits_zero` — unit-test `Report::ok()` with a fixture that
  has only `Warn` checks; assert `ok()` is `true` (warnings are pass-grade).

### Integration tests in `cli/tests/cli.rs`

- `doctor_help_documents_verb` — `grove doctor --help` exits 0 and prints
  "doctor".
- `doctor_universal_clean_exits_zero` — run against a provisioned dev root (no
  network); assert exit 0.
- `doctor_json_output_is_valid` — `grove doctor --json`; parse the output, assert
  `ok` field present and `checks` is an array.
- `doctor_fail_exits_nonzero` — induce a harness drift (e.g. write a wrong
  `.mcp.json`); assert exit code 1 under both human and `--json` modes.

---

## Acceptance Criteria

1. `core::harness` defines `GROVE_START`, `GROVE_END`, `MCP_SERVER_KEY`,
   `expected_mcp_args(Mode)`, `expected_claude_marker(Mode)`, `agents_md_expected(Mode)`.
   `cli/src/init.rs` imports and uses them — the sentinel strings exist in exactly
   one place in the codebase.

2. `core::doctor` defines `Status { Ok, Warn, Fail, Info }`,
   `Check { group, name, status, detail, hint }`, `Report { mode: Mode, checks }`,
   and `diagnose(root: &Path, force: ModeChoice) -> Report`. Pure / read-only.

3. Mode resolution via `active_mode(root, force)` (T03): no flag → auto-detect;
   `--explore`/`--standard` force.

4. **Universal checks** run for every project: `grove_version` (Info);
   `config_present` (Ok/Fail/Info); `legacy_explore_json` (Warn if drift);
   four harness sub-checks using `core::harness` constants; `registry_root`;
   `grammar_cache`; `project_languages`; `lock_integrity`.

5. **Harness checks** compare on-disk state to `harness::expected_*` → Fail on
   drift naming the mismatched file; hint instructs `grove init --as <mode>`.

6. **`grove.lock` sha256 verify** via T06 `verify_lock`: present → per-grammar
   Match→Ok / Mismatch→Fail / Missing→Warn; absent → Warn.

7. **Explore-mode checks** (McpLlm only): `explore_config_valid`,
   `provider_reachable`, `model_served`, `allowed_tools_known`, `tap_config`.

8. `cli/src/main.rs` gains `Cmd::Doctor { path, explore, standard }` + global
   `--json`; renders grouped ✓/⚠/✗ table or machine-readable JSON; sets exit
   code from `Report::ok()` — 0 when ok (warnings allowed), 1 on any Fail.

9. `Report::ok()` is `true` when only `Warn`/`Info` checks are present (no Fail).
   A warn-only run exits 0 under both output formats.

10. `cargo build` warning-clean; `cargo clippy -- -D warnings` clean;
    `cargo test` green; `--help` documents `doctor`.

---

## Operational Impact

- **Version bump:** required at release (new user-facing verb).
- **Regeneration:** none; additive command.
- **Security scan:** not required.
- **Change character:** additive — no existing behaviour modified.
- **`cli/src/init.rs` refactor:** moving constants to `core::harness` is a
  non-breaking internal refactor; all public APIs and on-disk file formats are
  unchanged.
