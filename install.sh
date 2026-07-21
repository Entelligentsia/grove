#!/bin/sh
# grove installer — downloads the right prebuilt binary from the latest
# GitHub Release, verifies its sha256, and installs it.
#
#   curl -fsSL https://raw.githubusercontent.com/Entelligentsia/grove/main/install.sh | sh
#
# Env overrides:
#   GROVE_INSTALL_DIR   where to put the binary   (default: $HOME/.local/bin)
#   GROVE_VERSION       a tag like v0.1.0          (default: latest release)
set -eu

REPO="Entelligentsia/grove"
INSTALL_DIR="${GROVE_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${GROVE_VERSION:-latest}"

err() { printf 'grove-install: %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || err "missing required tool: $1"; }

need uname
need tar
need mkdir
if command -v curl >/dev/null 2>&1; then
  DL="curl -fsSL -o"
elif command -v wget >/dev/null 2>&1; then
  DL="wget -qO"
else
  err "need curl or wget"
fi

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Linux)  os_part="unknown-linux-gnu" ;;
  Darwin) os_part="apple-darwin" ;;
  *)      err "unsupported OS: $os (prebuilts cover Linux and macOS; on others use: cargo install --git https://github.com/$REPO)" ;;
esac
case "$arch" in
  x86_64|amd64)  arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *)             err "unsupported arch: $arch" ;;
esac

target="${arch_part}-${os_part}"
asset="grove-${target}.tar.gz"

if [ "$VERSION" = "latest" ]; then
  base="https://github.com/$REPO/releases/latest/download"
else
  base="https://github.com/$REPO/releases/download/$VERSION"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf 'grove-install: fetching %s (%s)\n' "$asset" "$VERSION" >&2
$DL "$tmp/$asset" "$base/$asset" || err "download failed: $base/$asset"

# Verify checksum when the sidecar is available (and a hasher exists).
if $DL "$tmp/$asset.sha256" "$base/$asset.sha256" 2>/dev/null; then
  expected=$(awk '{print $1}' "$tmp/$asset.sha256")
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$asset" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')
  else
    actual=""
  fi
  if [ -n "$actual" ] && [ "$expected" != "$actual" ]; then
    err "checksum mismatch: expected $expected, got $actual"
  fi
fi

tar -C "$tmp" -xzf "$tmp/$asset" || err "extract failed"
mkdir -p "$INSTALL_DIR"
mv "$tmp/grove" "$INSTALL_DIR/grove"
chmod +x "$INSTALL_DIR/grove"
# Archives from v0.4.1+ also ship grove-explore; older single-binary archives
# should not abort the install.
if [ -f "$tmp/grove-explore" ]; then
  mv "$tmp/grove-explore" "$INSTALL_DIR/grove-explore"
  chmod +x "$INSTALL_DIR/grove-explore"
fi

printf 'grove-install: installed to %s\n' "$INSTALL_DIR" >&2
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) printf 'grove-install: add it to PATH:  export PATH="%s:$PATH"\n' "$INSTALL_DIR" >&2 ;;
esac
"$INSTALL_DIR/grove" --version || true
if [ -f "$INSTALL_DIR/grove-explore" ]; then
  "$INSTALL_DIR/grove-explore" --version || true
fi
