# Install

grove ships as a single static binary (Linux, macOS, Windows; x86_64 + aarch64
where applicable). No grammar is compiled in — they load at runtime from the
[hosted registry](languages.md).

## curl | sh

Detects your platform, verifies the sha256, installs to `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/Entelligentsia/grove/main/install.sh | sh
```

## Homebrew (macOS / Linux)

```bash
brew install Entelligentsia/grove/grove
```

Tap: [`Entelligentsia/homebrew-grove`](https://github.com/Entelligentsia/homebrew-grove).

## npm

Provides the `grove` binary via a downloaded, checksum-verified prebuilt:

```bash
npm install -g @entelligentsia/grove
```

## From source

No published crate — install straight from git:

```bash
cargo install --git https://github.com/Entelligentsia/grove
```

Build from a checkout:

```bash
cargo build --release      # first build compiles wasmtime (~30s), then incremental
# binary at target/release/grove
```

## As an agent skill (cross-harness)

The skill works across 70+ harnesses (Claude Code, Cursor, Codex, Cline, …) via
the [agent-skills tool](https://github.com/vercel-labs/skills):

```bash
npx skills add Entelligentsia/grove
```

The skill steers your agent to grove's MCP tools when present, else the `grove`
CLI — and **self-installs the binary on first use** if it's missing (`npm i -g
@entelligentsia/grove`, then `grove init --as skill` to fetch grammars). So this
one command is enough to get grove working in a fresh repo. See
[Setup](setup.md) for `grove init --as mcp|skill|both`.

## Prebuilt binaries

Attached to each [GitHub Release](https://github.com/Entelligentsia/grove/releases).

---

Next: [Setup](setup.md) · [Languages & grammars](languages.md)