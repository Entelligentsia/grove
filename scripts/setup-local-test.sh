#!/usr/bin/env bash
# Local testing setup for grove.
#
# Wires a freshly-built grove (with local source changes) into the agent test
# beds so an MCP/agent session exercises *your* binary and grammars, not the
# published 0.1.4. Idempotent — safe to re-run after every change.
#
# What it does:
#   1. cargo build --release
#   2. install that binary over the npm-vendored one the test beds' .mcp.json
#      already point at (so no per-bed config edits are needed)
#   3. regenerate the requested grammars in the OS cache via `grove ingest`
#      (the real shipping pipeline — applies registry-sources.json `extra_tags`,
#      e.g. C's file-scope variable patterns), mirroring a future `grove fetch`
#   4. verify the #25/#26 fixes against ../git if that checkout is present
#
# Usage:  scripts/setup-local-test.sh [lang ...]      (default: c)
set -euo pipefail

cd "$(dirname "$0")/.."
REPO="$PWD"
LANGS=("${@:-c}")

say() { printf '\n\033[1m== %s\033[0m\n' "$1"; }

say "1/4  build release binary"
cargo build --release
BIN="$REPO/target/release/grove"

say "2/4  install into the npm-vendored path the test beds use"
SHIM="$(command -v grove || true)"
if [[ -n "$SHIM" ]]; then
  PKG="$(dirname "$(dirname "$(readlink -f "$SHIM")")")"   # .../node_modules/@entelligentsia/grove
  VENDOR="$PKG/vendor/grove"
  if [[ -e "$VENDOR" ]]; then
    rm -f "$VENDOR"                # unlink first: the running MCP server holds the old inode
    cp "$BIN" "$VENDOR"
    chmod +x "$VENDOR"
    echo "installed -> $VENDOR"
  else
    echo "WARN: vendored binary not found at $VENDOR (skipping; test beds may use a different path)"
  fi
else
  echo "WARN: no global 'grove' on PATH (skipping vendored install; use GROVE_REGISTRY or target/release/grove directly)"
fi

say "3/4  regenerate grammars in the OS cache via the real ingest pipeline"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/grove/grammars"
mkdir -p "$CACHE"
"$BIN" ingest "${LANGS[@]}" --sources "$REPO/registry-sources.json" --out "$CACHE"
echo "cache: $CACHE"

say "4/4  verify against ../git (if present)"
GIT="$REPO/../git"
if [[ -d "$GIT" ]]; then
  echo "-- #25/#26 Bug1: full function body --"
  "$BIN" source "$GIT/git.c" get_builtin | tail -2
  echo "-- #26 Bug2: file-scope dispatch table as a variable --"
  "$BIN" symbols "$GIT" --name commands --kind variable | grep "git.c" || echo "MISSING: commands[] not found"
  echo "-- #2: definition shows the file column --"
  ( cd "$GIT" && "$BIN" definition cmd_commit -d . | head -2 )
else
  echo "../git not present — skipping live verification"
fi

cat <<EOF

Done. The test bed (../git) registers grove via the vendored
binary just updated, and resolves grammars from the OS cache just regenerated.

Next: start a fresh agent session in ../git (so its .mcp.json reloads the new
server) and exercise the grove tools on the C source.

Re-run this script after any source change. Note: \`grove fetch <lang> --force\`
will overwrite the cache with the *published* registry (no curated tags until
the registry is republished) — re-run this script to restore the local build.
EOF
