# Phase 2 — Discover

**Deliverable:** 7 KB architecture docs + 3 index files + `project-context.json` + calibration baseline.

Set `$FORGE_ROOT` and resolve `$KB_PATH` from `.forge/config.json`:

```sh
KB_PATH=$(node -e "try{console.log(require('./.forge/config.json').paths.engineering)}catch{console.log('engineering')}")
```

---

## Phase gate — verify Phase 1 foundation

Before starting any work, verify Phase 1 completed successfully:

```sh
node "$FORGE_ROOT/tools/verify-phase.cjs" --phase 1
```

If this exits 1, halt and surface the missing fields. Phase 2 cannot proceed
without a valid `.forge/config.json`.

---

## Steps (follow in order)

### Step 1 — Render phase header

```sh
node "$FORGE_ROOT/tools/banners.cjs" --phase 2 4 "Discover" oracle
```

### Step 2 — Scaffold directories

Create the directory structure (fast Bash operations):

```sh
mkdir -p "{KB_PATH}/architecture" "{KB_PATH}/business-domain" \
         "{KB_PATH}/sprints" "{KB_PATH}/bugs" "{KB_PATH}/tools" \
         ".forge/store/sprints" ".forge/store/tasks" \
         ".forge/store/bugs" ".forge/store/events"
touch "{KB_PATH}/sprints/.gitkeep" "{KB_PATH}/bugs/.gitkeep" \
      ".forge/store/sprints/.gitkeep" ".forge/store/tasks/.gitkeep" \
      ".forge/store/bugs/.gitkeep" ".forge/store/events/.gitkeep"
```

### Step 3 — Generate KB documents

Read `$FORGE_ROOT/init/generation/generate-kb-doc.md` once (the per-subagent rulebook).

Generate all 7 knowledge-base documents. For each document, analyse the
project codebase for that topic and write to its output path. After writing
each document, read it back and verify the confidence header is present
(`<!-- AUTO-GENERATED — confidence: NN% -->`).

| Document | Output path | Focus |
|----------|-------------|-------|
| stack.md | `{KB_PATH}/architecture/stack.md` | Languages, frameworks, runtime, versions |
| processes.md | `{KB_PATH}/architecture/processes.md` | Services, build/deploy topology |
| database.md | `{KB_PATH}/architecture/database.md` | Entities, relationships, field types |
| routing.md | `{KB_PATH}/architecture/routing.md` | API surface, route groups, auth strategy |
| deployment.md | `{KB_PATH}/architecture/deployment.md` | Environments, CI/CD, infra targets |
| entity-model.md | `{KB_PATH}/business-domain/entity-model.md` | Full entity inventory with fields |
| stack-checklist.md | `{KB_PATH}/stack-checklist.md` | Review checklist items from stack + testing |

You may spawn all 7 as parallel subagents in a single Agent tool message for speed.
Wait for all 7 to return. Retry any that returned FAILED: once.
Any still failing after one retry: halt and surface the id list.

### Step 4 — Create index files (sequential)

After all leaf docs are written:

1. **`{KB_PATH}/architecture/INDEX.md`** — list and link to the 5 architecture docs
2. **`{KB_PATH}/business-domain/INDEX.md`** — list and link to entity-model.md
3. **`{KB_PATH}/MASTER_INDEX.md`** — scaffold linking both INDEX files; include
   `## Domain Entities` section listing discovered entities (one per line)

Generate these sequentially — each builds on what's already on disk.

### Step 5 — Construct project-context.json

After all KB docs and MASTER_INDEX are written, construct `.forge/project-context.json`
inline (do NOT spawn a subagent for this step).

Using the `x-placeholder` annotations from
`$FORGE_ROOT/schemas/project-context.schema.json` as a guide, map discovered
facts to schema fields. Required fields: `project.name`, `project.prefix`
(must be non-empty strings). All array fields must be arrays.

Write the JSON to `.forge/project-context.json`.

**Structural validation failure halts Phase 2** with a descriptive error:
```
× Phase 2 validation failed: project.name is missing or empty.
```

### Step 6 — Write calibration baseline

After `project-context.json` is written, compute and write `calibrationBaseline`
into `.forge/config.json`:

1. Read `$FORGE_ROOT/.claude-plugin/plugin.json` → `version`
2. Hash `{KB_PATH}/MASTER_INDEX.md` (strip blank lines + `<!--` lines, SHA-256)
3. List done sprint IDs from `.forge/store/sprints/`
4. Merge into config: `calibrationBaseline: { lastCalibrated, version, masterIndexHash, sprintsCovered }`

Write `.forge/init-progress.json`:
```json
{ "lastPhase": 2, "timestamp": "<current ISO timestamp>" }
```

---

## Verify Phase 2

After writing `init-progress.json`, verify the deliverable:

```sh
node "$FORGE_ROOT/tools/verify-phase.cjs" --phase 2 --kb-path "$KB_PATH"
```

- **Exit 0:** Phase 2 complete. Proceed to Phase 3.
- **Exit 1:** Read the JSON output to identify missing documents. Fix them and
  re-run verify once. If it still fails, halt and surface the JSON error to
  the user.
