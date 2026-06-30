---
requirements:
  reasoning: High
  context: High
  speed: Low
audience: orchestrator-only
deps:
  personas: [engineer]
  skills: [engineer, generic]
  templates: []
  sub_workflows: []
  kb_docs: []
  config_fields: [paths.engineering]
---

# Meta-Workflow: Structural Migration (v0.x → v0.40)

## Purpose

Translate a prose-heavy v0.x `.forge/` instance into the v0.40 format:
`project-context.json` (T02 schema) + clean base-pack substitution (T03) +
`structure-versions.json` snapshot tracking (T05). Every operation is
reversible, guarded by user confirmation, and idempotent on re-run.

**Trigger:** Invoked by `/forge:init --migrate` when the user passes `--structural`
or when `.forge/structure-versions.json` is absent (pre-T05 install detected).

**Scope:** v0.x → v0.40 only. Installs older than v0.x should use
`/forge:health` → `/forge:update` to reach a supported baseline first.

---

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance -->
## Iron Laws

- Migration operations are reversible and user-confirmed before any destructive writes. Do not skip the Phase 2 confirmation gate — proceed only after the user explicitly accepts the migration plan.
- Read `.forge/personas/engineer.md` first; print the persona identity line (emoji, name, tagline) to stdout before any other tool use.
- All store I/O via `forge_store`. Never edit `.forge/store/*.json` directly.

## Pre-conditions

- `/forge:init` has run: `.forge/config.json` exists and is readable.
- `.forge/structure-versions.json` is absent OR `--structural` was passed.
- The Forge plugin root is resolvable. Migration reads plugin source
  (`$FORGE_ROOT/init/base-pack`, `$FORGE_ROOT/.claude-plugin/plugin.json`)
  that is not vendored into `.forge/`, so resolve the active plugin install
  root from `CLAUDE_PLUGIN_ROOT` (this workflow runs plugin-side):
  ```sh
  export FORGE_ROOT="${CLAUDE_PLUGIN_ROOT}"
  ```

---

## Algorithm

### Phase 0 — Pre-flight

**0a. Verify init has completed.**

Check that `.forge/config.json` exists. If it does not, stop:

> "Forge has not been initialised in this project. Run `/forge:init` first."

Resolve `FORGE_ROOT` from `${CLAUDE_PLUGIN_ROOT}` (the active plugin install root).

**0b. Detect install generation.**

```sh
ls .forge/structure-versions.json 2>/dev/null && echo "EXISTS" || echo "ABSENT"
```

- If `structure-versions.json` EXISTS: this is a post-T05 install. Warn the
  user that structural migration is not needed and offer to run the standard
  store-schema migration (Steps 1–7 of `/forge:init --migrate`) instead. Stop.
- If ABSENT: proceed (pre-T05 install confirmed).

**0c. Check for in-progress sentinel (idempotency).**

```sh
ls .forge/archive/pre-migration/.migration-in-progress 2>/dev/null && echo "FOUND" || echo "NOT_FOUND"
```

If FOUND:

- Check if a draft `project-context.json` already exists AND is valid JSON:
  ```sh
  node -e "JSON.parse(require('fs').readFileSync('.forge/project-context.json','utf8')); console.log('VALID');" 2>/dev/null || echo "INVALID_OR_ABSENT"
  ```
- Resume heuristic: sentinel exists AND `.forge/project-context.json` exists AND
  validates against the schema → offer resume (ask user: "A previous migration
  was interrupted. A draft project-context.json already exists. Resume from
  Phase 2 (confirm and write)? [Y/n/restart]"). If user chooses restart, remove
  the sentinel file and continue as a fresh run.
- Otherwise (no valid draft): offer fresh restart.

**0d. Create sentinel directory FIRST, then write sentinel file.**

ORDER IS MANDATORY — writing the file before the directory fails:

```sh
mkdir -p .forge/archive/pre-migration/
echo "in-progress" > .forge/archive/pre-migration/.migration-in-progress
```

Record the migration start timestamp:
```sh
MIGRATION_START=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
```

---

### Phase 1 — Read and Extract

Read all existing `.forge/` artifacts in five categories and extract
project-specific content. Generic boilerplate lines (role definitions,
algorithm steps, universal Iron Laws) are skipped.

**Security note:** All extracted strings are copied verbatim into
`project-context.json` field values. They are never executed or evaluated.
Downstream T03 substitution (`substitute-placeholders.cjs`) treats all values
as opaque text strings and only replaces the canonical `REQUIRED_KEYS` +
`RUNTIME_PASSTHROUGH_KEYS` placeholder set. No arbitrary code paths are opened
by the extracted content.

#### Extraction categories

| Category | Files | Extraction targets |
|---|---|---|
| Config | `.forge/config.json` | All fields — direct mapping (highest confidence) |
| Personas | `.forge/personas/*.md` | Project name, prefix, KB path, impact categories, entity refs, skill directives |
| Skills | `.forge/skills/*.md` (esp. engineer, architect) | Stack summary, key directories, commands, technical debt, verification commands |
| Workflows | `.forge/workflows/*.md` | Project prefix occurrences, task ID format, branching convention, deployment environments |
| Templates | `.forge/templates/*.md` | Hardcoded project-specific values (task ID format, prefix, paths) |

#### Extraction rules

- **Config fields** → direct mapping to `project-context.json` (highest confidence).
- **`PROJECT_NAME`, `PREFIX` patterns** in persona/skill files → extract by pattern.
- **`{{KEY}}` tokens that survived substitution** → attempt to fill from KB; if
  not resolvable, flag for user review.
- **Specific project paths, entity names, stack references** → extract with moderate confidence.
- **Generic boilerplate** (role definitions, algorithm steps, universal rules) → skip.

#### Target `project-context.json` fields (19 string-leaf fields)

These are the 19 scalar string leaves of `project-context.schema.json` that
count toward the ≥80% (≥15 of 19) population threshold:

1. `project.name`
2. `project.prefix`
3. `project.description`
4. `project.commands.test`
5. `project.commands.build`
6. `project.commands.deploy`
7. `architecture.frameworks.backend`
8. `architecture.frameworks.frontend`
9. `architecture.frameworks.database`
10. `architecture.dataAccess`
11. `architecture.deployment`
12. `conventions.branching`
13. `conventions.taskIdFormat`
14. `deployment.impactNotes`
15. `verification.typeCheck`
16. `verification.lint`
17. `verification.test`
18. `verification.build`
19. `verification.infraBuild`

#### No-data-loss guarantee

Any project-specific fact that cannot be confidently mapped to a
`project-context.json` field MUST be written to
`.forge/archive/pre-migration/MIGRATION_NOTES.md` for manual review. It must
never be silently dropped.

Format for each item in `MIGRATION_NOTES.md`:
```
## Unresolved: <short description>
Source: <file>:<line range>
Extracted: <verbatim text>
Suggested mapping: <field name or "none — needs manual review">
```

Also document in `MIGRATION_NOTES.md`:
- The old plugin version detection result: if a `version` field was found in any
  `.forge/store/sprints/*.json` record, record it as
  `"derived-from-sprint-updated-<date>"`. Otherwise record `"unknown-pre-v0.40"`.
  This provides an audit trace for future reviews.
- Note: the plugin version in `$FORGE_ROOT/.claude-plugin/plugin.json` reflects
  the *new* installed plugin version and is NOT used to determine the pre-migration
  version.

Produce a draft `project-context.json` covering all extractable T02 fields.
Fields that cannot be confidently populated are left at their schema defaults.

---

### Phase 2 — Synthesise and Show (Confirmation Gate)

Display the synthesised `project-context.json` to the user. Show:

1. The full JSON content (or a diff from schema defaults if large).
2. A summary line:
   ```
   String fields populated: N / 19
   Items needing review:    K (see MIGRATION_NOTES.md)
   ```
3. The archive plan:
   ```
   Files to archive: N (to .forge/archive/pre-migration/)
   ```
4. The substitution plan:
   ```
   Files to write: N (new .forge/personas/, .forge/skills/, .forge/workflows/, .forge/templates/)
   ```

Prompt for explicit confirmation:
```
Migration plan ready.
  String fields populated: N / 19
  Items needing review: K (see MIGRATION_NOTES.md)
  Files to archive: N
  Files to write: N

Proceed with migration? [Y/n]
```

**If user declines:**
```sh
rm .forge/archive/pre-migration/.migration-in-progress
# Remove sentinel dir only if it is now empty
rmdir .forge/archive/pre-migration/ 2>/dev/null || true
```
Exit cleanly with the message: "Migration cancelled. No changes made."

**If user accepts:** proceed to Phase 3.

---

### Phase 3 — Write (archive → substitute → register)

#### Step 3a — Archive

Note: `.forge/archive/pre-migration/` was already created in Phase 0 (sentinel
setup). The `mkdir -p` below is idempotent.

```sh
mkdir -p .forge/archive/pre-migration/
```

Copy every file from `.forge/` (excluding `.forge/store/` and `.forge/archive/`
themselves) to `.forge/archive/pre-migration/`. Preserve directory structure.
Example:
```sh
# List files to archive (excluding store/ and archive/ subtrees)
find .forge/ -not -path '.forge/store/*' -not -path '.forge/archive/*' -type f
```

For each file found, copy it to `.forge/archive/pre-migration/<relative-path>`,
creating intermediate directories as needed.

Write an md5 manifest:
```sh
find .forge/archive/pre-migration/ -not -name 'MANIFEST.md5' -not -name '.migration-in-progress' -type f \
  | sort | xargs md5sum > .forge/archive/pre-migration/MANIFEST.md5
```

Verify manifest completeness: re-read the directory list and confirm every
archived file (excluding `MANIFEST.md5` itself and `.migration-in-progress`)
has an entry in `MANIFEST.md5`. If any file is missing, add it and report the
discrepancy to the user.

#### Step 3b — Write `project-context.json`

Write `.forge/project-context.json` with the synthesised content.

Validate via the store tool:
```sh
forge_validate_store({ dryRun: true })
```

If this exits non-zero, report the validation errors to the user and HALT. Do
NOT remove the sentinel — the user can fix the issue and re-run.

#### Step 3c — Substitute (T03)

```sh
export FORGE_ROOT
node .forge/tools/substitute-placeholders.cjs \
  --forge-root "$FORGE_ROOT" \
  --base-pack  "$FORGE_ROOT/init/base-pack" \
  --config     ".forge/config.json" \
  --context    ".forge/project-context.json" \
  --out        "."
```

This overwrites `.forge/personas/`, `.forge/skills/`, `.forge/workflows/`,
`.forge/templates/`, and `.claude/commands/forge/` with the freshly substituted
base-pack.

If `substitute-placeholders.cjs` exits non-zero: halt, report the error to the
user. Do NOT remove the sentinel (preserves ability to re-run after fixing).

#### Step 3d — Register snapshot (T05)

```sh
node .forge/tools/manage-versions.cjs init
```

`manage-versions init` is idempotent. Since `.forge/structure-versions.json`
does not exist yet, this writes snapshot 0 with `source: "base-pack"` and
`createdAt` set to the current time.

After `init` completes, read the written file and patch snapshot 0:

1. Set `source` → `"migration-from-v{old}"` where `{old}` is the old plugin
   version determined in Phase 1 (defaults to `"unknown-pre-v0.40"`).
   The `source` field is `type: string` with no enum constraint — this free-form
   value is valid per schema.
2. Set `createdAt` of snapshot 0 → `$MIGRATION_START` (the timestamp recorded in
   Phase 0d). This ensures snapshot 0 reflects the conceptual moment of
   migration, not the `manage-versions init` run time.

Example patch (using Node.js):
```sh
node -e "
const fs = require('fs');
const sv = JSON.parse(fs.readFileSync('.forge/structure-versions.json','utf8'));
if (sv.snapshots && sv.snapshots.length > 0) {
  sv.snapshots[0].source = 'migration-from-v{old}';
  sv.snapshots[0].createdAt = process.env.MIGRATION_START;
}
fs.writeFileSync('.forge/structure-versions.json', JSON.stringify(sv, null, 2) + '\n');
console.log('Patched snapshot 0');
" MIGRATION_START="$MIGRATION_START"
```

#### Step 3e — Cleanup sentinel

```sh
rm .forge/archive/pre-migration/.migration-in-progress
```

---

### Phase 4 — Verify and Emit

**Verification (CLI-accessible only — do NOT invoke `/forge:health` here):**

```sh
# 1. Validate the store
forge_validate_store({ dryRun: true })

# 2. Verify substitution outputs are non-empty
ls .forge/personas/*.md .forge/skills/*.md .forge/workflows/*.md .forge/templates/*.md

# 3. Verify project-context.json exists and is valid JSON
node -e "JSON.parse(require('fs').readFileSync('.forge/project-context.json','utf8')); console.log('project-context.json OK');"
```

Report findings to the user.

Tell the user: "Run `/forge:health` after migration completes to check overall
knowledge base status."

**Event emission — canonical schema compliant:**

Determine the sprint ID for the event. Look up the first sprint record in the
store:
```sh
ls .forge/store/sprints/*.json 2>/dev/null | sort | head -n1
```

Read that file and extract the `sprintId` field. If no sprint files exist, use
`"migration"` as the `sprintId` placeholder.

```sh
MIGRATION_END=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
forge_store({ command: "emit", args: [""{projectSprintId}"", '{
  "eventId": "migration-'] })"$(date -u +%Y%m%dT%H%M%SZ)"'",
  "taskId": "migration",
  "sprintId": "{projectSprintId}",
  "role": "migration-agent",
  "action": "migration-completed",
  "phase": "write",
  "iteration": 1,
  "startTimestamp": "'"$MIGRATION_START"'",
  "endTimestamp": "'"$MIGRATION_END"'",
  "durationMinutes": {elapsed},
  "model": "{current-model}",
  "notes": "{\"source\":\"migration-from-v{old}\",\"archivePath\":\".forge/archive/pre-migration/\",\"projectContextPath\":\".forge/project-context.json\"}"
}'
```

If `store-cli.cjs emit` exits non-zero, report the error but do NOT block
migration completion — the event is diagnostic metadata, not a prerequisite for
a working `.forge/` install.

**Report completion summary:**

```
Structural migration complete.

  project-context.json: .forge/project-context.json (N / 19 string fields populated)
  Archive:              .forge/archive/pre-migration/ (N files, MANIFEST.md5 included)
  Structure snapshot:   .forge/structure-versions.json (source: migration-from-v{old})
  Review items:         K items in .forge/archive/pre-migration/MIGRATION_NOTES.md

Next steps:
  1. Run /forge:health to verify the knowledge base.
  2. Review MIGRATION_NOTES.md for any extractions requiring manual attention.
  3. If something looks wrong, use the rollback procedure below.
```

---

## Rollback Procedure

The archive at `.forge/archive/pre-migration/` contains the complete pre-migration
state. To restore:

```sh
# Step 1 — Remove the migrated outputs
rm -rf .forge/personas .forge/skills .forge/workflows .forge/templates
rm -f  .forge/project-context.json .forge/structure-versions.json

# Step 2 — Restore from archive
cp -r .forge/archive/pre-migration/personas  .forge/personas  2>/dev/null || true
cp -r .forge/archive/pre-migration/skills    .forge/skills    2>/dev/null || true
cp -r .forge/archive/pre-migration/workflows .forge/workflows 2>/dev/null || true
cp -r .forge/archive/pre-migration/templates .forge/templates 2>/dev/null || true
cp    .forge/archive/pre-migration/project-context.json .forge/project-context.json 2>/dev/null || true

# Step 3 — Verify restoration
ls .forge/personas/*.md .forge/skills/*.md .forge/workflows/*.md .forge/templates/*.md

# Step 4 (optional) — Keep archive for audit, or remove it
# rm -rf .forge/archive/pre-migration/
```

To verify the restored files match the originals, compare against `MANIFEST.md5`:
```sh
cd .forge && md5sum -c archive/pre-migration/MANIFEST.md5 2>/dev/null | grep -v OK || echo "All files match"
```

---

## Error Handling

| Situation | Action |
|---|---|
| `.forge/config.json` missing | Stop. Tell user to run `/forge:init` first. |
| `structure-versions.json` already exists | Stop. Structural migration not needed for post-T05 installs. |
| Sentinel found + valid draft exists | Offer resume from Phase 2 or fresh restart. |
| Sentinel found + no valid draft | Offer fresh restart (remove sentinel and continue). |
| User declines confirmation | Remove sentinel, exit cleanly. No writes made. |
| `validate-store.cjs --dry-run` fails after writing `project-context.json` | Halt. Report errors. Do NOT remove sentinel. User fixes and re-runs. |
| `substitute-placeholders.cjs` exits non-zero | Halt. Report error. Do NOT remove sentinel. |
| `manage-versions.cjs init` exits non-zero | Halt. Report error. Archive is complete — rollback is available. |
| `store-cli.cjs emit` exits non-zero | Report error. Continue — event is diagnostic only. |
| Any unexpected error | Describe the error, point user to rollback procedure, suggest `/forge:report-bug`. |

<!-- See _fragments/generation-instructions.md for Generation Instructions template -->
## Generation Instructions

- **Workflow Structure:** The generated `migrate_structural.md` must follow the strict multi-phase Algorithm block format (Phase 0 pre-flight → Phase 1 read/extract → Phase 2 confirmation gate → Phase 3 write → Phase 4 verify/emit → Rollback Procedure → Error Handling).
- **Context Isolation:** Forbid inline execution of archival or substitution operations; use `forge_store` reads and structured `node` invocations for all store interactions.
- **Project Specifics:**
  - Reference the project's `paths.engineering` from `.forge/config.json` for path resolutions; resolve the plugin root from `${CLAUDE_PLUGIN_ROOT}` (not `config.paths.forgeRoot`, which is retired).
  - Include the project's migration path docs in the Error Handling table (e.g., expected schema files, archive paths).
- **Token Reporting:** See `_fragments/finalize.md` — wire via `file_ref:`. Token reporting is diagnostic only (migration is not an orchestrated phase — it emits its own record via `store-cli emit` in Phase 4).
- **Event Emission:** Migration emits its own completion event directly via `store-cli emit` in Phase 4 (orchestrator-exception; this is not a task phase). The "do NOT emit yourself" rule does not apply here.
