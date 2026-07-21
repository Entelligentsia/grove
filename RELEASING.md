# Releasing grove

The canonical runbook for cutting a grove release. It is short on purpose — an
agent (or a human) should be able to ship a release end-to-end from this file
without rediscovering the process.

The release is **tag-driven**: pushing a `vX.Y.Z` tag triggers
[`.github/workflows/release.yml`](.github/workflows/release.yml), which builds
the five platform archives and creates the GitHub Release with the archives and
their `.sha256` sidecars. Each archive contains **both** `grove` and
`grove-explore` binaries. Everything downstream (npm, Homebrew) reads from that
Release, so the tag must land **before** those steps.

## Dry-run mode

`release.yml` supports a non-publishing dry-run via `workflow_dispatch`:

```sh
gh workflow run release.yml --ref main -f dry_run=true
```

This builds and packages both binaries for all five targets, uploads the ten
workflow artifacts (5 archives + 5 `.sha256` sidecars), and **skips** the GitHub
Release upload and crates.io publish. Use it to prove the release train before
creating a real tag.

## The crates (workspace layout)

grove is a **Cargo workspace** with four published members. The crates.io ids are
namespaced because `grove`, `grove-core`, and `grove-explore` are taken by
unrelated projects — but the library/binary names are unchanged:

| Dir | crates.io package | lib/bin name | What it is |
|---|---|---|---|
| `core/` | **`grove-cst`** | lib name `grove_core` | the engine library (`use grove_core::…`) |
| `explore/` | **`grove-explore-core`** | lib name `grove_explore_core` | the LLM config / transport / agent loop library |
| `cli/` | **`grove-cst-cli`** | `[[bin]]` name `grove` | the CLI + MCP binary (`cargo install grove-cst-cli` → binary `grove`) |
| `grove-explore/` | **`grove-explore-bin`** | `[[bin]]` name `grove-explore` | the explore TUI / MCP binary (`cargo install grove-explore-bin` → binary `grove-explore`) |

The root `Cargo.toml` is a virtual manifest (`[workspace]`) and carries **no
version** — the four member crates do, and they move together.

## Version source of truth

The version lives in the four crate manifests (kept in lockstep), the exact
intra-workspace dependency pins, the npm wrapper, the lockfile, and the
changelog:

| File | What to change |
|---|---|
| `cli/Cargo.toml` | the `grove-cst-cli` `[package] version = "X.Y.Z"` **and** the `grove-cst = { …, version = "=X.Y.Z" }` and `grove-explore-core = { …, version = "=X.Y.Z" }` pins |
| `core/Cargo.toml` | the `grove-cst` `[package] version = "X.Y.Z"` |
| `explore/Cargo.toml` | the `grove-explore-core` `[package] version = "X.Y.Z"` **and** the `grove-cst = { …, version = "=X.Y.Z" }` pin |
| `grove-explore/Cargo.toml` | the `grove-explore-bin` `[package] version = "X.Y.Z"` **and** the `grove-cst = { …, version = "=X.Y.Z" }` and `grove-explore-core = { …, version = "=X.Y.Z" }` pins |
| `Cargo.lock` | the `grove-cst`, `grove-explore-core`, `grove-cst-cli`, and `grove-explore-bin` entries (refresh with `cargo update -p grove-cst -p grove-explore-core -p grove-cst-cli -p grove-explore-bin`) |
| `dist/npm/package.json` | `"version": "X.Y.Z"` |
| `CHANGELOG.md` | add a dated `## [X.Y.Z] - YYYY-MM-DD` section at the top |

`scripts/bump-version.sh X.Y.Z` edits all of these in one shot (see step 2).

**Derived, do not hand-edit:** the GitHub Release assets, `dist/homebrew/grove.rb`
hashes (filled by `update-formula.sh` from the release sidecars), and the tap's
`Formula/grove.rb`.

> `Cargo.lock` also lists unrelated crates that happen to share a version number
> (e.g. some dep at `0.1.7`) — only the four workspace member entries are ours.
> And `README.md` cites historical "grove vX.Y.Z" benchmark data; those are
> factual and must **not** be bumped.

## Steps

1. **Branch** `release/vX.Y.Z` off `main`.
2. **Bump** the seven version locations above with one command — it edits the
   four `Cargo.toml` files, refreshes the `Cargo.lock` entries, bumps
   `dist/npm/package.json`, and inserts a dated `CHANGELOG.md` stub:
   ```sh
   scripts/bump-version.sh X.Y.Z
   ```
   Then fill in the CHANGELOG stub, and run `cargo build --release --locked`
   (release CI uses `--locked`) and `cargo test --release --locked`.
3. **Commit** `release: vX.Y.Z`, push, open a PR to `main`, wait for `ci` green,
   merge.
4. **Tag the merge commit and push** — this is what ships:
   ```sh
   git checkout main && git pull --ff-only
   git tag -a vX.Y.Z -m "grove vX.Y.Z" && git push origin vX.Y.Z
   ```
5. **Publish crates to crates.io** — **automated.** The `publish-crates` job in
   `release.yml` runs after the binaries build and publishes in dependency order:
   `grove-cst` → `grove-explore-core` → `grove-cst-cli` → `grove-explore-bin`.
   Each `cargo publish` blocks until the crate is indexed, so the next crate in
   the sequence can resolve its exact-version pins from the registry. It
   requires the **`CARGO_REGISTRY_TOKEN`** repo secret (a crates.io API token with
   publish scope) and asserts the tag matches all four crate versions before
   publishing.

   Verify afterwards with:
   ```sh
   cargo search grove-cst
   cargo search grove-explore-core
   cargo search grove-cst-cli
   cargo search grove-explore-bin
   ```

   If you ever need to publish **manually** (secret missing, re-run, first claim):
   ```sh
   cargo publish -p grove-cst --locked
   cargo publish -p grove-explore-core --locked
   cargo publish -p grove-cst-cli --locked
   cargo publish -p grove-explore-bin --locked
   ```

   > **First-ever publish only:** the names are unclaimed until the first release.
   > Publishing claims them under the token's crates.io account — afterwards add the
   > team as owners with `cargo owner --add <gh-user-or-team> grove-cst` (and the
   > same for the other three). A verified email is required to publish, and the
   > `CARGO_REGISTRY_TOKEN` secret must be set for the automated job to work.

6. **Watch the release build** and confirm the assets:
   ```sh
   gh run watch "$(gh run list --workflow=release.yml -L1 --json databaseId -q '.[0].databaseId')" --exit-status
   gh release view vX.Y.Z --json assets -q '.assets[].name'   # expect 5 archives + 5 .sha256
   ```
7. **Release notes** — populate the body from the changelog section:
   ```sh
   awk '/^## \[X\.Y\.Z\]/{f=1} /^## \[/{if(f&&!/X\.Y\.Z/)exit} f' CHANGELOG.md > /tmp/notes.md
   gh release edit vX.Y.Z --notes-file /tmp/notes.md
   ```
8. **npm publish** — from `dist/npm/`, `npm publish`. `install.js` downloads from
   `releases/download/vX.Y.Z`, so the Release (step 6) must already exist. The
   package now exposes both `grove` and `grove-explore` commands.
9. **Homebrew tap** — regenerate the formula from the published sidecars and PR it
   into `Entelligentsia/homebrew-grove`:
   ```sh
   dist/homebrew/update-formula.sh vX.Y.Z > ../homebrew-grove/Formula/grove.rb
   # branch + commit "release: vX.Y.Z" + PR + merge in the tap repo
   ```
   `update-formula.sh` pulls each `sha256` from the release's `.sha256` assets —
   never edit hashes by hand. The resulting formula installs both `grove` and
   `grove-explore`.

## Testing the bump script

`scripts/bump-version.sh` edits real workspace files — **never test it against
the working tree without a restore plan.** Use this run→verify→restore→confirm
protocol to exercise the script safely:

```sh
# 1. Capture before-checksums
sha256sum core/Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml \
          Cargo.lock dist/npm/package.json CHANGELOG.md > /tmp/grove-checksums-before.txt

# 2. Run the script with a dummy version
scripts/bump-version.sh 9.9.9

# 3. Verify all seven locations mutated
grep "^version = \"9.9.9\"" core/Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml
grep "=9.9.9" cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml
grep '"version": "9.9.9"' dist/npm/package.json
grep "## \[9.9.9\]" CHANGELOG.md

# 4. Restore — git reverts everything except package.json (which had prior
#    legitimate uncommitted changes); restore its version field with perl
git checkout HEAD -- core/Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml Cargo.lock CHANGELOG.md
perl -i -pe 's/("version":\s*)"9\.9\.9"/$1"0.4.1"/ if !$done && /"version":/ and ($done=1)' dist/npm/package.json

# 5. Verify checksums match the before-snapshot
sha256sum core/Cargo.toml cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml \
          Cargo.lock dist/npm/package.json CHANGELOG.md > /tmp/grove-checksums-after.txt
diff /tmp/grove-checksums-before.txt /tmp/grove-checksums-after.txt   # must be empty
```

The `git checkout HEAD --` approach is the correct restore path for files
tracked in git. `dist/npm/package.json` requires the targeted perl edit when it
carries other uncommitted changes (the grove-explore bin registration) that must
be preserved.

## Ordering constraints (why the sequence matters)

- **Tag/Release before npm and Homebrew.** Both `install.js` and
  `update-formula.sh` fetch artifacts/hashes from the `vX.Y.Z` Release; if it
  doesn't exist yet they fail or pin stale data.
- **Bump npm `package.json` before publishing**, not after — its version *is* the
  download URL (`releases/download/v${version}`).
- **`grove-cst` before `grove-explore-core` before the binaries on crates.io.**
  The CLI crate depends on `grove-cst = "=X.Y.Z"` and `grove-explore-core =
  "=X.Y.Z"`; `grove-explore-bin` depends on both. crates.io resolves these exact
  pins from the registry at publish time, so each dependency must be indexed
  before the dependent is published.

## Smoke checks

- `gh release view vX.Y.Z` shows 10 assets (5 archives + 5 `.sha256`), and each
  archive contains both `grove` and `grove-explore`.
- `brew install Entelligentsia/grove/grove` resolves the new version and
  installs both commands (after the tap PR merges).
- `npx @entelligentsia/grove@X.Y.Z --version` and
  `npx --package=@entelligentsia/grove@X.Y.Z grove-explore --version`
  (after `npm publish`).
