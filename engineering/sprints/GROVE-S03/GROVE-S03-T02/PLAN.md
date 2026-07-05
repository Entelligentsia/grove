# PLAN — GROVE-S03-T02: Legacy `.grove/explore.json` Migration on Load

## Objective

Extend `GroveConfig::load` so that projects created before `.grove/config.json`
existed (all deployed `mcp-llm` projects from S02) silently upgrade on first
`load`. When `config.json` is absent but the legacy `.grove/explore.json` is
present, the loader synthesises a canonical `GroveConfig`, rewrites it forward to
`config.json` atomically, emits a one-time deprecation warning to stderr, and
returns the migrated config — exactly as if the project had been initialised with
`grove init --as mcp-llm` against the new schema.

---

## Background: What T01 Built (Dependencies)

T01 (`GROVE-S03-T01`) delivered:
- `Mode` enum with kebab-case on-disk spelling (`mcp-llm`).
- `GroveConfig { version: u32, mode: Mode, explore: Option<ExploreConfig> }` —
  the new canonical config record.
- `ExploreConfig.steering: Steering` — renamed from the S02 `mode` key.
- `GroveConfig::save(root)` — atomic write to `.grove/config.json`.
- `GroveConfig::load(root)` — currently bails with an actionable error when
  `config.json` is absent.

T02 extends `load` with the migration branch.

---

## Legacy Format

Before S03, `mcp-llm` projects stored their entire configuration in
`.grove/explore.json`. The wire shape:

```json
{
  "provider": "ollama",
  "base_url": "http://localhost:11434/v1",
  "model": "qwen2.5-coder:7b",
  "mode": "standard",
  "allowed_tools": ["grove", "rg", "grep", "find"],
  "tap": false,
  "trace_retain": 50
}
```

The key difference from the current schema is:
- The field `"mode"` in the legacy file encodes the **steering level** (not the
  integration mode). T01 renamed this to `"steering"` in `ExploreConfig`.
- There is no integration mode field — the presence of `explore.json` implied
  `mode: mcp-llm`.

The current `ExploreConfig` deserialization (with the `steering` field required)
intentionally rejects the old `mode` key — see the `legacy_mode_key_rejected`
test in `explore/config.rs`.

---

## Approach

All changes live in `core/src/config.rs`. No changes to
`core/src/explore/config.rs`.

### Step 1 — `LegacyExploreRaw` (private struct)

Add a private `LegacyExploreRaw` struct that matches the *old* `explore.json`
wire shape: `mode: String` (steering level), plus `provider`, `base_url`,
`model`, `allowed_tools`, `tap`, `trace_retain`. Derive `Deserialize` only.

```text
struct LegacyExploreRaw {
    provider: String,
    base_url: String,
    model: String,
    mode: String,                  // ← old steering key
    allowed_tools: Vec<String>,
    tap: bool,                     // serde default
    trace_retain: u32,             // serde default = default_trace_retain()
}
```

### Step 2 — `migrate_from_legacy_explore` (private free function)

```text
fn migrate_from_legacy_explore(root: &Path) -> Result<GroveConfig>
```

Logic:
1. Read `ExploreConfig::config_path(root)` (i.e. `.grove/explore.json`).
2. Deserialize into `LegacyExploreRaw`.
3. Map the old `mode` string to `Steering` using `Steering::from_name`, yielding
   a descriptive error if the value is not a legal steering level.
4. Map `provider` string to `Provider` using `Provider::from_name`.
5. Construct `ExploreConfig { steering, provider, base_url, model, allowed_tools,
   tap, trace_retain }`.
6. Construct `GroveConfig { version: 1, mode: Mode::McpLlm, explore: Some(explore_cfg) }`.
7. Call `config.validate()` (belt-and-suspenders).
8. Call `config.save(root)` — writes `config.json` atomically.
9. Emit the deprecation warning to **stderr**:

   ```
   warning: .grove/explore.json is deprecated and will be removed in a future
            version of grove. Your configuration has been automatically migrated
            to .grove/config.json. Please commit the new file and remove
            .grove/explore.json from your repository.
   ```

10. Return `Ok(config)`.

This function is the only place that touches the legacy file. It does not delete
`explore.json` — removal is left to the user (doctor will warn about its
presence after migration, per the sprint requirements).

### Step 3 — Extend `GroveConfig::load`

Replace the current "no file → bail" block with a three-branch cascade:

```text
pub fn load(root: &Path) -> Result<Self> {
    let path = Self::config_path(root);
    if path.exists() {
        // (existing path) read, deserialize, validate
        ...
    } else if ExploreConfig::config_path(root).exists() {
        // NEW: legacy migration branch
        migrate_from_legacy_explore(root)
    } else {
        // (existing path) actionable "run grove init" error
        bail!(...)
    }
}
```

The `ExploreConfig::config_path` call requires no new imports — `ExploreConfig`
is already in scope via `use crate::explore::ExploreConfig`.

---

## Files to Modify

| File | Change |
|---|---|
| `core/src/config.rs` | Add `LegacyExploreRaw` struct; add `migrate_from_legacy_explore` fn; extend `GroveConfig::load` with legacy branch; add four tests (see below). |

`core/src/explore/config.rs` — **no changes**. Its `config_path` method is
reused (read-only) by the new migration code.

---

## Data Model Changes

No changes to any persistent struct's wire format. The migration *reads* the
old shape (via `LegacyExploreRaw`) and *writes* the new shape (via the existing
`GroveConfig::save`). After migration the legacy file stays on disk but is never
read again by `load`.

---

## Testing Strategy

Four new `#[test]` functions added to `mod tests` in `core/src/config.rs`:

| Test | AC | What it asserts |
|---|---|---|
| `migrate_legacy_explore_writes_config_json` | 5a | Given a root with only `explore.json` (old `mode` key), `GroveConfig::load` succeeds; returned config has `mode == McpLlm` and `explore.steering` matching the old `mode` value; `.grove/config.json` exists on disk after the call. |
| `second_load_after_migration_reads_config_not_legacy` | 5b | After a first `load` (which migrates), a second `load` succeeds and returns an equal config; the legacy `explore.json` is not modified by the second load. |
| `config_json_present_ignores_stale_explore_json` | 5c | Given a root with both `config.json` (mcp mode) and a stale `explore.json`, `load` returns the `config.json` values; the stale file is silently ignored. |
| `deprecation_warning_emitted` | 5d | The migration path emits the deprecation warning string to stderr. Verified by replacing `stderr` with a pipe (use `std::process::Command` to exec a helper binary, or capture stderr via the `gag` / `capture` pattern). Alternatively: confirm the warning is present in the test by observing it as a side-channel during CI output (acceptable for a first pass if cross-platform stdio capture is not available). |

All existing tests must continue to pass. Run the full suite with:
```
cargo test --release --locked
```
and lint with:
```
cargo clippy -- -D warnings
```

### Test fixture (legacy explore.json)

All tests that exercise the migration path write this inline JSON to
`<temp_root>/.grove/explore.json`:

```json
{
  "provider": "ollama",
  "base_url": "http://localhost:11434/v1",
  "model": "qwen2.5-coder:7b",
  "mode": "balanced",
  "allowed_tools": ["grove"],
  "tap": false,
  "trace_retain": 50
}
```

`"mode": "balanced"` is chosen so the asserted `steering` value is distinct from
the default `Standard`, proving the mapping was applied.

---

## Acceptance Criteria Mapping

| # | Criterion | Covered by |
|---|---|---|
| 1 | `load` migrates when `config.json` absent + `explore.json` present | Step 3 + test 5a |
| 2 | Legacy `explore.mode` maps to `explore.steering` | `migrate_from_legacy_explore` + test 5a |
| 3 | Deprecation warning emitted to stderr, one-time only | Step 2 item 9 + test 5d, 5b |
| 4 | Neither file → same actionable error | Unchanged bail path + existing test `missing_file_actionable_error` |
| 5 | Four tests (a–d) pass | Tests above |
| 6 | `cargo build` clean, `clippy -D warnings` clean, `cargo test` green | CI commands |

---

## Operational Impact

- **Material change** — alters on-disk behaviour: creates `config.json` on first
  load for all deployed `mcp-llm` projects. A version bump is required at
  release.
- **No user action required** — migration is automatic on next `serve`/`load`.
- **Deprecation warning** informs users to commit `config.json` and drop
  `explore.json`.
- **Idempotent after first run** — subsequent loads read `config.json` directly,
  never re-entering the migration branch.
- **No deletion** of `explore.json` — the legacy file remains; `grove doctor`
  (a later task) will flag its presence as a warning.
- **Security scan:** not required.
