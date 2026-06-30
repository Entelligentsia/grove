# Phase 1 — Collect

**Deliverable:** `.forge/config.json` with all required fields.

Set `$FORGE_ROOT` to the forge plugin directory (the parent of this file's
parent directory — the folder containing `meta/` and `init/`).

---

## Pre-flight — Knowledge Base Folder

> **Orchestrator note:** If your orchestrator (e.g. the `wfl:init` driver or the CLI-first
> wrapper) has already supplied a `kbFolder` value, use it directly — skip this prompt.
> This interactive block applies only to unmediated orchestrator-free execution.

Before Phase 1 begins, ask the user where to create the knowledge base folder:

```
Forge will create a folder for architecture docs, sprints, bugs, and features.
Default name: engineering/

Does "engineering" conflict with an existing folder in this project? [n/Y]
If yes, enter your preferred name (e.g. ai-docs, .forge-kb, docs/ai): ___
```

- If the user accepts the default (types `n` or presses Enter): no config write needed.
  `paths.engineering` defaults to `"engineering"` in the schema.
- If the user provides a custom name: write it immediately:
  ```sh
  node "$FORGE_ROOT/tools/manage-config.cjs" set paths.engineering "{chosen_name}"
  ```
  Note: folder name must not contain spaces.

After this question (and any config write), resolve `KB_PATH` for use in all
subsequent phases:

```sh
KB_PATH=$(node -e "try{console.log(require('./.forge/config.json').paths.engineering)}catch{console.log('engineering')}")
```

---

## Steps (follow in order)

### Step 1 — Render phase header

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 1 4 "Collect" north
```

### Step 2 — Discovery

Read each discovery prompt file below, then analyse the project codebase for
that topic. Summarise your structured findings for each topic before writing
anything to disk.

| Prompt | File |
|--------|------|
| Stack | `$FORGE_ROOT/init/discovery/discover-stack.md` |
| Processes | `$FORGE_ROOT/init/discovery/discover-processes.md` |
| Database | `$FORGE_ROOT/init/discovery/discover-database.md` |
| Routing | `$FORGE_ROOT/init/discovery/discover-routing.md` |
| Testing | `$FORGE_ROOT/init/discovery/discover-testing.md` |

Complete all 5 discovery reads before proceeding to Step 3.

### Step 3 — Write config.json

Using your discovery findings, write `.forge/config.json` with this structure:

```json
{
  "version": "1",
  "project": { "name": "<project name>", "prefix": "<UPPERCASE_ABBREV>" },
  "stack": { "primary": [...], "test": "<framework>", "build": "<tool>", "lint": "<tool>" },
  "commands": { "test": "<test cmd>", "build": "<build cmd>", "lint": "<lint cmd>" },
  "paths": {
    "engineering": "engineering",
    "store": ".forge/store",
    "workflows": ".forge/workflows",
    "commands": ".claude/commands/forge",
    "templates": ".forge/templates"
  }
}
```

`paths.commands` is ALWAYS `".claude/commands/forge"` — the command namespace is fixed (CLI-first redesign); it does NOT derive from the project prefix.

Write using:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" set <key> <value>
```
Or write `.forge/config.json` directly as valid JSON.

Write `mode` as `"full"` unconditionally:
```sh
node "$FORGE_ROOT/tools/manage-config.cjs" set mode "full"
```

### Step 4 — Self-check

Read back `.forge/config.json` and confirm all 8 required keys are present
and non-empty: `version`, `project.name`, `project.prefix`, `stack`,
`commands`, `paths.engineering`, `paths.store`, `paths.workflows`.

If any key is missing or empty, fix it now before proceeding.

### Step 5 — Marketplace Skills

> **Orchestrator note:** If your orchestrator handles the skills install offer post-workflow
> (e.g. the `init.md` wrapper does this after `wfl:init` returns), skip this step. Return
> `{ matches, alreadyInstalled }` from the config-writer agent instead.

Read `$FORGE_ROOT/meta/skill-recommendations.md` for the full mapping.

Using the stack discovered above, look up matching skills from the recommendation
mapping. For each match:

1. Run `node "$FORGE_ROOT/tools/list-skills.js"` to get all currently installed skill names.
2. Split matching skills into two buckets: Already installed / Not yet installed.
3. Group not-yet-installed matches by confidence: High, Medium, Low.
4. Present to the user for optional installation.
5. Write `"installedSkills"` to `.forge/config.json` as the union of installed skills.
6. Track skipped skills for the Report.

**If the user skips or installs none:** proceed without blocking.

Write `.forge/init-progress.json`:
```json
{ "lastPhase": 1, "timestamp": "<current ISO timestamp>" }
```

---

## Verify Phase 1

After writing `init-progress.json`, verify the deliverable:

```sh
node "$FORGE_ROOT/tools/verify-phase.cjs" --phase 1
```

- **Exit 0:** Phase 1 complete. Proceed to Phase 2.
- **Exit 1:** Read the JSON output to identify missing fields. Fix them and
  re-run verify once. If it still fails, halt and surface the JSON error to
  the user.
