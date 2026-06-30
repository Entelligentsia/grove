---
name: refresh-kb-links
description: Refresh Forge KB and workflow links in agent instruction files (CLAUDE.md, AGENTS.md, etc.)
---

# forge:refresh-kb-links

Scan every coding-agent instruction file in the project and ensure each has
up-to-date links to the Forge knowledge base and generated workflow entry points.
This skill is invoked by `/forge:refresh-kb-links` or conversationally via `/forge:ask`.

## Setup

Read the Forge config to determine the KB root path:

```sh
KB_PATH: !`node -e "try{console.log(require('./.forge/config.json').paths.engineering)}catch{console.log('engineering')}"`
```

## Known Agent Instruction Files

Detect which of these files exist in the project root:

| File | Coding tool |
|------|------------|
| `CLAUDE.md` | Claude Code |
| `AGENTS.md` | OpenAI Codex / generic |
| `AGENT.md` | generic |
| `.github/copilot-instructions.md` | GitHub Copilot |
| `.cursorrules` | Cursor (legacy) |
| `.cursor/rules/*.mdc` | Cursor (current) |
| `GEMINI.md` | Google Gemini |

For each file found, check whether it already contains managed Forge sections.

## Managed Section Markers

Each file may have two independent managed sections:

### KB links section

Open marker (prefix match — tolerates minor text variations):
```
<!-- forge-kb-links
```

Close marker (exact):
```
<!-- /forge-kb-links -->
```

### Workflow links section

Open marker (prefix match):
```
<!-- forge-workflow-links
```

Close marker (exact):
```
<!-- /forge-workflow-links -->
```

## Content Rules

### KB section content

Only include rows for files that actually exist on disk:

- `{KB_PATH}/MASTER_INDEX.md` → "All sprints, tasks, bugs, and features"
- `{KB_PATH}/architecture/INDEX.md` → "Stack, processes, database, routing, deployment"
- `{KB_PATH}/business-domain/INDEX.md` → "Entity model and domain concepts"

```markdown
<!-- forge-kb-links: managed by Forge — do not edit manually -->
## Forge Knowledge Base

| Index | Contents |
|-------|----------|
| [MASTER_INDEX]({KB_PATH}/MASTER_INDEX.md) | All sprints, tasks, bugs, and features |
| [Architecture]({KB_PATH}/architecture/INDEX.md) | Stack, processes, database, routing, deployment |
| [Business Domain]({KB_PATH}/business-domain/INDEX.md) | Entity model and domain concepts |

Personas live in `.forge/personas/`.
<!-- /forge-kb-links -->
```

### Workflow section content

Only include rows for workflow files that actually exist on disk. Check each:

Atomic workflows (LLM-generated markdown in `.forge/workflows/`):

- `.forge/workflows/plan_task.md` → "Research codebase → implementation plan"
- `.forge/workflows/implement_plan.md` → "Execute approved plan → code changes"
- `.forge/workflows/architect_sprint_plan.md` → "Sprint planning and task decomposition"
- `.forge/workflows/architect_sprint_intake.md` → "Sprint intake and requirements elicitation"

Orchestration is deterministic JS, not prose. The LLM orchestration workflows
(`orchestrate_task` / `run_sprint` / `fix_bug`) are **retired** — link the JS
drivers in `.claude/workflows/` instead (only include rows where the file exists):

- `.claude/workflows/wfl-run-task.js` → "Full task pipeline (plan → implement → review → approve → commit)"
- `.claude/workflows/wfl-run-sprint.js` → "Full sprint orchestration"
- `.claude/workflows/wfl-fix-bug.js` → "Triage → fix → verify"

```markdown
<!-- forge-workflow-links: managed by Forge — do not edit manually -->
## Forge Workflows

| Workflow | Purpose |
|----------|---------|
| [Plan](.forge/workflows/plan_task.md) | Research codebase → implementation plan |
| [Implement](.forge/workflows/implement_plan.md) | Execute approved plan → code changes |
| [Run task](.claude/workflows/wfl-run-task.js) | Full task pipeline (plan → implement → review → approve → commit) |
| [Run sprint](.claude/workflows/wfl-run-sprint.js) | Full sprint orchestration |
| [Fix bug](.claude/workflows/wfl-fix-bug.js) | Triage → fix → verify |
<!-- /forge-workflow-links -->
```

(Only include rows where the referenced file exists on disk.)

## KB Integrity Check

Before scanning agent instruction files, verify that the knowledge graph itself is complete.
The collate tool generates INDEX.md files at every node of the graph; if they are missing,
links in MASTER_INDEX.md are broken and agents navigate to dead ends.

Check the following, using `KB_PATH` as the root:

1. **Sprint INDEX files** — for every directory under `{KB_PATH}/sprints/`, check that
   `{KB_PATH}/sprints/{sprint-dir}/INDEX.md` exists.

2. **Task INDEX files** — for every subdirectory under `{KB_PATH}/sprints/{sprint-dir}/`
   that looks like a task folder (contains at least one `.md` file), check that its
   `INDEX.md` exists.

3. **Bug INDEX files** — for every directory under `{KB_PATH}/bugs/`, check that
   `{KB_PATH}/bugs/{bug-dir}/INDEX.md` exists.

If any INDEX files are missing, prepend this warning to the output (before the agent-file
approval prompt):

```
⚠️  KB knowledge graph has broken links — INDEX.md files are missing:

  △ {KB_PATH}/sprints/PROJ-S01/INDEX.md         — missing
  △ {KB_PATH}/sprints/PROJ-S01/PROJ-S01-T01/INDEX.md — missing
  △ {KB_PATH}/bugs/PROJ-B01-some-bug/INDEX.md   — missing

Run collate to regenerate them:
  node "$FORGE_ROOT/tools/collate.cjs"
```

If all INDEX files are present (or there are no sprint/task/bug folders yet), skip this
block entirely. Do not warn when the KB is intact.

## Staleness Check

For each detected agent instruction file, check both sections:

1. If the section marker is absent → section is **missing**
2. If the section markers are present → extract the current content between them.
   Compare current content vs. the content you would write (with current KB_PATH
   and only existing files). If they differ → section is **stale**. If identical → **current**.

## Approval Prompt

After scanning all files, present a single consolidated approval prompt:

```
🏮 forge:refresh-kb-links — KB & Workflow Visibility

Forge has generated a knowledge base and SDLC workflows for this project. Without links
to these in your agent instruction files, every new conversation starts blind — no KB
context, no workflow playbook.

Found agent instruction files:
  〇 CLAUDE.md        — no Forge KB links, no workflow links
  〇 AGENTS.md        — no Forge KB links, no workflow links

Add ## Forge Knowledge Base and ## Forge Workflows sections to each? [Y/n]
(or choose individually: [c])
```

Adapt the status lines to reflect the actual state:
- `〇 {filename}        — no Forge KB links, no workflow links` (both missing)
- `〇 {filename}        — KB links stale` (KB section present but stale)
- `〇 {filename}        — workflow links stale` (workflow section present but stale)
- `〇 {filename}        — KB links stale, workflow links stale` (both stale)
- `〇 {filename}        — links current, no changes needed` (both present and current — skip)

**If all detected files already have current links:** output:

```
🏮 forge:refresh-kb-links — all KB and workflow links are current. No changes needed.
```

And return without prompting.

## Writing Sections

On approval ([Y] or per-file confirm):

- **Missing section:** append the section to the end of the file.
- **Stale section:** replace content between the open and close markers with fresh content.
  Preserve everything outside the markers exactly.

Write the `{KB_PATH}` substitution with the actual resolved path value (not the literal
string `{KB_PATH}`). The resulting markdown should have real paths like `engineering/MASTER_INDEX.md`
or `ai-docs/MASTER_INDEX.md`.

## Idempotency

On a second run where all links are already current, output the "all links current" message
and return immediately. Do not re-write unchanged sections.

## Rename Instructions

If the user later renames the KB folder:

1. Rename the folder on disk.
2. Run: `node "$FORGE_ROOT/tools/manage-config.cjs" set paths.engineering <new-name>`
3. Re-run `forge:refresh-kb-links` to refresh the links in all agent instruction files.
