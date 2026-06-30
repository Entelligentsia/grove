---
name: status
description: Sprint and task summary — shows current sprint name, task status counts, and recent activity
allowed-tools:
  - Bash
---

# /forge:status

Quick overview of the current sprint and task state.

## Locate plugin root

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

## Locate project config

Check whether `.forge/config.json` exists in the current working directory.
If it does not exist, output:

```
× forge:status — not inside a Forge project. Run /forge:init first.
```

and stop.

## Gather sprint data

Run:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" query --list-sprints --no-excerpts
```

Parse the JSON output. Filter `results` to entries where `status == "active"`.
Sort by `id` descending (most recent sprint first). If no active sprints are found, output:

```
forge:status — No active sprint found.
Run /forge:init to initialise a project, or check your store with /forge:search --list-sprints.
```

and stop.

Use the first (most recent) active sprint as the current sprint.

## Gather task data

Run:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" query --sprint <SPRINT_ID> --no-excerpts
```

From the JSON output, collect all results where `type == "task"`.

## Format and output

Print a markdown status report in this format:

```
## Sprint: <SPRINT_ID> — <sprint.title> (<sprint.status>)

### Tasks (<total> total)
  <status1>: <count>  |  <status2>: <count>  |  ...

### Recent Activity
  - <TASK_ID>: <task.title truncated to 60 chars> → <task.status>
  ...

### Next: /forge:run-task <NEXT_TASK_ID>
```

**Task counts:** Group tasks by status. Show only status buckets with count > 0, separated by `  |  `. Omit zero-count buckets.

**Recent Activity:** Show up to 5 tasks sorted by taskId descending (highest T-number first). Each line:
`  - <task.id>: <title> → <status>`

**Next action:** Find the first task (sorted by taskId ascending) that has status `implementing` or `planned`. Suggest `/forge:run-task <TASK_ID>`. If all tasks are committed or abandoned, output `### Next: Sprint complete — run /forge:retro`.

**Multiple active sprints:** If more than one active sprint exists, add a note after the report:
```
  Note: <N> other active sprint(s) found. Showing most recent (<SPRINT_ID>).
```

## Arguments

$ARGUMENTS

| Argument | Purpose |
|----------|---------|
| `--sprint <ID>` | Show status for a specific sprint instead of the most recent active one |

If `--sprint <ID>` is in `$ARGUMENTS`, use that sprint ID directly (skip the list-sprints step and active-filter). If the sprint is not found, output:
```
× forge:status — sprint <ID> not found. Run /forge:search --list-sprints to see available sprints.
```

## On error

If any step above fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it? Run `/forge:report-bug` — I'll pre-fill the report from this conversation."
