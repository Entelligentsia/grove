---
name: init
description: Bootstrap Forge's LLM half — KB discovery, config generation, and artifact registration. Run once after `4ge init claude .` has scaffolded the project structure.
---

# /forge:init

You are the Forge init orchestrator. Your job is to run the LLM half of the
Forge bootstrap — knowledge-base discovery, config generation, and artifact
registration — in the current working directory. The CLI half (`4ge init claude .`)
has already scaffolded the project structure, installed the wfl-init.js driver,
and installed this command wrapper.

## Resume Detection

Before showing the pre-flight plan, check for an existing checkpoint:

```sh
cat .forge/init-progress.json 2>/dev/null
```

If the file exists and contains valid JSON, inspect it:

- If `lastPhase > 4`, contains a `mode` field, contains a `phase-7-substep-map` key,
  or is missing a `timestamp` field — treat as a stale checkpoint from a previous
  run of the old 12-phase init. Delete the file:
  ```sh
  rm -f .forge/init-progress.json
  ```
  Then proceed to the **Pre-flight interactive prompts** (new 4-phase flow, no mode prompt).
  Set `startPhase = 1`.

- If the file contains valid JSON with `lastPhase` in range 1–4 and a `timestamp`
  field, emit the resume banner:

```
〇 Previous init detected — last completed phase: {lastPhase} of 4

Resume from Phase {nextPhase}? [Y] Start over [n]
```

Use the following mapping:

| lastPhase | Resume from (nextPhase) |
|-----------|-------------------------|
| 1         | Phase 2                 |
| 2         | Phase 3                 |
| 3         | Phase 4                 |

If the user chooses to resume: set `startPhase = nextPhase`.
If the user chooses to start over: delete `.forge/init-progress.json`
(`rm -f .forge/init-progress.json`) and set `startPhase = 1`.

If the file does not exist, or contains invalid JSON, or contains an
unrecognised `lastPhase` value outside 1–4: delete any corrupt file and
set `startPhase = 1`.

If parsing the file throws (malformed JSON): log a one-line warning
`△ init-progress.json is malformed — deleting and starting fresh.`, delete
the file, and set `startPhase = 1`.

## Hero

Render the Forge hero block once per session:

```
forge_banner({ name: "forge" })
forge_banner({ name: "forge", subtitle: "AI SDLC bootstrapper · forge:init" })
```

The hero runs once. If the user resumes mid-init, do NOT re-render the hero —
just emit the phase banner for the resume target phase.

## Flag Handling

`--fast` and `--full` are accepted as no-ops for backwards compatibility with
scripts and CI pipelines. Both flags proceed with the standard 4-phase flow.

**`--migrate` flag:** When `$ARGUMENTS` contains `--migrate`, redirect to the
store migration workflow. Read `.forge/config.json` to locate `paths.forgeRoot`,
then read `{forgeRoot}/meta/workflows/meta-migrate.md` and follow it.
Do NOT proceed to the pre-flight prompts when `--migrate` is present.

## Pre-flight Interactive Prompts

Gather these values before dispatching the workflow:

### a. KB Folder Prompt

```
What should your engineering knowledge base folder be named?
  Default: engineering
  (This is where Forge writes sprint artifacts, docs, and KB files.)

KB folder name [engineering]: ___
```

If the user provides a custom name (non-empty, not "engineering"), write it now:

```
forge_config({ subcommand: "set", key: "paths.engineering", value: "{name}" })
```

Set `kbFolder` to the chosen name (default: `"engineering"`).

### b. CLAUDE.md Offer

Check whether any agent instruction file exists:

```sh
ls CLAUDE.md AGENTS.md CLAUDE.local.md .cursorrules 2>/dev/null
```

If NONE of those files exists, ask:

```
No CLAUDE.md / AGENTS.md found in this project.
Create a minimal CLAUDE.md with Forge KB links? [Y/n]: ___
```

Set `createClaudeMd = true` if user answers Y (or presses Enter); `false` otherwise.
If any of those files already exists, set `createClaudeMd = false` (silently skip the offer).

### c. Timestamp

Mint the ISO timestamp now (before workflow dispatch, in this command wrapper context):

```js
isoTimestamp = new Date().toISOString()
```

## Pre-flight Plan

Before dispatching, emit a summary and wait for confirmation:

```
## Forge Init — <project-name if discoverable, otherwise current directory name>

4 phases will run via the wfl:init workflow:

  1   Collect      — 5 parallel discovery scans → config.json
                     KB folder prompt already answered above
  2   Discover     — KB doc generation (LLM fan-out) + project-context.json
  3   Materialize  — substitute-placeholders.cjs → fully functional workflows
  4   Register     — versioning, manifest, cache, store entries, Tomoshibi

Starting from Phase {startPhase}.

Proceed? [Y/n]: ___
```

If the user declines, stop and advise them to run `/forge:init` again when ready.

## Workflow Dispatch

Execute the workflow:

```
workflow('wfl:init', {
  forgeRoot,
  kbFolder,
  startPhase,
  createClaudeMd,
  isoTimestamp,
  rawArguments: $ARGUMENTS
})
```

Where `forgeRoot` is resolved from `.forge/config.json`'s `paths.forgeRoot` field.

**Workflow-tool-missing error** — if the Workflow tool is unavailable, halt immediately
with this message:

> The Workflow tool is required to run `/forge:init`. This Claude Code build does not
> support the Workflow tool. Upgrade Claude Code and try again.
>
> (Alternatively, run `4ge init claude .` again to re-scaffold, then upgrade Claude Code.)

Do NOT fall back to reading any other document — halt with the above message only.

## Post-Workflow (on success)

When `workflow('wfl:init', ...)` returns with `result.ok === true`:

### Closing Banner

```
forge_banner({ name: "forge" })
forge_banner({ name: "forge", subtitle: "灯 SDLC ready — welcome to your Forge SDLC" })
```

### Welcome Block

Emit the welcome summary:

```
✓ Forge SDLC initialised

Your project is now configured with:
  · Knowledge base in {kbFolder}/
  · Workflows, personas, and skills in .forge/
  · Sprint-ready store in .forge/store/

Next steps:
  · /forge:new-sprint — plan your first sprint
  · /forge:health     — verify store health
  · /forge:plan       — plan a single task
```

### Marketplace Skills Offer

If `result.skillMatches` is present and non-empty, present skill buckets by
confidence (High → Medium → Low). For each bucket, list skill IDs and descriptions.
Ask: `Install recommended skills? [Y/n]` (per bucket or all at once — your call).

For each skill the user accepts:

```
forge_config({ subcommand: "set", key: "installedSkills.{skillId}", value: "true" })
```

Track accepted/skipped counts for the final report. **If the user skips the
entire offer, proceed without blocking.**

### KB Links Refresh

Invoke the `forge:refresh-kb-links` skill via the Skill tool:

```
Skill('forge:refresh-kb-links')
```

If the Skill tool is unavailable or the skill fails, emit:

```
△ Could not auto-refresh KB links — run /forge:refresh-kb-links manually
  to wire your CLAUDE.md to your new KB docs.
```

### Final Report

```
Forge init complete.

  KB docs generated:   {count from result.kbDocCount or "see .forge/"}
  Workflows installed: {count from result.workflowCount or "see .forge/workflows/"}
  Commands installed:  {count from result.commandCount or "see .claude/commands/"}
  Skills accepted:     {acceptedCount}
  Skills skipped:      {skippedCount}

Run `/forge:health` to verify the installation.
```

## Post-Workflow (on failure)

When `workflow('wfl:init', ...)` returns with `result.ok === false`, surface the
failure as structured JSON and offer a bug report:

```
✗ Forge init failed.

Failure details:
{result.failure as formatted JSON}

This looks like a Forge bug. Would you like to file a report?
Run `/forge:report-bug` — I'll pre-fill the report from this conversation.
```

## Arguments

$ARGUMENTS

## On Error

If any step above fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it?
> Run `/forge:report-bug` — I'll pre-fill the report from this conversation."
