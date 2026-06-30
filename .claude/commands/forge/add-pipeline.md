---
name: add-pipeline
description: Conversational pipeline manager — add, customize, view, or remove Forge pipelines and their phase commands
---

# /forge:add-pipeline

🌊 **Pipeline Manager** — I guide you through building and rewiring pipelines.

## Setup

```
FORGE_ROOT: !`echo "${CLAUDE_PLUGIN_ROOT:-$(pwd)/.forge}"`
```

Read project config values:
```
PROJECT_PREFIX:       !`node -e "try{console.log(require('./.forge/config.json').project.prefix)}catch{console.log('PROJECT')}"`
CUSTOM_COMMANDS_DIR:  !`node -e "try{console.log(require('./.forge/config.json').paths.customCommands)}catch{console.log('engineering/commands')}"`
```

## Arguments

$ARGUMENTS

Parse for shortcuts:
- `--list` → jump directly to **View mode**
- `--remove <name>` → jump directly to **Remove mode** for that name
- A plain pipeline name with no other flags → jump to **Add/Edit mode** pre-filled with that name

---

## Opening conversation

If no shortcut was triggered, greet the user and ask intent:

> 🌊 **Pipeline Manager**
>
> What would you like to do?
>
> **1.** Add a new pipeline
> **2.** Customize an existing pipeline — override phases, swap commands, change models, rewire
> **3.** View configured pipelines
> **4.** Remove a pipeline
>
> ── Reply with a number, or just describe what you need.

Interpret free-text generously:
- "I want to change the model on the review phase" → mode 2
- "add a pipeline for hotfixes" / "new pipeline" → mode 1
- "show me what's configured" / "list" → mode 3
- "delete X" / "remove X" → mode 4

---

## Mode 1 — Add new pipeline

### Step 1 — Understand the use case

Ask:

> What kind of tasks should this pipeline handle?
> ── Describe the use case in a sentence or two.
> ── Example: "Data science tasks — need a schema validation step before coding begins"

From the description, suggest a pipeline name. The name must be `[a-z0-9_-]`:

> How about **`{suggested-name}`**? Does that work, or would you prefer something else?

Wait for confirmation or a new name.

### Step 2 — Phase-by-phase walkthrough

Display the standard phases as a starting point:

> Here are the standard Forge phases. For each one I'll ask whether to keep it as-is,
> adjust the model, replace it with a different command, skip it, or create a new custom command.
>
> | # | Phase role | Default command | Default model |
> |---|-----------|----------------|---------------|
> | 1 | plan | `plan` | sonnet |
> | 2 | review-plan | `review-plan` | opus |
> | 3 | implement | `implement` | sonnet |
> | 4 | review-code | `review-code` | opus |
> | 5 | validate | `validate` | opus |
> | 6 | approve | `approve` | opus |
> | 7 | commit | `commit` | haiku |
>
> ── Walk me through phase by phase, or describe your whole pipeline at once.

For each phase the user wants to customise, ask what they're trying to achieve before
asking for a command name. Based on their description of intent, suggest the right approach:

| Intent | Suggested approach |
|--------|-------------------|
| Stricter review, same role | Custom command with tighter instructions |
| Faster / cheaper phase | Keep command, lower the model (e.g. sonnet → haiku) |
| Domain-specific validation | New custom command |
| Skip a phase entirely | Remove it from the phase list |
| Different agent persona | New custom command |

For any phase using a custom command — check whether it exists before collecting more input:
```sh
ls {CUSTOM_COMMANDS_DIR}/{name}.md 2>/dev/null || ls .claude/commands/{name}.md 2>/dev/null
```
If it doesn't exist → go to **Custom Command Creation** below, then return here.

### Step 3 — Check for existing pipeline

```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline get {PIPELINE_NAME} 2>/dev/null
```

If output is non-empty, show the existing pipeline and ask:
> △ A pipeline named `{name}` already exists (shown above).
> Replace it, edit it instead, or pick a different name?

If "edit it" → switch to **Mode 2** for this pipeline, pre-filled with current phases.

### Step 4 → Preview and confirm (see Shared section)

---

## Mode 2 — Customize existing pipeline

### Step 1 — Select pipeline

List current pipelines:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" list-pipelines
```

If no pipelines exist:
> △ No pipelines configured yet. Would you like to add one? (Say yes to go to mode 1.)

If only one pipeline exists, select it automatically. Otherwise ask:
> Which pipeline would you like to customize?

### Step 2 — Show current phases

```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline get {PIPELINE_NAME}
```

Display the output as-is. It already renders as a phase table.

### Step 3 — Understand the change

Ask:

> What would you like to change?
>
> - **Override** a phase with a different command
> - **Change model** for one or more phases
> - **Add** a new phase at a specific position
> - **Remove** a phase
> - **Reorder** (rewire) the phases
>
> ── Describe what you need in plain language.

Listen to intent first, then collect specifics. Examples of intent-to-action mapping:

| User says | Action |
|-----------|--------|
| "I want a stricter security review" | Override `review-code` with a custom security-focused command |
| "skip plan review, we move fast" | Remove the `review-plan` phase |
| "use haiku for planning to save cost" | Update `model` on the `plan` phase to `haiku` |
| "add a data validation step before coding" | Insert a new `review-plan`-role phase before `implement` |
| "swap out the architect with my own approval command" | Override `approve` with a custom command |

For model changes — after the user names the phases, ask which model:
> Options: `haiku` (fast/cheap), `sonnet` (balanced), `opus` (thorough)

For phase overrides with a custom command — check if the command exists, then go to
**Custom Command Creation** if not.

For reordering — show the current sequence, ask for the new order, verify no
revision-loop is broken (a `review-*` phase needs a preceding non-review phase to
loop back to).

### Step 4 → Preview and confirm (see Shared section)

---

## Mode 3 — View

```sh
node "$FORGE_ROOT/tools/manage-config.cjs" list-pipelines
```

If no pipelines are configured, say so and offer to add one.

If pipelines exist, offer to drill into one:
> ── To see a pipeline's full phase detail, tell me its name.

For a named pipeline:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline get {NAME}
```

---

## Mode 4 — Remove

If no name was provided, list pipelines and ask which to remove.

Check for tasks referencing this pipeline:
```sh
grep -rl '"pipeline": "{NAME}"' .forge/store/tasks/ 2>/dev/null
```

If any found:
> △ {N} task(s) reference `{NAME}`. They will fall back to the default pipeline at runtime.
> The tasks themselves won't be modified — only the routing changes.
>
> Confirm removal? (yes / no)

On confirmation:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline remove {NAME}
```

---

## Custom Command Creation

When a phase needs a command that doesn't exist yet, guide the user through creating it.

Tell the user:

> The command **`{name}`** doesn't exist yet. I can create it now.
>
> A few questions:
>
> 1. **What should it do?**
>    Describe the behavior — what it evaluates, produces, or enforces.
>
> 2. **What persona should it have?**
>    Describe the agent's character and constraints.
>    Example: "Strict data validator — refuses to approve if schema files are missing"
>
> 3. **What artifact does it produce?**
>    Example: `SCHEMA_REVIEW.md`, `VALIDATION_REPORT.md`

From the answers, determine:
- **Persona symbol** — pick based on the phase role:
  - Plan / implement / commit phases → 🌱
  - Review phases → 🌿
  - Approval / sign-off phases → ⛰️
  - Domain-specific validator → 🌿
  - Flow controller → 🌊
  - Analysis / investigation → 🍂
- **Announcement line** — first-person, present tense, one sentence
- **Next status** — derive from the phase role (e.g. `review-plan` → `plan-approved` or `plan-revision-required`)

Use `.forge/templates/CUSTOM_COMMAND_TEMPLATE.md` as the scaffold. Fill in all
`{PLACEHOLDER}` values from the conversation.

Ensure the directory exists and write the file:
```sh
mkdir -p {CUSTOM_COMMANDS_DIR}
```

Create: `{CUSTOM_COMMANDS_DIR}/{name}.md`

Set the phase's `workflow` field to `"{CUSTOM_COMMANDS_DIR}/{name}.md"` in the pipeline
being built. This allows the orchestrator to read and follow the file directly — the
command name is used for display and manual reference only.

Confirm:
> 〇 Created `{CUSTOM_COMMANDS_DIR}/{name}.md`
> ── Review and edit it before running a task with this pipeline.
> ── The orchestrator will read it directly via the `workflow` field.

Return to the phase walkthrough where you left off.

---

## Shared: Preview and confirm

Before writing, construct the full pipeline JSON and display it clearly:

> 🌊 Here's the pipeline as it will be saved:
>
> **`{name}`** — {description}
>
> | # | Role | Command | Workflow | Model | maxIter |
> |---|------|---------|----------|-------|---------|
> | 1 | plan | `plan` | (built-in) | sonnet | — |
> | 2 | review-plan | `schema-validator` | `engineering/commands/schema-validator.md` | sonnet | 3 |
> ...
>
> ── Save this pipeline? (yes / edit / cancel)

- **yes** → proceed to Write
- **edit** → return to the relevant collection step
- **cancel** → exit without writing, no changes made

---

## Shared: Write

Invoke the tool:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" pipeline add {NAME} --description "{DESCRIPTION}" --phases '{PHASES_JSON}'
```

`PHASES_JSON` is a JSON array where each element includes `command`, `role`, `model`,
and — when a custom command was created — `workflow`. Example:
```json
[
  {"command": "plan", "role": "plan", "model": "sonnet"},
  {"command": "schema-validator", "role": "review-plan", "model": "sonnet",
   "workflow": "engineering/commands/schema-validator.md", "maxIterations": 3},
  {"command": "implement", "role": "implement", "model": "sonnet"},
  {"command": "review-code", "role": "review-code", "model": "opus"},
  {"command": "approve", "role": "approve", "model": "opus"},
  {"command": "commit", "role": "commit", "model": "haiku"}
]
```

On success, print next steps:

```
〇 Pipeline '{NAME}' saved to .forge/config.json.

── Next steps:
   1. If you created custom commands, review them in {CUSTOM_COMMANDS_DIR}/
      and fill in any {PLACEHOLDER} sections before running a task.
   2. Assign this pipeline to a task: set "pipeline": "{NAME}" in the task's
      .forge/store/tasks/{TASK_ID}.json, or let the sprint planner auto-assign it.
   3. Run /forge:rebuild to update the orchestrator's pipeline routing.
```

---

## On error

If any step fails unexpectedly:
> × Something went wrong: {error}
>
> ── This may be a Forge bug. Run `/forge:report-bug` and I'll pre-fill the report.
