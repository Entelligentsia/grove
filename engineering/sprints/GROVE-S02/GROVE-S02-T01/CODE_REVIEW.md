# CODE_REVIEW — GROVE-S02-T01: Explore config model + persistence (`core::explore::config`)

Code review of the ExploreConfig implementation (standalone review, iteration 1).

**Verdict:** Approved

## What was reviewed

Read the actual diff and new files (not just PROGRESS.md):
- `core/src/lib.rs` — `pub mod explore;` + curated re-export of `ExploreConfig`, `Mode`, `Provider` with doc comment, matching the existing re-export style.
- `core/src/explore/mod.rs` — module root, re-exports config surface, doc comment scoping later S02 tasks.
- `core/src/explore/config.rs` — full read of enums, struct, `Default`, `load`/`save`/`validate`, custom `Deserialize`, and all 8 inline tests.

## Independent gate verification (all re-run by reviewer)

| Gate | Result |
| --- | --- |
| `cargo test --release --locked -p grove-cst explore` | 8/8 pass |
| `cargo test --release --locked --workspace` | 146 pass (95+32+18+1), 0 fail |
| `cargo clippy --release --locked --workspace` | clean, no warnings |
| `cargo tree -p grove-cst` | no clap |
| `grep -ri fastcontext core/` | absent |

## Plan compliance

- All three planned file changes executed exactly as planned; purely additive — only `lib.rs` touched among existing files. ✓
- All seven acceptance criteria met: module exists, load/save/validate against `.grove/explore.json` (sibling to `.grove/grammars/`), fail-fast steering to `grove init --as mcp-llm`/`grove config`, Ollama defaults with the `[grove, rg, grep, find]` allowlist, atomic temp+rename save, tests, dependency/string gates. ✓
- All three plan-review advisories addressed: (1) field-named enum errors via `RawExploreConfig` + manual `Deserialize` — verified by `bad_enum_names_field_and_legal_values`, which asserts both the field name and the legal values; (2) model pinned to `qwen2.5-coder:7b` and asserted in `defaults_are_ollama`; (3) Windows rename caveat — see advisory below. ✓

## Quality notes

- The `RawExploreConfig` design is the right call: serde's stock unknown-variant error names neither field nor struct; the manual bridge keeps `Serialize` derived (lowercase via `rename_all`) while owning the deserialization error text. `Provider::LEGAL`/`Mode::LEGAL` keep error text and parser in one place.
- Atomic save validates before writing, creates `.grove/` if absent, PID-suffixed temp file, rename into place — genuine hardening over the registry's plain `fs::write`. Test asserts no leaked `.tmp.` files.
- Tests are meaningful: fixed-fixture deserialization guards wire-format stability (not just round-trip identity), lowercase spellings asserted explicitly, both empty-field validation branches covered, missing-file steering asserted on the full anyhow chain.

## Advisories (non-blocking)

1. **Windows rename-over-existing:** `fs::rename` onto an existing `explore.json` can fail on Windows. Noted in the plan review; grove's current CI/targets are Unix-first, and the failure mode is a clean error (not a torn file), so acceptable. Revisit if Windows support lands.
2. **Temp-file collision:** the temp name is keyed on PID only; two threads in the same process saving to the same root could collide. Not a realistic hazard for this config's usage pattern — flagging for awareness only.
3. **Crate-name nit:** PLAN.md says `cargo tree -p grove-core`, but the package is named `grove-cst` (lib `grove_core`). PROGRESS.md already uses the correct name; no action needed.
