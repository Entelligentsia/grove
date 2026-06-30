---
name: add-task
description: Add a new task to an existing sprint mid-flight — mini intake, sequential ID assignment, store write, and collate
---

# /forge:add-task

Add a task to an existing sprint without re-running the full sprint planner.

## Setup

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

Read project config values:
```
PROJECT_PREFIX:       !`node -e "try{console.log(require('./.forge/config.json').project.prefix)}catch{console.log('PROJECT')}"`
ENGINEERING_PATH:     !`node -e "try{console.log(require('./.forge/config.json').paths.engineering)}catch{console.log('engineering')}"`
STORE_ROOT:           !`node -e "try{console.log(require('./.forge/config.json').paths.store)}catch{console.log('.forge/store')}"`
```

## Arguments

$ARGUMENTS

Parse for shortcuts:
- `--sprint <SPRINT_ID>` → skip sprint selection, use this sprint directly
- `--title <title>` → skip title prompt, use this title directly
- `--estimate <S|M|L|XL>` → skip estimate prompt, use this estimate directly

---

## Step 1 — Sprint Selection

List all sprints in the store:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" list sprint
```

If `--sprint <ID>` was provided:
- Verify the sprint exists by checking the output for the given ID.
- If not found: `× Sprint {ID} not found. Available sprints shown above.`
- If found: use it, skip to Step 2.

If no `--sprint` argument:
- Filter to sprints with status `active` or `planning` (these are the only
  states where adding tasks makes sense).
- If no active/planning sprints exist:
  > △ No active or planning sprints found. Create a sprint first using new-sprint.
  > Exit without making changes.
- If only one active/planning sprint: auto-select it and tell the user:
  > 〇 Auto-selected sprint: **{SPRINT_ID}** — {title}
- If multiple: present a numbered list and ask the user to choose:
  > Which sprint should receive the new task?
  >
  > | # | Sprint | Title | Status | Tasks |
  > |---|--------|-------|--------|-------|
  > | 1 | FORGE-S09 | Calibration, Guardrails, and Bug Closure | active | 9 |
  > | ... |
  >
  > ── Reply with a number.

---

## Step 2 — Mini-Intake Interview

Conduct a focused interview to capture what is needed for immediate implementation.

### Title

If `--title` was provided, use it. Otherwise ask:

> What is the task title?
> ── A short, imperative description (e.g. "Add health-check command")

### Objective

Ask:

> What does this task achieve?
> ── One paragraph: user-facing value or technical goal.

### Acceptance Criteria

Ask:

> What are the acceptance criteria?
> ── List the concrete, testable outcomes that mark this task as done.
> ── Include `node --check` and `validate-store --dry-run` if the task touches `forge/` code.

### Estimate

If `--estimate` was provided, use it. Otherwise ask:

> What is the estimate?
> ── **S** (under 1 hour) / **M** (1-3 hours) / **L** (3-6 hours) / **XL** (6+ hours)

### Pipeline (optional)

Ask:

> Which pipeline should this task use?
> ── Press Enter for **default**, or type a named pipeline (e.g. `hotfix`).

If the user presses Enter or types "default", set pipeline to `"default"`.
If the user provides a name, verify it exists:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline get {NAME} 2>/dev/null
```
If not found:
> △ Pipeline `{NAME}` not found. Using **default**.
Set pipeline to `"default"`.

---

## Step 3 — Assign Task ID

Determine the next sequential task number within the selected sprint.

1. Read the sprint record:
   ```sh
   node "$FORGE_ROOT/tools/store-cli.cjs" read sprint {SPRINT_ID} --json
   ```
2. Parse `taskIds` from the sprint JSON. Each task ID follows the pattern
   `{PREFIX}-S{NN}-T{NN}` (e.g. `FORGE-S09-T07`).
3. Extract the T-numbers (the `NN` part after `T`), find the maximum.
4. The next task number is `max + 1`, zero-padded to 2 digits.
5. Construct the task ID: `{PREFIX}-{SNN}-T{next}` (e.g. `FORGE-S09-T10`).

Example: If `taskIds` contains `FORGE-S09-T01` through `FORGE-S09-T09`,
the next task ID is `FORGE-S09-T10`.

---

## Step 4 — Derive Directory Slug

Convert the task title to a lower-kebab-case slug for the directory name:

1. Lowercase the title.
2. Replace each sequence of non-alphanumeric characters with a single hyphen.
3. Remove leading and trailing hyphens.
4. Truncate to 30 characters.
5. Remove trailing hyphens after truncation.

This follows the `deriveSlug()` algorithm from `seed-store.cjs`.

The task directory name is: `{TASK_ID}-{slug}`
For example: `FORGE-S09-T10-close-bugs`

---

## Step 5 — Create Task Directory and TASK_PROMPT.md

Create the task directory:

```sh
mkdir -p {ENGINEERING_PATH}/sprints/{SPRINT_DIR}/{TASK_DIR}
```

Where:
- `SPRINT_DIR` is the sprint's directory name (from the sprint record's `path`
  field, or derived as `{SPRINT_ID}-{slug}` if no `path` field)
- `TASK_DIR` is `{TASK_ID}-{slug}` from Step 4

Write `TASK_PROMPT.md` using the project's template. Read the template first:

```sh
cat .forge/templates/TASK_PROMPT_TEMPLATE.md
```

Fill in all placeholders:
- `{TASK_ID}` — the assigned task ID
- `{TASK_TITLE}` — the title from the interview
- `{SPRINT_ID}` — the selected sprint ID
- `{S/M/L/XL}` — the estimate
- `default *(or specify a named pipeline)*` — the pipeline choice
- Objective, Acceptance Criteria, Context, Plugin Artifacts, Operational Impact —
  from the interview answers

Write the file:
```
{ENGINEERING_PATH}/sprints/{SPRINT_DIR}/{TASK_DIR}/TASK_PROMPT.md
```

---

## Step 6 — Write Task Record to Store

Construct the task JSON record:

```json
{
  "taskId": "{TASK_ID}",
  "feature_id": null,
  "sprintId": "{SPRINT_ID}",
  "title": "{TITLE}",
  "status": "planned",
  "path": "{ENGINEERING_PATH}/sprints/{SPRINT_DIR}/{TASK_DIR}",
  "estimate": "{S|M|L|XL}",
  "dependencies": [],
  "pipeline": "{PIPELINE}"
}
```

Write it via the store custodian:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" write task '{TASK_JSON}'
```

If the write fails (validation error), report the error and ask the user
to fix the data. Do not proceed until the store accepts the record.

---

## Step 7 — Update Sprint Record

Read the current sprint record:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" read sprint {SPRINT_ID} --json
```

Append the new task ID to the `taskIds` array. Write the updated sprint:

```sh
node "$FORGE_ROOT/tools/store-cli.cjs" write sprint '{UPDATED_SPRINT_JSON}'
```

---

## Step 8 — Collate

Run collate to regenerate views (MASTER_INDEX.md, COST_REPORT.md, etc.):

```sh
node "$FORGE_ROOT/tools/collate.cjs"
```

---

## Step 9 — Confirm

Display a summary (substitute `{PREFIX}` with `PROJECT_PREFIX.toLowerCase()`):

```
〇 Task added successfully!

   ID:         {TASK_ID}
   Sprint:     {SPRINT_ID}
   Directory:  {ENGINEERING_PATH}/sprints/{SPRINT_DIR}/{TASK_DIR}/
   Prompt:     {ENGINEERING_PATH}/sprints/{SPRINT_DIR}/{TASK_DIR}/TASK_PROMPT.md
   Estimate:   {ESTIMATE}
   Pipeline:   {PIPELINE}

── Next steps:
   1. Run /forge:run-task {TASK_ID} to execute the full pipeline.
   2. Or run /forge:plan {TASK_ID} to plan it first.
```

---

## On error

If any step fails unexpectedly, describe what went wrong and ask:

> "This looks like a Forge bug. Would you like to file a report to help improve it? Run `/forge:report-bug` — I'll pre-fill the report from this conversation."