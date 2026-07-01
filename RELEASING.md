# Releasing grove

The canonical runbook for cutting a grove release. It is short on purpose — an
agent (or a human) should be able to ship a release end-to-end from this file
without rediscovering the process.

The release is **tag-driven**: pushing a `vX.Y.Z` tag triggers
[`.github/workflows/release.yml`](.github/workflows/release.yml), which builds
the five platform binaries and creates the GitHub Release with the archives and
their `.sha256` sidecars. Everything downstream (npm, Homebrew) reads from that
Release, so the tag must land **before** those steps.

## The crates (workspace layout)

grove is a **Cargo workspace** with two published members. The crates.io ids are
namespaced because `grove` and `grove-core` are taken by unrelated projects — but
the library name and the installed binary are unchanged:

| Dir | crates.io package | lib/bin name | What it is |
|---|---|---|---|
| `core/` | **`grove-cst`** | `lib` name `grove_core` | the engine library (`use grove_core::…`) |
| `cli/` | **`grove-cst-cli`** | `[[bin]]` name `grove` | the CLI + MCP binary (`cargo install grove-cst-cli` → binary `grove`) |

The root `Cargo.toml` is a virtual manifest (`[workspace]`) and carries **no
version** — the two member crates do, and they move together.

## Version source of truth

The version lives in the two crate manifests (kept in lockstep) plus the npm
wrapper and the exact dependency pin, plus the changelog:

| File | What to change |
|---|---|
| `cli/Cargo.toml` | the `grove-cst-cli` `[package] version = "X.Y.Z"` **and** the `grove-cst = { …, version = "=X.Y.Z" }` pin |
| `core/Cargo.toml` | the `grove-cst` `[package] version = "X.Y.Z"` |
| `Cargo.lock` | the `name = "grove-cst"` and `name = "grove-cst-cli"` entries (refresh with `cargo update -p grove-cst -p grove-cst-cli`) |
| `dist/npm/package.json` | `"version": "X.Y.Z"` |
| `CHANGELOG.md` | add a dated `## [X.Y.Z] - YYYY-MM-DD` section at the top |

`scripts/bump-version.sh X.Y.Z` edits all of these in one shot (see step 2).

**Derived, do not hand-edit:** the GitHub Release assets, `dist/homebrew/grove.rb`
hashes (filled by `update-formula.sh` from the release sidecars), and the tap's
`Formula/grove.rb`.

> `Cargo.lock` also lists unrelated crates that happen to share a version number
> (e.g. some dep at `0.1.7`) — only the `grove-cst` / `grove-cst-cli` entries are
> ours. And `README.md` cites historical "grove vX.Y.Z" benchmark data; those are
> factual and must **not** be bumped.

## Steps

1. **Branch** `release/vX.Y.Z` off `main`.
2. **Bump** the four version locations above with one command — it edits
   `Cargo.toml`, refreshes the `Cargo.lock` grove entry, bumps
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
5. **Publish crates to crates.io** — publish the library `grove-cst` first, then
   the binary `grove-cst-cli`. `grove-cst` must be published and indexed before
   `grove-cst-cli`, because crates.io resolves the exact-version dependency
   `grove-cst = "=X.Y.Z"` from the registry at publish time. Publishing the CLI
   before `grove-cst` is indexed will fail.
   ```sh
   cargo publish -p grove-cst
   # wait ~30 s for crates.io to index the new grove-cst version
   cargo publish -p grove-cst-cli
   ```
   Verify with `cargo search grove-cst` and `cargo search grove-cst-cli` that both
   appear at the new version.

   > **First-ever publish only:** the names are unclaimed until the first release.
   > Publishing claims them under your crates.io account — afterwards add the team
   > as owners with `cargo owner --add <gh-user-or-team> grove-cst` (and the same
   > for `grove-cst-cli`). A verified email is required to publish.

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
   `releases/download/vX.Y.Z`, so the Release (step 6) must already exist.
9. **Homebrew tap** — regenerate the formula from the published sidecars and PR it
   into `Entelligentsia/homebrew-grove`:
   ```sh
   dist/homebrew/update-formula.sh vX.Y.Z > ../homebrew-grove/Formula/grove.rb
   # branch + commit "release: vX.Y.Z" + PR + merge in the tap repo
   ```
   `update-formula.sh` pulls each `sha256` from the release's `.sha256` assets —
   never edit hashes by hand.

## Ordering constraints (why the sequence matters)

- **Tag/Release before npm and Homebrew.** Both `install.js` and
  `update-formula.sh` fetch artifacts/hashes from the `vX.Y.Z` Release; if it
  doesn't exist yet they fail or pin stale data.
- **Bump npm `package.json` before publishing**, not after — its version *is* the
  download URL (`releases/download/v${version}`).
- **`grove-cst` before `grove-cst-cli` on crates.io.** The CLI crate depends on
  `grove-cst = "=X.Y.Z"`. crates.io resolves this exact pin from the registry at
  publish time; if `grove-cst` is not yet indexed the `grove-cst-cli` publish fails.

## Smoke checks

- `gh release view vX.Y.Z` shows 10 assets (5 `tar.gz`/`zip` + 5 `.sha256`).
- `brew install Entelligentsia/grove/grove` resolves the new version (after the
  tap PR merges).
- `npx @entelligentsia/grove@X.Y.Z --version` (after `npm publish`).
