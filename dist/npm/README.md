# @entelligentsia/grove

**grove** gives coding agents structural, byte-precise, token-cheap access to a
codebase via tree-sitter, instead of reading whole files. One engine, six tools,
two faces — a human CLI (`grove <verb>`) and an MCP server (`grove serve`).

This npm package is a thin installer: on install it downloads the prebuilt
`grove` binary for your platform from the
[GitHub Release](https://github.com/Entelligentsia/grove/releases) matching this
package's version, verifies its `sha256`, and puts `grove` on your PATH. The
binary that runs is the same native build shipped to every channel.

## Install

```sh
npm install -g @entelligentsia/grove
grove --version
```

Supported prebuilt targets: macOS and Linux (x86_64 + aarch64), Windows
(x86_64). Other platforms: install from source with
`cargo install --git https://github.com/Entelligentsia/grove`.

Requires network access and `tar` at install time (present on macOS, Linux, and
Windows 10+).

## Use

```sh
grove init                  # wire grove into a project (.mcp.json + CLAUDE.md + lock)
grove outline src/app.ts    # compact definition skeleton of a file
grove symbols src           # find symbols across a directory
grove serve                 # run the MCP server over stdio (for coding agents)
```

`grove init` detects the project's languages and auto-fetches the tree-sitter
grammars it needs. Run `grove --help` for the full tool surface.

## Links

- Repository & docs: <https://github.com/Entelligentsia/grove>
- Other install methods (curl | sh, Homebrew, cargo): see the repo README.

MIT licensed.
