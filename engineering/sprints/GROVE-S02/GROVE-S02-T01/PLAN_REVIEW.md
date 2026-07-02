# PLAN REVIEW — GROVE-S02-T01: Explore config model + persistence (standalone review)

**Verdict:** Approved

## Independent Verification

All plan claims were checked against the actual repository, not taken from the plan:

- `core/Cargo.toml` — serde (with derive), serde_json, anyhow are already deps; **no clap** present (AC #6 baseline holds).
- `grep -ri fastcontext core/` — no occurrences today; purely a preservation gate.
- `core/src/lib.rs` — module list (`engine, init, ops, registry, fetch, ingest`) and curated re-export block exist exactly as the plan describes; adding `pub mod explore;` + optional re-exports is consistent with the established pattern.
- `core/src/registry.rs:399,426` — registry writes are plain `std::fs::write`, confirming the plan's claim that the temp-file-then-rename save is a deliberate hardening, not an existing pattern being copied.
- `core/src/registry.rs:152-154` — `.grove/grammars` project-local sibling convention exists; `root.join(".grove").join("explore.json")` follows it correctly.
- Test convention (unique `std::env::temp_dir()` dirs keyed by pid) matches existing `registry.rs` tests (line 476 et seq.).

## Assessment

1. **Correctness / spec coverage** — The plan maps 1:1 onto all seven task acceptance criteria: types match the required shapes (`Provider{Ollama,LlamaCpp}`, `Mode{Standard,Balanced,Aggressive}`, `ExploreConfig` with the five required fields), load/save/validate against `.grove/explore.json`, Ollama defaults with the exact `["grove","rg","grep","find"]` allowlist, and the build/clippy/test/tree gates.
2. **Scope discipline** — Correctly keeps steering *content* out (T03's job) per the task Context; purely additive, only `lib.rs` touched among existing files. Good.
3. **Architecture** — Follows existing conventions (anyhow `.with_context`, serde lowercase enums, project-local `.grove/` sibling). Atomic rename-in-same-dir is the right approach (same-filesystem rename).
4. **Testing** — Covers all four required test categories plus a leftover-temp-file assertion; command gates enumerated explicitly.

## Advisory Notes (non-blocking)

1. **"Bad enum value names the field"** (task AC #3): a raw serde_json unknown-variant error enumerates legal values but reports line/column, not the field name. Ensure `load` wraps deserialization with enough `.with_context(...)` (or a custom message) that the surfaced error actually names the field, as the AC demands — the validation-error unit test should assert on the field name, not just the variant list.
2. **Default `model`** is specified only as "a sensible default identifier" — pick a concrete value during implementation and assert it in the defaults test so the contract with T04/T05/T06 is unambiguous.
3. **Atomic save on Windows**: `rename` over an existing file can fail on Windows; grove appears POSIX-first, but if cross-platform matters, note the limitation or handle the pre-existing-destination case.
