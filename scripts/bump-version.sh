#!/usr/bin/env bash
# Bump grove's version in every place that carries it, in one shot, so no
# source-of-truth is forgotten (see RELEASING.md). Idempotent: re-running with
# the same version is a no-op beyond a lockfile touch.
#
# What it does:
#   1. Cargo.toml          — the [package] `version`
#   2. Cargo.lock          — the `name = "grove"` entry (via `cargo update`)
#   3. dist/npm/package.json — the npm wrapper `"version"`
#   4. CHANGELOG.md        — insert a dated `## [X.Y.Z]` stub (skipped if present)
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

say "1/4  cli/Cargo.toml -> $VERSION"
# Only the first `version = "..."` line (the [package] version).
perl -i -pe 'if (!$done && /^version = "/) { s/^version = ".*"/version = "'"$VERSION"'"/; $done=1 }' cli/Cargo.toml
grep -m1 '^version = ' cli/Cargo.toml

say "2/4  Cargo.lock (grove entry)"
cargo update -p grove >/dev/null 2>&1 || cargo generate-lockfile >/dev/null
lock_v="$(awk '/^name = "grove"$/{getline; print; exit}' Cargo.lock)"
echo "Cargo.lock grove $lock_v"
case "$lock_v" in
  *"\"$VERSION\""*) : ;;
  *) echo "error: Cargo.lock grove entry is not $VERSION ($lock_v)" >&2; exit 1 ;;
esac

say "3/4  dist/npm/package.json -> $VERSION"
perl -i -pe 's/("version":\s*)"[^"]*"/$1"'"$VERSION"'"/ if !$done && /"version":/ and ($done=1)' dist/npm/package.json
grep -m1 '"version":' dist/npm/package.json

say "4/4  CHANGELOG.md"
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

Done. Edited: Cargo.toml, Cargo.lock, dist/npm/package.json, CHANGELOG.md.
Next (see RELEASING.md):
  1. Fill in the CHANGELOG [$VERSION] section.
  2. cargo build --release --locked && cargo test --release --locked
  3. git switch -c release/v$VERSION && git commit -am "release: v$VERSION"
     then PR to main, merge.
  4. git tag -a v$VERSION -m "grove v$VERSION" && git push origin v$VERSION
  5. npm publish (from dist/npm/) + homebrew tap regen, after the Release exists.
EOF
