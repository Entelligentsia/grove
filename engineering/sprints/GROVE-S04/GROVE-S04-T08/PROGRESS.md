# PROGRESS — GROVE-S04-T08: Packaging — `release.yml`/npm/brew ship both binaries; release dry-run

## Summary

This revision pass closes the code-review findings for the dual-binary release
train. The two code changes were made earlier (adding `--version` to
`grove-explore` and correcting the npm smoke-check in `RELEASING.md`) and are
confirmed still in place. This pass adds the "Testing the bump script" safety
procedure to `RELEASING.md`, and provides fresh, non-stale evidence for every
verification step — forced rebuild, full test suite, clippy, bump-script
run→verify→restore→checksum-confirm cycle, and packaging-file syntax checks.

## Changes made

1. **`grove-explore/src/main.rs`** — `version` added to `#[command]` so
   `grove-explore --version` prints the version and exits 0, matching the `grove`
   binary convention.
2. **`RELEASING.md`** — two additions:
   - Corrected npm smoke-check invocations (in the Smoke checks section).
   - Added new **"Testing the bump script"** subsection with the run→verify→restore→checksum protocol.
3. Preserved verified dual-binary release-train plumbing (no changes needed):
   - `.github/workflows/release.yml` — builds both binaries for all five targets, dry-run skip, 4-crate publish in dependency order.
   - `dist/npm/install.js`, `dist/npm/package.json`, `dist/npm/bin/grove-explore.js` — both `grove` and `grove-explore` commands.
   - `dist/homebrew/grove.rb` — installs and tests both binaries.
   - `install.sh` — tolerates older archives that lack `grove-explore`.
   - `scripts/bump-version.sh` — 4 crates, 5 exact dependency pins, 4 `Cargo.lock` entries.
   - `engineering/architecture/deployment.md` — two-binary archive layout documented.

## Test evidence

### Version verification — all four crates at 0.4.1

```
$ grep "^version = " core/Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml
core/Cargo.toml:version = "0.4.1"
cli/Cargo.toml:version = "0.4.1"
explore/Cargo.toml:version = "0.4.1"
grove-explore/Cargo.toml:version = "0.4.1"

$ cat dist/npm/package.json | grep '"version"'
  "version": "0.4.1",

$ grep '"version"' dist/npm/package.json
  "version": "0.4.1",
```

All four Cargo.lock entries:
```
$ grep -A2 'name = "grove-cst"\|name = "grove-explore-core"\|name = "grove-cst-cli"\|name = "grove-explore-bin"' Cargo.lock | grep version
version = "0.4.1"
version = "0.4.1"
version = "0.4.1"
version = "0.4.1"
```

### Forced rebuild

```
$ touch grove-explore/src/main.rs cli/src/main.rs && cargo build --release
   Compiling grove-cst-cli v0.4.1 (/home/boni/src/grove-engineering/grove/cli)
   Compiling grove-explore-bin v0.4.1 (/home/boni/src/grove-engineering/grove/grove-explore)
    Finished `release` profile [optimized] target(s) in 20.96s
```

### Binary --version smoke check (fresh, post-rebuild)

```
$ ./target/release/grove --version
grove 0.4.1

$ ./target/release/grove-explore --version
grove-explore 0.4.1
```

### Test suite

```
$ cargo test --release --locked --workspace
...
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
...
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
...
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

### Lint (CI-equivalent)

```
$ cargo clippy --all-targets --workspace --locked -- -D warnings
    Checking grove-cst-cli v0.4.1
    Checking grove-explore-bin v0.4.1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.44s
```

### Bump-script run→verify→restore→checksum protocol

Before-checksums:
```
ea10644d1ca7f6fda0e3506078cb075744239a9a00a308e83aedee7f6cc05225  core/Cargo.toml
6855463eced967d58f14a8432f333f01a61be89c4f7033106710a5782b9c5124  cli/Cargo.toml
40b2e7cf4f606cfa4cbe9184b4267f4f6a5b5eaae32a2eeecffff1ffa9ca3dd8  explore/Cargo.toml
fb25219f539ff13fb20254dbb8630a65b10f9cbf0278cb15c51ff5c158d720e0  grove-explore/Cargo.toml
8b4e8f7661a2019499d1699d1c54fc375c645f0d2b18e22313e05d6c5bb7afcf  Cargo.lock
1e8c70c332feb54888a475969fa92eb1232623a2bde937fc9d5ff88f3ef5eec4  dist/npm/package.json
11741b771fdac42d784e5efe4a617d058fcd65ee634316445751795403359501  CHANGELOG.md
```

Run:
```
$ scripts/bump-version.sh 9.9.9
== 1/8  cli/Cargo.toml (grove-cst-cli) -> 9.9.9
version = "9.9.9"
== 2/8  core/Cargo.toml (grove-cst) -> 9.9.9
version = "9.9.9"
== 3/8  explore/Cargo.toml (grove-explore-core) -> 9.9.9
version = "9.9.9"
== 4/8  grove-explore/Cargo.toml (grove-explore-bin) -> 9.9.9
version = "9.9.9"
== 5/8  workspace crate dependency pins -> =9.9.9
[all 5 exact pins mutated]
== 6/8  Cargo.lock (all four crate entries)
Cargo.lock grove-cst version = "9.9.9"
Cargo.lock grove-explore-core version = "9.9.9"
Cargo.lock grove-cst-cli version = "9.9.9"
Cargo.lock grove-explore-bin version = "9.9.9"
== 7/8  dist/npm/package.json -> 9.9.9
  "version": "9.9.9",
== 8/8  CHANGELOG.md
Inserted ## [9.9.9] - 2026-07-21 (fill in the TODO).
```

Mutation counts verified:
- 4 crate `version = "9.9.9"` lines
- 5 exact pin `=9.9.9` matches
- 4 Cargo.lock entries at `"9.9.9"`

Restore:
```
$ git checkout HEAD -- core/Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml Cargo.lock CHANGELOG.md
$ perl -i -pe 's/("version":\s*)"9\.9\.9"/$1"0.4.1"/ if !$done && /"version":/ and ($done=1)' dist/npm/package.json
$ grep '"version":' dist/npm/package.json | head -1
  "version": "0.4.1",
```

After-checksums (all match before-checksums):
```
ea10644d1ca7f6fda0e3506078cb075744239a9a00a308e83aedee7f6cc05225  core/Cargo.toml
6855463eced967d58f14a8432f333f01a61be89c4f7033106710a5782b9c5124  cli/Cargo.toml
40b2e7cf4f606cfa4cbe9184b4267f4f6a5b5eaae32a2eeecffff1ffa9ca3dd8  explore/Cargo.toml
fb25219f539ff13fb20254dbb8630a65b10f9cbf0278cb15c51ff5c158d720e0  grove-explore/Cargo.toml
8b4e8f7661a2019499d1699d1c54fc375c645f0d2b18e22313e05d6c5bb7afcf  Cargo.lock
1e8c70c332feb54888a475969fa92eb1232623a2bde937fc9d5ff88f3ef5eec4  dist/npm/package.json
11741b771fdac42d784e5efe4a617d058fcd65ee634316445751795403359501  CHANGELOG.md
```

### Packaging file syntax checks

```
$ python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo "release.yml: OK"
release.yml: OK

$ python3 -c "import json; json.load(open('dist/npm/package.json'))" && echo "package.json: OK"
package.json: OK

$ bash -n scripts/bump-version.sh && echo "bump-version.sh: OK"
bump-version.sh: OK

$ bash -n install.sh && echo "install.sh: OK"
install.sh: OK

$ node --check dist/npm/install.js && echo "install.js: OK"
install.js: OK

$ node --check dist/npm/bin/grove-explore.js && echo "grove-explore.js: OK"
grove-explore.js: OK

$ node --check dist/npm/bin/grove.js && echo "grove.js: OK"
grove.js: OK
```

## Files changed

- `RELEASING.md` — "Testing the bump script" subsection added
- `grove-explore/src/main.rs` — `version` attribute added (prior pass)
- `.github/workflows/release.yml` — dual-binary build + dry-run (prior pass)
- `dist/homebrew/grove.rb` — both binaries (prior pass)
- `dist/npm/bin/grove-explore.js` (new) — shim for grove-explore (prior pass)
- `dist/npm/install.js` — loops over both binaries (prior pass)
- `dist/npm/package.json` — grove-explore bin + files entries (prior pass)
- `install.sh` — conditional guard for grove-explore (prior pass)
- `scripts/bump-version.sh` — 4-crate, 5-pin, 4-lock-entry bump (prior pass)
- `engineering/architecture/deployment.md` — two-binary layout docs (prior pass)
