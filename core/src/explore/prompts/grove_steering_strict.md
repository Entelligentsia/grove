## MANDATORY TOOL POLICY — use Grove, not Grep, for symbols

This repository is indexed by **Grove**, a tree-sitter structural-search CLI exposed as
your `Grove` tool. It is byte-precise and returns stable ids `<lang>:<relpath>#<name>@<line>`.

**Hard rules — follow these exactly:**
1. Your FIRST tool call MUST be a `Grove` call, not Grep or Read.
2. To find ANY named symbol (function, struct, type, macro), you MUST use `Grove`, never
   `Grep`. Using `Grep` to hunt for a symbol name is forbidden.
3. Only use `Grep` for plain text that is NOT a symbol (a string literal, a macro's value,
   a TODO). Only use `Read` to confirm a specific line range Grove already gave you.

**Grove recipes (pass args in `command`, without the leading `grove`):**
- Find a symbol by name anywhere: `symbols . --name-contains --name <PART>` (add `--kind function`).
- List a file's definitions without reading it: `outline <FILE>`.
- Print one symbol's exact body: `source <ID>`  (id from symbols/outline).
- Callers of a symbol: `callers <NAME>`.  Definition site: `definition <NAME>`.

Example correct first move for "where is the ort merge and rename detection":
{"command": "symbols . --kind function --name-contains --name merge"}
then {"command": "symbols . --kind function --name-contains --name rename"}
then `source` the ids that matter, and cite `<file>:<line>` from the ids.
