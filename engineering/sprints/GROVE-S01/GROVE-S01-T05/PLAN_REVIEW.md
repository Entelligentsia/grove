# PLAN REVIEW — GROVE-S01-T05: Make CI & release tooling workspace-aware

🌿 **grove Supervisor** — (standalone review)

**Task:** GROVE-S01-T05
**Sprint:** GROVE-S01
**Phase:** review-plan

---

## Verdict: **Approved**

The plan is correct, well-reasoned, and low-risk. I verified every premise
against the actual codebase independently — the workspace layout, the current
state of all three target files, the four verify-only files, and the
`cargo build --release --locked -p grove` dry validation all confirm the
plan's factual claims. The mechanical edits described are sound.

---

## Independent Verification (premises checked against actual files)

| Premise | Verified | Notes |
|---|---|---|
| Root `Cargo.toml` is a virtual `[workspace]` with members `core`+`cli`, no root `[package]` | ✅ | `[workspace] members = ["core", "cli"]`, resolver = "2", only `[profile.release]` |
| `cli/Cargo.toml` = package `grove` v0.1.11 with `[[bin]] name = "grove"` | ✅ | `path = "src/main.rs"`; depends on `grove-core = { path = "../core" }` |
| `core/Cargo.toml` = package `grove-core` v0.1.11 | ✅ | tree-sitter 0.25 wasm, serde, ignore, sha2, ureq, dirs |
| `Cargo.lock` has both `grove` (0.1.11) and `grove-core` (0.1.11) entries | ✅ | lines 539–552; awk anchors `^name = "grove"$` won't false-match `grove-core` |
| `release.yml` currently builds bare `cargo build --release --locked --target <t>` (no `-p`); tars `target/<t>/release/grove` | ✅ | 5 targets confirmed; unix tar / windows 7z / sha256 / upload glob `grove-<target>.*` |
| `ci.yml` currently runs bare `cargo build/test --release --locked` (no `--workspace`) | ✅ | linux-only build-test job; `paths-ignore` includes `dist/**` (fine) |
| `bump-version.sh` already bumps `cli/Cargo.toml` (not root), `cargo update -p grove`, npm, CHANGELOG | ✅ | header comment stale ("Cargo.toml") but code targets `cli/Cargo.toml`; 4 steps labelled "1/4".."4/4" |
| `setup-local-test.sh` resolves `BIN="$REPO/target/release/grove"` from `cargo build --release` | ✅ | workspace root build emits the `grove` binary at that path |
| `dist/npm/install.js` downloads `grove-<target>.<ext>` by name | ✅ | 5 TARGETS map; asset names tied to release.yml, unchanged by `-p grove` |
| `dist/homebrew/update-formula.sh` pulls `grove-<target>.tar.gz.sha256` | ✅ | 4 unix targets (no Windows — correct for Homebrew) |
| `cargo build --release --locked -p grove` succeeds and emits `target/release/grove` | ✅ | ran it: Finished, binary present (12.6 MB) |

---

## Spec Compliance (AC-by-AC)

- **AC#1** (release.yml `-p grove`, 5 targets, asset path unchanged): ✅
  Plan correctly pins the build to the `grove` package. Since only `cli`
  declares a `grove` binary, `target/<t>/release/grove[.exe]` is byte-for-byte
  unchanged. The change is defensive (prevents a future second binary from
  silently entering the asset) — correct rationale.
- **AC#2** (ci.yml `--workspace` build+test): ✅
  The `--workspace` flag is the right choice for CI. See advisory note A1
  re: the current virtual-workspace behaviour.
- **AC#3** (bump-version.sh: core+cli Cargo.toml, both lock entries, npm, idempotent): ✅
  `cargo update -p grove -p grove-core` is valid (multiple `-p` flags
  accepted). Path dep `grove-core = { path = "../core" }` has no version
  constraint, so the lock picks up the new version cleanly. Idempotency holds:
  same-version re-run → perl no-ops + `cargo update` no-ops.
- **AC#4** (setup-local-test.sh resolves binary): ✅ Confirmed unchanged.
- **AC#5** (npm + homebrew verify-only): ✅ Confirmed unchanged.
- **AC#6** (dry validation + asset-name review): ✅ Strategy is adequate;
  see advisory A2 for a recommended strengthening.

---

## Advisory Notes (non-blocking)

**A1 — ci.yml `--workspace` is future-proofing, not a current fix.**
In the current virtual workspace (no root `[package]`), bare `cargo build`
and `cargo test` already operate on all members — `grove-core` is already
built and tested today. The `--workspace` flag makes this explicit and
guards against a future root `[package]` being added (which would silently
narrow bare `cargo build` to the root crate only). The change is correct and
worthwhile, but the plan's framing ("was non-workspace single-crate") is
inherited from the task prompt's pre-T02 state and is slightly inaccurate for
the current tree. No action needed — just be aware the immediate observable
effect is zero; the value is defensive.

**A2 — Promote the throwaway-version bump from optional to recommended.**
The plan's testing strategy lists an idempotent same-version `bump-version.sh`
run as the primary script validation, and a throwaway-version bump as
"optionally." The same-version run proves the perl edits and lock assertions
work, but does NOT prove `cargo update -p grove -p grove-core` actually
writes a NEW version into both lock entries. Recommend making the throwaway
bump (e.g. `0.1.12`, inspect `git diff` across all five sites, then `git
checkout --`) a required validation step, not optional.

**A3 — "Four version sites" should be five.**
The Approach section says "all four version sites (core, cli, lockfile, npm)."
There are actually five: `core/Cargo.toml`, `cli/Cargo.toml`, the `grove`
lock entry, the `grove-core` lock entry, and `dist/npm/package.json`. The AC
is precise ("both Cargo.lock entries") but the prose undercounts. Cosmetic —
the edit itself is correct.

**A4 — Update the stale `bump-version.sh` docstrings.**
Two stale strings should be refreshed during the edit (the plan doesn't
call them out explicitly):
- Header comment line "1. Cargo.toml — the [package] `version`" → should
  reference both `core/Cargo.toml` and `cli/Cargo.toml`.
- The final `cat <<EOF` message says "Edited: Cargo.toml, Cargo.lock, …" →
  should say "core/Cargo.toml, cli/Cargo.toml, Cargo.lock, …".

**A5 — The `|| cargo generate-lockfile` fallback.**
The existing `cargo update -p grove >/dev/null 2>&1 || cargo generate-lockfile`
fallback regenerates the ENTIRE lockfile if `cargo update` fails, which could
pull in newer transitive dependency versions unexpectedly. This is
pre-existing behaviour (not introduced by this plan) and is only hit when no
lockfile exists. If the plan retains this fallback after switching to
`cargo update -p grove -p grove-core`, no harm — just flag it as technical
debt for a future cleanup.

---

## Security

No security concerns. These are build/release tooling changes with no user
input handling, no auth, no injection surface. The `bump-version.sh` perl
edits interpolate `"$VERSION"` after a strict `^[0-9]+\.[0-9]+\.[0-9]+$`
regex gate — shell-safe. ✅

## Architecture & Conventions

Follows the established workspace pattern and existing script conventions
(`set -euo pipefail`, `say()` helper, perl in-place edits). Stack checklist
items for binary packaging (tar.gz/zip + sha256), 5 release targets, and
npm/Homebrew asset naming are all respected. ✅