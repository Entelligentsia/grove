#!/usr/bin/env bash
# Bump grove's version in every place that carries it, in one shot, so no
# source-of-truth is forgotten (see RELEASING.md). Idempotent: re-running with
# the same version is a no-op beyond a lockfile touch.
#
# What it does (the repo is a Cargo workspace: cli=grove, core=grove-core):
#   1. cli/Cargo.toml      — the grove [package] `version`
#   2. core/Cargo.toml     — the grove-core [package] `version`
#   3. Cargo.lock          — both `grove` and `grove-core` entries (via `cargo update`)
#   4. dist/npm/package.json — the npm wrapper `"version"`
#   5. CHANGELOG.md        — insert a dated `## [X.Y.Z]` stub (skipped if present)
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

say "1/5  cli/Cargo.toml (grove) -> $VERSION"
# Only the first `version = "..."` line (the [package] version).
perl -i -pe 'if (!$done && /^version = "/) { s/^version = ".*"/version = "'"$VERSION"'"/; $done=1 }' cli/Cargo.toml
grep -m1 '^version = ' cli/Cargo.toml

say "2/5  core/Cargo.toml (grove-core) -> $VERSION"
perl -i -pe 'if (!$done && /^version = "/) { s/^version = ".*"/version = "'"$VERSION"'"/; $done=1 }' core/Cargo.toml
grep -m1 '^version = ' core/Cargo.toml

say "3/5  Cargo.lock (grove + grove-core entries)"
cargo update -p grove -p grove-core >/dev/null 2>&1 || cargo generate-lockfile >/dev/null
lock_v="$(awk '/^name = "grove"$/{getline; print; exit}' Cargo.lock)"
echo "Cargo.lock grove $lock_v"
case "$lock_v" in
  *"\"$VERSION\""*) : ;;
  *) echo "error: Cargo.lock grove entry is not $VERSION ($lock_v)" >&2; exit 1 ;;
esac
lock_core_v="$(awk '/^name = "grove-core"$/{getline; print; exit}' Cargo.lock)"
echo "Cargo.lock grove-core $lock_core_v"
case "$lock_core_v" in
  *"\"$VERSION\""*) : ;;
  *) echo "error: Cargo.lock grove-core entry is not $VERSION ($lock_core_v)" >&2; exit 1 ;;
esac

say "4/5  dist/npm/package.json -> $VERSION"
perl -i -pe 's/("version":\s*)"[^"]*"/$1"'"$VERSION"'"/ if !$done && /"version":/ and ($done=1)' dist/npm/package.json
grep -m1 '"version":' dist/npm/package.json

say "5/5  CHANGELOG.md"
if grep -qE "^## \[$VERSION\]" CHANGELOG.md; then
  echo "CHANGELOG already has a [$VERSION] section — leaving it."
else
  today="$(date +%F)"
  # Insert a stub immediately before the first existing release section.
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

Done. Edited: cli/Cargo.toml, core/Cargo.toml, Cargo.lock, dist/npm/package.json, CHANGELOG.md.
Next (see RELEASING.md):
  1. Fill in the CHANGELOG [$VERSION] section.
  2. cargo build --release --locked && cargo test --release --locked
  3. git switch -c release/v$VERSION && git commit -am "release: v$VERSION"
     then PR to main, merge.
  4. git tag -a v$VERSION -m "grove v$VERSION" && git push origin v$VERSION
  5. npm publish (from dist/npm/) + homebrew tap regen, after the Release exists.
EOF
