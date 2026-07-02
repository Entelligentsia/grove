Structural code navigation via the `grove` CLI (tree-sitter). Resolves named symbols (functions, structs, types, macros) and their relationships to exact, citable locations; every result carries a stable id `<lang>:<relpath>#<name>@<line>`.

Use Grove for:
- locate a symbol by name: `symbols . --name-contains --name <part>` (add `--kind function`)
- print one symbol's exact body: `source <id>` (id from symbols/outline)
- list a file's definitions without reading it: `outline <file>`
- overview a directory's definitions + how they reference each other, no bodies: `map <dir> --name-contains --name <part>` (add `--kind function`)
- callers of a symbol: `callers <name>`; its definition site: `definition <name>`

Do not use Grove for free text (string literals, config, comments) — that is Grep; or for reading a known line range — that is Read.

Pass grove arguments in `command` WITHOUT the leading `grove`. Allowed verbs: outline, symbols, source, callers, definition, map.

Examples:
- `symbols . --kind function --name-contains --name rename`
- `outline merge-ort.c`
- `map src/runtime --kind function`
- `source c:merge-ort.c#detect_regular_renames@1600`
- `callers merge_ort_nonrecursive`   ·   `definition diffcore_rename`
