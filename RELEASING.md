# Releasing grove

The canonical runbook for cutting a grove release. It is short on purpose — an
agent (or a human) should be able to ship a release end-to-end from this file
without rediscovering the process.

The release is **tag-driven**: pushing a `vX.Y.Z` tag triggers
[`.github/workflows/release.yml`](.github/workflows/release.yml), which builds
the five platform binaries and creates the GitHub Release with the archives and
their `.sha256` sidecars. Everything downstream (npm, Homebrew) reads from that
Release, so the tag must land **before** those steps.

## Version source of truth

The version lives in three places that must move together, plus the changelog:

| File | What to change |
|---|---|
| `Cargo.toml` | `version = "X.Y.Z"` |
| `Cargo.lock` | the `name = "grove"` entry (refresh with `cargo build`, or `cargo update -p grove`) |
| `dist/npm/package.json` | `"version": "X.Y.Z"` |
| `CHANGELOG.md` | add a dated `## [X.Y.Z] - YYYY-MM-DD` section at the top |

**Derived, do not hand-edit:** the GitHub Release assets, `dist/homebrew/grove.rb`
hashes (filled by `update-formula.sh` from the release sidecars), and the tap's
`Formula/grove.rb`.

> `Cargo.lock` also lists unrelated crates that happen to share a version number
> (e.g. some dep at `0.1.7`) — only the `name = "grove"` entry is ours. And
> `README.md` cites historical "grove vX.Y.Z" benchmark data; those are factual
> and must **not** be bumped.

## Steps

1. **Branch** `release/vX.Y.Z` off `main`.
2. **Bump** the four version locations above. Run `cargo build --release --locked`
   to refresh `Cargo.lock` and prove it still builds locked (release CI uses
   `--locked`). Run `cargo test --release --locked`.
3. **Commit** `release: vX.Y.Z`, push, open a PR to `main`, wait for `ci` green,
   merge.
4. **Tag the merge commit and push** — this is what ships:
   ```sh
   git checkout main && git pull --ff-only
   git tag -a vX.Y.Z -m "grove vX.Y.Z" && git push origin vX.Y.Z
   ```
5. **Watch the release build** and confirm the assets:
   ```sh
   gh run watch "$(gh run list --workflow=release.yml -L1 --json databaseId -q '.[0].databaseId')" --exit-status
   gh release view vX.Y.Z --json assets -q '.assets[].name'   # expect 5 archives + 5 .sha256
   ```
6. **Release notes** — populate the body from the changelog section:
   ```sh
   awk '/^## \[X\.Y\.Z\]/{f=1} /^## \[/{if(f&&!/X\.Y\.Z/)exit} f' CHANGELOG.md > /tmp/notes.md
   gh release edit vX.Y.Z --notes-file /tmp/notes.md
   ```
7. **npm publish** — from `dist/npm/`, `npm publish`. `install.js` downloads from
   `releases/download/vX.Y.Z`, so the Release (step 5) must already exist.
8. **Homebrew tap** — regenerate the formula from the published sidecars and PR it
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

## Smoke checks

- `gh release view vX.Y.Z` shows 10 assets (5 `tar.gz`/`zip` + 5 `.sha256`).
- `brew install Entelligentsia/grove/grove` resolves the new version (after the
  tap PR merges).
- `npx @entelligentsia/grove@X.Y.Z --version` (after `npm publish`).
