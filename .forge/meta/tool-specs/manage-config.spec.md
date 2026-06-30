# Tool Spec: manage-config

## Purpose

Read and write `.forge/config.json` safely. Deterministic — no AI needed.
Preserves key order, indentation, and all fields not explicitly modified.

## Inputs

- `.forge/config.json` — the config file to read or modify
- CLI arguments — subcommand and options (see CLI Interface)

## Outputs

- Modified `.forge/config.json` in-place (write subcommands)
- Printed values or reports to stdout (read subcommands)
- Exit 0 on success, 1 on validation error, 2 on usage error

## CLI Interface

### Read subcommands

```
<tool> manage-config get <key.path>
```
Print the value at the dot-notation path (e.g., `project.prefix`,
`pipelines.measure-conversion`). Exit 1 if the path does not exist.

```
<tool> manage-config list-pipelines
```
Print a table of all pipeline names, their descriptions, and phase counts.
If no `pipelines` key exists, print `── No pipelines configured.` and exit 0.

```
<tool> manage-config pipeline get <name>
```
Print the full detail of a named pipeline as a markdown phase table with
columns `# | Role | Command | Workflow | Model | maxIter`.
Exit 1 with `× Pipeline '{name}' not found` if the name does not exist.

### Write subcommands

```
<tool> manage-config pipeline add <name> --description <text> --phases <json>
```
Add or replace a named pipeline. `--phases` accepts a JSON array string of
phase objects, each with `command` and `role` (required), and optionally
`model`, `maxIterations`, `on_revision`, and `workflow`.

```
<tool> manage-config pipeline remove <name>
```
Remove a named pipeline. Exit 1 if the name does not exist.

```
<tool> manage-config set <key.path> <json-value>
```
Set an arbitrary dot-notation path to a JSON value. Creates intermediate
objects as needed. Intended for first-party Forge tooling only — not
advertised in user-facing help.

## Validation Rules

Applied before any write. Exit 1 and print a clear error if any rule fails.

### Pipeline phases
- Each phase must have `command` (non-empty string) and `role` (string).
- `role` must be one of: `plan`, `review-plan`, `implement`, `review-code`,
  `approve`, `commit`.
- `maxIterations` must be a positive integer when present.
- `workflow` (optional string) — path to a custom workflow file. When present,
  the orchestrator invokes this file directly instead of the built-in workflow
  for the command name. Used for custom phase commands in `engineering/commands/`.
- At least one phase is required per pipeline.

### Pipeline name
- Must be a non-empty string containing only `[a-z0-9_-]`.
- The name `default` is valid and overrides the hardcoded default pipeline.

### File integrity
- If `.forge/config.json` is not valid JSON, exit 1 with a parse error.
  Never overwrite a corrupt file.

## Algorithm

### Read path (`get`)
1. Read and parse `.forge/config.json`.
2. Traverse the dot-notation path.
3. Print the value as JSON (pretty-printed if object/array, bare if scalar).

### Write path (`pipeline add`)
1. Read and parse `.forge/config.json`. Fail fast on parse error.
2. Run validation rules against the incoming phases. Fail fast on error.
3. Ensure `config.pipelines` key exists (create empty object if absent).
4. Merge the new pipeline entry under `config.pipelines[name]`.
5. Serialise back to JSON with the same indentation as the source file
   (detect indent from the first indented line; default to 2 spaces).
6. Write atomically: write to a temp file alongside `config.json`, then
   rename over the original. Never leave a partial write.

### Write path (`pipeline remove`)
1. Read and parse `.forge/config.json`.
2. Check `config.pipelines[name]` exists; exit 1 if not.
3. Delete the key.
4. If `config.pipelines` is now empty, remove the `pipelines` key entirely.
5. Serialise and write atomically (same indent detection as above).

## Error Handling

- Wrap the entire entry point in a top-level exception handler.
- On unexpected errors (file I/O failures, JSON parse errors, unhandled
  exceptions), print a clear one-line message to stderr and exit 1.
- Never let the tool crash with an unhandled exception or stack trace visible
  to the caller — all errors are caught and reported cleanly.
- Python pattern:
  ```python
  if __name__ == "__main__":
      try:
          sys.exit(main())
      except Exception as e:
          print(f"Error: {e}", file=sys.stderr)
          sys.exit(1)
  ```
- JS/TS pattern:
  ```js
  process.on('uncaughtException', (e) => {
      process.stderr.write(`Error: ${e.message}\n`);
      process.exit(1);
  });
  ```

## Formatting Rules

- Preserve the original indentation of the file.
- Do not reorder top-level keys.
- Do not add or remove trailing newlines beyond what was present.
- `list-pipelines` output: markdown pipe table with columns
  `Name | Description | Phases`. Print `(none)` when description is absent.
- `pipeline get` output: optional description line prefixed with `──`, then
  a markdown pipe table with columns `# | Role | Command | Workflow | Model | maxIter`.
  Print `(built-in)` when `workflow` is absent; `—` when `maxIterations` is absent.
- Success messages prefixed with `〇`; errors prefixed with `×`; neutral info with `──`.
