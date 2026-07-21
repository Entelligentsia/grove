#!/usr/bin/env bash
# Bump grove's version in every place that carries it, in one shot, so no
# source-of-truth is forgotten (see RELEASING.md). Idempotent: re-running with
# the same version is a no-op beyond a lockfile touch.
#
# The repo is a Cargo workspace with four published members:
#   - core/         -> crates.io `grove-cst`          (lib name `grove_core`)
#   - explore/      -> crates.io `grove-explore-core`  (lib name `grove_explore_core`)
#   - cli/          -> crates.io `grove-cst-cli`      (bin name `grove`)
#   - grove-explore/ -> crates.io `grove-explore-bin`  (bin name `grove-explore`)
#
# What it edits:
#   1-4. Each member's [package] `version`
#   5.   Every exact dependency pin (`grove-cst = "=X.Y.Z"`, `grove-explore-core = "=X.Y.Z"`)
#        in the workspace crate manifests that consume them
#   6.   Cargo.lock entries for all four crates (via `cargo update`)
#   7.   dist/npm/package.json `"version"`
#   8.   CHANGELOG.md dated stub (skipped if present)
#
# It does NOT commit, tag, or push — it only edits files and prints the next
# steps. The Homebrew formula and GitHub Release assets are derived AFTER the
# tag and are intentionally not touched here.
#
# Usage:  scripts/bump-version.sh X.Y.Z
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="${1:?usage: bump-version.sh X.Y.Z}"
case "$VERSION" in
  v*) echo "error: pass the bare version (X.Y.Z), not a tag ($VERSION)" >&2; exit 1 ;;
esac
if ! printf '%s' "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "error: '$VERSION' is not X.Y.Z" >&2; exit 1
fi

say() { printf '\n\033[1m== %s\033[0m\n' "$1"; }

bump_package_version() {
  local file="$1" label="$2"
  say "$label"
  perl -i -pe 'if (!$done && /^version = "/) { s/^version = ".*"/version = "'"$VERSION"'"/; $done=1 }' "$file"
  grep -m1 '^version = ' "$file"
}

bump_package_version cli/Cargo.toml        "1/8  cli/Cargo.toml (grove-cst-cli) -> $VERSION"
bump_package_version core/Cargo.toml       "2/8  core/Cargo.toml (grove-cst) -> $VERSION"
bump_package_version explore/Cargo.toml    "3/8  explore/Cargo.toml (grove-explore-core) -> $VERSION"
bump_package_version grove-explore/Cargo.toml "4/8  grove-explore/Cargo.toml (grove-explore-bin) -> $VERSION"

say "5/8  workspace crate dependency pins -> =$VERSION"
# Update every exact version pin on intra-workspace dependencies so they match
# the new package versions. The pins must be exact (=X.Y.Z) for crates.io
# publish to resolve them from the registry.
for f in cli/Cargo.toml explore/Cargo.toml grove-explore/Cargo.toml; do
  perl -i -pe 's/(grove-(cst|explore-core)\s*=\s*\{[^}]*version\s*=\s*)"=[^"]*"/$1"='"$VERSION"'"/' "$f"
  grep -E 'grove-(cst|explore-core)' "$f"
done

say "6/8  Cargo.lock (all four crate entries)"
cargo update -p grove-cst -p grove-explore-core -p grove-cst-cli -p grove-explore-bin >/dev/null 2>&1 || cargo generate-lockfile >/dev/null

lock_entry_ok() {
  local pkg="$1"
  local lock_v
  lock_v="$(awk '/^name = "'"$pkg"'"$/{getline; print; exit}' Cargo.lock)"
  echo "Cargo.lock $pkg $lock_v"
  case "$lock_v" in
    *"\"$VERSION\""*) : ;;
    *) echo "error: Cargo.lock $pkg entry is not $VERSION ($lock_v)" >&2; exit 1 ;;
  esac
}

lock_entry_ok "grove-cst"
lock_entry_ok "grove-explore-core"
lock_entry_ok "grove-cst-cli"
lock_entry_ok "grove-explore-bin"

say "7/8  dist/npm/package.json -> $VERSION"
perl -i -pe 's/("version":\s*)"[^"]*"/$1"'"$VERSION"'"/ if !$done && /"version":/ and ($done=1)' dist/npm/package.json
grep -m1 '"version":' dist/npm/package.json

say "8/8  CHANGELOG.md"
if grep -qE "^## \[$VERSION\]" CHANGELOG.md; then
  echo "CHANGELOG already has a [$VERSION] section — leaving it."
else
  today="$(date +%F)"
  # Insert a stub immediately before the first existing release section. The
  # scratch file is removed on any exit path so a failed awk leaves no litter.
  trap 'rm -f CHANGELOG.md.tmp' EXIT
  awk -v ver="$VERSION" -v day="$today" '
    !done && /^## \[/ {
      print "## [" ver "] - " day "\n\n### Added\n\n- TODO: describe the change.\n"
      done=1
    }
    { print }
  ' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md
  echo "Inserted ## [$VERSION] - $today (fill in the TODO)."
fi

cat <<EOF

Done. Edited:
  - cli/Cargo.toml (package version + dependency pins)
  - core/Cargo.toml (package version)
  - explore/Cargo.toml (package version + dependency pin)
  - grove-explore/Cargo.toml (package version + dependency pins)
  - Cargo.lock (all four crate entries)
  - dist/npm/package.json
  - CHANGELOG.md

Next (see RELEASING.md):
  1. Fill in the CHANGELOG [$VERSION] section.
  2. cargo build --release --locked && cargo test --release --locked
  3. git switch -c release/v$VERSION && git commit -am "release: v$VERSION"
     then PR to main, merge.
  4. git tag -a v$VERSION -m "grove v$VERSION" && git push origin v$VERSION
  5. npm publish (from dist/npm/) + homebrew tap regen, after the Release exists.
EOF
