# Tool Spec: generation-manifest

## Purpose

Track and verify the integrity of Forge-generated files via content hashes.
Distinguishes between files that are pristine (unchanged since generation) and
files that have been manually modified. Used by regeneration, migration, and
health checks to avoid silently overwriting user edits.

## Manifest file

Stored at `.forge/generation-manifest.json`. Committed to version control so
the whole team shares the same baseline. Structure:

```json
{
  "files": {
    ".forge/workflows/plan_task.md": {
      "hash": "sha256:a3f9...",
      "generatedAt": "2026-04-06T10:00:00.000Z",
      "generatedByVersion": "0.5.0"
    }
  }
}
```

## Inputs

- `.forge/generation-manifest.json` — the manifest file (read/write)
- CLI arguments — subcommand and target path

## Outputs

- Modified `.forge/generation-manifest.json` in-place (write subcommands)
- Printed status or tables to stdout (read subcommands)
- Exit codes (see per-subcommand below)

## CLI Interface

```
generation-manifest record <path>
```
Hash the file at `<path>` and store/update its entry in the manifest.
Records `hash`, `generatedAt` (ISO timestamp), and `generatedByVersion`
(read from `.forge/config.json → version`, or `"unknown"`).
Exit 0 on success.

```
generation-manifest record-all
```
Re-hash every file currently listed in the manifest. Skips missing files
with a `△ Missing` note. Prints a summary line on completion.
Exit 0 always (missing files are not an error).

```
generation-manifest check <path>
```
Compare the current content hash of `<path>` against the stored hash.

| Exit code | Meaning |
|-----------|---------|
| 0 | Pristine — content matches the stored hash |
| 1 | Modified — content has changed since it was recorded |
| 2 | Untracked — file is not in the manifest |
| 3 | File not found on disk |

Output one status line to stdout regardless of exit code.

```
generation-manifest list [--modified]
```
Print a markdown pipe table of all tracked files with columns
`Status | File | Version | Date`.
With `--modified`: only show modified and missing files.
If all files are pristine, print `〇 All tracked files are pristine.`

```
generation-manifest status
```
Print a compact summary: total tracked count, and counts per status
(pristine / modified / missing). No table.

```
generation-manifest remove <path>
```
Remove a file from the manifest without touching the file itself.
Exit 1 if the file is not tracked.

## Hash algorithm

SHA-256 of normalised content:
1. Normalise line endings to LF (`\r\n` → `\n`)
2. Strip trailing whitespace from each line
3. Hash the resulting UTF-8 string
4. Prefix the hex digest with `"sha256:"`

This prevents false positives from editor-inserted trailing spaces or
CRLF/LF differences across platforms.

## What to track

Record hashes for files that Forge generates and might overwrite on
regeneration, but that users may legitimately customise:

| Category | Pattern |
|----------|---------|
| Workflows | `.forge/workflows/*.md` |
| Templates | `.forge/templates/*.md` |
| Generated commands | `.claude/commands/{forge-generated}.md` |

Do NOT track:
- Knowledge base files (`engineering/architecture/`, `stack-checklist.md`) — expected to evolve via writeback
- Collated views (`MASTER_INDEX.md`, `TIMESHEET.md`) — rebuilt from the store
- Store JSON files (`.forge/store/`) — data, not generated artifacts

## Status symbols

| Symbol | Meaning |
|--------|---------|
| 〇 | Pristine — matches stored hash |
| △ | Modified — diverged from stored hash |
| × | Missing — file not found on disk |

## Error handling

- Wrap entry point in `process.on('uncaughtException')` — all errors print to
  stderr prefixed with `×` and exit 1. No unhandled exceptions or stack traces.
- If `.forge/generation-manifest.json` is missing, treat it as an empty manifest
  (no files tracked) — do not error.
- If `.forge/generation-manifest.json` contains invalid JSON, exit 1 with a
  parse error message.
- Write atomically: temp file + rename. Never leave a partial write.

## Formatting rules

- `list` output: markdown pipe table, status column shows `{symbol} {status}`
- `check` output: one line to stdout: `{symbol} {relpath}: {status-description}`
- `status` output: one line per non-zero count, prefixed with symbol
- Success messages prefixed with `〇`; warnings with `△`; errors with `×`
