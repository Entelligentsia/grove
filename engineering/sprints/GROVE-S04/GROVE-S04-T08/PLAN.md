# PLAN — GROVE-S04-T08 (Revision): Packaging — `release.yml`/npm/brew ship both binaries; release dry-run

**Revision context:** The first code-review pass returned a `revision` verdict because
`scripts/bump-version.sh 9.9.9` was run against the real working tree during testing,
leaving all four workspace crates, `Cargo.lock`, `dist/npm/package.json`, and
`CHANGELOG.md` at `9.9.9`. The packaging implementation itself (release.yml, npm,
Homebrew, install.sh, bump-version.sh, RELEASING.md, deployment.md) was assessed as
correct; only the version-bump test pollution needed remediation.

Current repo state (verified before writing this plan): all version files are at
`0.4.1`. The implementation plan below targets what needs to happen in the implement
pass to produce clean, trustworthy evidence and close the revision.

---

## Objective

Close the code-review revision for the dual-binary release train by: (1) producing a
fresh, forced rebuild at `0.4.1`, (2) running the full test and lint suite against the
clean tree, and (3) exercising `bump-version.sh` **only against a temp copy** to
demonstrate script correctness without mutating the working tree. Everything already
verified in code review (release.yml, npm, Homebrew, install.sh, RELEASING.md,
deployment.md) is preserved as-is.

---

## Approach

### Step 1 — Confirm version cleanliness

Verify every version-bearing location is at `0.4.1` before doing any build work:

| File | Field | Expected |
|---|---|---|
| `Cargo.toml` | `[package] version` | `0.4.1` |
| `cli/Cargo.toml` | `[package] version` | `0.4.1` |
| `cli/Cargo.toml` | `grove-cst = { … version = "=…" }` | `=0.4.1` |
| `cli/Cargo.toml` | `grove-explore-core = { … version = "=…" }` | `=0.4.1` |
| `explore/Cargo.toml` | `[package] version` | `0.4.1` |
| `explore/Cargo.toml` | `grove-cst = { … version = "=…" }` | `=0.4.1` |
| `grove-explore/Cargo.toml` | `[package] version` | `0.4.1` |
| `grove-explore/Cargo.toml` | `grove-cst = { … version = "=…" }` | `=0.4.1` |
| `grove-explore/Cargo.toml` | `grove-explore-core = { … version = "=…" }` | `=0.4.1` |
| `Cargo.lock` | all four crate name entries | `0.4.1` |
| `dist/npm/package.json` | `"version"` | `0.4.1` |
| `CHANGELOG.md` | top section header | no `9.9.9` section |

If any file still contains `9.9.9`, restore it from HEAD with `git checkout HEAD -- <file>`.
For `dist/npm/package.json` — which cannot be fully restored from HEAD because it also
carries the legitimate `grove-explore` bin/files entries added in this task — set the
version field to `"0.4.1"` with a targeted edit.

### Step 2 — Fresh forced build and --version smoke check

Force a full recompile of both binaries to ensure the binary on disk reflects the
clean tree (not a stale pre-pollution binary):

```bash
touch core/src/lib.rs cli/src/main.rs grove-explore/src/main.rs
cargo build --release --locked --workspace
./target/release/grove --version          # must print: grove 0.4.1
./target/release/grove-explore --version  # must print: grove-explore 0.4.1
```

Capture and record both outputs as evidence in PROGRESS.md.

### Step 3 — Full test and lint suite

```bash
cargo test --release --locked --workspace
cargo clippy --all-targets --workspace --locked -- -D warnings
```

All tests must pass; clippy must produce no warnings. Record output in PROGRESS.md.

### Step 4 — Bump-script temp-copy exercise

To demonstrate `scripts/bump-version.sh` correctly handles all four crates, five
exact pins, and four `Cargo.lock` entries — **without corrupting the working tree**:

```bash
# Create a throwaway copy directory
TMPDIR=$(mktemp -d)
# Copy only the version-bearing files into temp copy
cp Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml \
   Cargo.lock dist/npm/package.json CHANGELOG.md "$TMPDIR/"
# Run the script's bump logic against the temp copies by sourcing with env override
# (or review the script's output functions with bash --verbose on the temp files)
```

Implementation note: `scripts/bump-version.sh` operates in-place on the working tree;
it cannot be pointed at a temp directory without modification. The safe approach is:

1. **Review-only pass**: read `scripts/bump-version.sh` and confirm it correctly
   handles all four package versions, all five exact pins, and all four `Cargo.lock`
   entries via grep against the current file.
2. **Checksum before**: record `sha256sum` of all version-bearing files.
3. **Run the script** against the real tree with `scripts/bump-version.sh 9.9.9`.
4. **Verify output**: grep each file to confirm the expected mutations.
5. **Restore immediately** with `git checkout HEAD -- Cargo.toml cli/Cargo.toml
   explore/Cargo.toml grove-explore/Cargo.toml Cargo.lock CHANGELOG.md` and
   edit `dist/npm/package.json` version back to `"0.4.1"` (targeted edit, preserving
   the grove-explore bin/files entries).
6. **Checksum after**: confirm all checksums match the before snapshot.
7. Record both the mutation evidence and the restoration confirmation in PROGRESS.md.

This procedure is the canonical bump-script test from now on: run → verify → restore → checksum-confirm. Document in RELEASING.md under the "Testing the bump script" subsection.

### Step 5 — JavaScript and shell syntax checks

```bash
node --check dist/npm/install.js
node --check dist/npm/bin/grove.js
node --check dist/npm/bin/grove-explore.js
sh -n install.sh
sh -n dist/homebrew/update-formula.sh
bash -n scripts/bump-version.sh
python3 -c "import json; json.load(open('dist/npm/package.json'))" && echo "package.json OK"
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo "release.yml OK"
```

Record all outputs in PROGRESS.md.

### Step 6 — RELEASING.md bump-script section

Add a "Testing the bump script" subsection to `RELEASING.md` (under the "Version
bump" section) that codifies the run → verify → restore → checksum-confirm procedure
from Step 4. This prevents recurrence of the version-corruption pattern by making
the safe testing procedure canonical and visible.

---

## Files to Modify

| File | Change |
|---|---|
| `RELEASING.md` | Add "Testing the bump script" subsection documenting run→verify→restore→checksum-confirm procedure |
| `engineering/sprints/GROVE-S04/GROVE-S04-T08/PROGRESS.md` | Write fresh evidence: version verification table, forced-rebuild --version output, test/lint output, bump-script mutation evidence, restoration checksum confirmation |

All other files (`release.yml`, `dist/npm/`, `dist/homebrew/`, `install.sh`,
`scripts/bump-version.sh`, `engineering/architecture/deployment.md`) were assessed as
correct in the code review and are **not changed**.

---

## Data Model Changes

None.

---

## Testing Strategy

All test evidence (version verification, forced rebuild, test suite, lint, bump-script
mutation, restoration checksum, JS/shell syntax checks) is recorded verbatim in
PROGRESS.md with command + output pairs. The reviewer can verify each claim by
repeating the commands against the current working tree.

---

## Acceptance Criteria

- [ ] All twelve version-bearing locations confirm `0.4.1` (no `9.9.9` remains).
- [ ] `./target/release/grove --version` prints `grove 0.4.1` from a forced-recompile build.
- [ ] `./target/release/grove-explore --version` prints `grove-explore 0.4.1` from the same build.
- [ ] `cargo test --release --locked --workspace` passes with no failures.
- [ ] `cargo clippy --all-targets --workspace --locked -- -D warnings` produces no warnings.
- [ ] `scripts/bump-version.sh 9.9.9` correctly mutates all four crates, five pins, and four Cargo.lock entries (verified), then all files are restored to `0.4.1` and the sha256 checksums match the pre-run snapshot.
- [ ] `RELEASING.md` includes a "Testing the bump script" subsection documenting the safe run→verify→restore→checksum-confirm procedure.
- [ ] All JS/shell/YAML/JSON syntax checks pass.
- [ ] PROGRESS.md contains fresh, non-stale evidence for every criterion above.

---

## Operational Impact

No user-visible changes. The packaging implementation is not altered; only documentation
and test-evidence hygiene are updated. Versions remain at `0.4.1` at the conclusion of
this task. The next real release tag will pick up the dual-binary train as-is.
