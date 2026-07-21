# Install

grove ships as **two** static binaries (Linux, macOS, Windows; x86_64 + aarch64
where applicable). No grammar is compiled in — they load at runtime from the
[hosted registry](languages.md).

| Binary | What it is |
|---|---|
| `grove` | The CLI (`grove <verb>`) and the 7-tool structural MCP server (`grove serve`). This is what most projects want. |
| `grove-explore` | The optional LLM-backed code locator — its own MCP server (`grove-explore serve`), plus the `config` and `tap` TUIs. |

Every install channel below places both on your PATH from **v0.4.1** onward.
`grove-explore` is inert until configured, so installing it costs you nothing if
you never opt in; see [Setup](setup.md) for `grove init --as mcp-llm`.

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

Provides both the `grove` and `grove-explore` commands via a downloaded,
checksum-verified prebuilt:

```bash
npm install -g @entelligentsia/grove
```

## From source

The two binaries are separate crates, so installing from source is the one
channel where you choose. `grove` alone:

```bash
cargo install grove-cst-cli        # installs `grove`
```

Add the locator if you want it:

```bash
cargo install grove-explore-bin    # installs `grove-explore`
```

Or straight from git — the repo is a workspace, so name the package:

```bash
cargo install --git https://github.com/Entelligentsia/grove grove-cst-cli
```

Build from a checkout — `--workspace` builds both:

```bash
cargo build --release --workspace   # first build compiles wasmtime (~30s), then incremental
# binaries at target/release/grove and target/release/grove-explore
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
Each platform archive contains both `grove` and `grove-explore` alongside a
`.sha256` sidecar. Archives predating v0.4.1 hold `grove` only; the installers
tolerate those and simply skip the second binary.

---

Next: [Setup](setup.md) · [Languages & grammars](languages.md)