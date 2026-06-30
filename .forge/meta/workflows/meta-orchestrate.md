---
requirements:
  reasoning: High
  context: High
  speed: Medium
audience: orchestrator-only
deps:
  personas: [architect, engineer, supervisor, bug-fixer, collator, qa-engineer]
  skills: [architect, engineer, supervisor, generic]
  templates: []
  sub_workflows: [plan_task, implement_plan, review_plan, review_code, fix_bug, architect_approve, commit_task, validate_task]
  kb_docs: [architecture/stack.md]
  context_pack: .forge/cache/context-pack.md
  config_fields: [paths.engineering]
---

# 🌊 Meta-Workflow: Orchestrate Task

## Purpose

Wire the atomic workflows into a pipeline that drives a single task through
the complete lifecycle. This is the task state machine.

## Session Preflight (run once, before the phase loop)

The deterministic pre-dispatch glue — FORGE_ROOT resolution, config
reconciliation, generation-manifest state, calibration-baseline freshness,
MASTER_INDEX hashing, and the structure check — is bundled into a single
deterministic tool, `forge/tools/forge-preflight.cjs`. **Do NOT hand-run these
checks turn-by-turn.** Read the one compact blob it produces, once, at the top
of the orchestration:

1. The SessionStart hook (`hooks/preflight-session.cjs`) primes the blob at
   `.forge/cache/preflight-status.json` for any project that has a `.forge/`
   directory. Read that file. If it is absent or stale, run the tool once:
   `node .forge/tools/forge-preflight.cjs` and read its stdout.
2. Branch on `blob.ok`:
   - **`ok: true`** → proceed to the phase loop using `blob.forgeRoot` and the
     recorded state. Do not re-derive any field the blob already carries
     (`masterIndexHash`, `calibrationFresh`, `manifestState`, `structureOk`);
     surface `calibrationFresh.suggest` to the operator if `fresh` is false, but
     this is advisory and does not block the run.
   - **`ok: false`** → **halt before phase 1** (fast-fail-safe). This is a
     pre-dispatch halt: print `blob.warnings`, route through the existing
     escalation idiom (see `§ Escalation Procedure`) — emit the standard
     escalation event and message — and instruct the operator to fix the
     surfaced preflight warning and re-run. A half-initialized run must never
     proceed.

The blob is the single source of truth for these concerns for the remainder of
the run. The SessionStart hook is command-name-independent (SessionStart fires
before any command and carries no per-command signal); the scoping to
run-task / fix-bug / run-sprint contexts lives here, in the orchestration
preamble, which only those commands reach.

## Pipeline Phases

Each phase has:
- `name` — identifier
- `agent` — which role executes
- `workflow` — which workflow file to load
- `requires` — prerequisite artifact
- `produces` — output artifact
- `max_iterations` — revision loop limit (for review phases)
- `gate_checks` — conditions that must pass before proceeding

## Model Resolution

Detect cluster from env vars at session start, then dispatch accordingly:

| Env var | Purpose |
|---------|---------|
| `ANTHROPIC_DEFAULT_OPUS_MODEL` | What "opus" resolves to |
| `ANTHROPIC_DEFAULT_SONNET_MODEL` | What "sonnet" resolves to |
| `ANTHROPIC_DEFAULT_HAIKU_MODEL` | What "haiku" resolves to |

- **Single cluster** — all three vars equal (or unset): omit `model` on Agent spawns; subagents inherit the parent.
- **Tiered cluster** — vars differ: pass `model=tier` (opus/sonnet/haiku) based on ROLE_TIER mapping.
- **Unknown cluster** — no `ANTHROPIC_DEFAULT_*` vars: pass the canonical model ID from ROLE_TIER_DEFAULTS.
- **Per-phase override** — `model` field in `config.pipelines` phase takes highest precedence.

### Role-to-Tier Mapping

| Role | Tier |
|------|------|
| `review-plan`, `review-code`, `validate`, `approve` | opus |
| `plan`, `implement` | sonnet |
| `commit`, `writeback` | haiku |

Unknown cluster canonical defaults: opus → `claude-opus-4-5`, sonnet → `claude-sonnet-4-6`, haiku → `claude-haiku-4-5`.

Phase announcement format: `→ TASK-ID  [tier → resolved-model]` (e.g. `→ SPECT-T01  [opus → claude-opus-4-6]`).
On single cluster, show the model directly. On unknown, show `tier → canonical`.

## Pipeline Resolution

The orchestrator supports pluggable pipelines. When starting a task:

1. Read the task manifest from `.forge/store/tasks/{TASK_ID}.json`.
2. If `task.pipeline` is set, look up that key in `.forge/config.json` → `pipelines`.
3. If found, use the phases defined in that pipeline.
4. If `task.pipeline` is not set or the key is not found, use the `default` pipeline
   (either from `config.pipelines.default` or the hardcoded default below).

Each phase in a pipeline has:
- `command` — the slash command to invoke (passed the task ID as argument)
- `role` — semantic role (`plan`, `review-plan`, `implement`, `review-code`, `approve`, `commit`)
- `maxIterations` — for review roles, the revision loop limit (default 3)
- `on_revision` — (optional) command name of the phase to re-invoke on "Revision Required";
  if absent, defaults to the nearest preceding phase whose role is not a review role

## Default Pipeline

```
plan → review-plan → [loop max 3] → implement → review-code → [loop max 3] → validate → [loop max 3] → approve → writeback → commit
```

When no `pipelines` section exists in config, the orchestrator uses this
hardcoded default. Projects that define `config.pipelines.default` override it.

## Context Isolation

**Each phase MUST run as a subagent (Agent tool call), NOT inline.**

Invoking phases inline accumulates context from every prior phase and task into
the orchestrator's window. This violates Forge's design principle of keeping
context light and nimble. By the time a sprint reaches its third or fourth task,
an inline orchestrator is carrying tens of thousands of tokens of prior work that
is irrelevant to the current phase.

The fix: use the Agent tool to spawn a subagent per phase. Each subagent:
- Starts with a fresh context window
- Receives only what it needs: the workflow file path and the task ID
- Receives a PROJECT_OVERLAY (task-scoped index slice) instead of reading MASTER_INDEX.md directly
- Writes results to disk (artifacts, task status updates)
- Returns to the orchestrator, which then reads the verdict from disk

The orchestrator itself stays minimal — it only holds the phase loop and event log.

## Token Self-Reporting

Each phase subagent is responsible for reporting its own token usage via a sidecar file.

**Before returning, every subagent MUST:**

1. Probe token usage for the session: invoke `/cost` if the host runtime
   supports it (Claude Code only); on any other runtime treat as unavailable.
   Do NOT shell out to a `cost-cli.cjs` — there is no such tool.
2. Parse the output for the five fields:
   `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `estimatedCostUSD`.
3. Write the usage sidecar via `forge_store({ command: "emit", args: ["{sprintId}", '{sidecar-json}'] }) --sidecar` with the exact format:
   ```json
   {
     "inputTokens": <integer>,
     "outputTokens": <integer>,
     "cacheReadTokens": <integer>,
     "cacheWriteTokens": <integer>,
     "estimatedCostUSD": <number>
   }
   ```

The `eventId` is computed by the orchestrator before spawning and passed in the subagent prompt —
it follows the format `{ISO_TIMESTAMP}_{TASK_ID}_{role}_{action}` (e.g.
`20260415T141523000Z_ACME-S02-T03_engineer_implement`).

The leading underscore on the sidecar filename marks it as ephemeral — `validate-store.cjs` skips
files prefixed with `_`, so the sidecar will never be treated as a real event record. If `/cost` is
unavailable or token data cannot be parsed, skip writing the sidecar silently — the orchestrator
handles missing sidecars gracefully (see Execution Algorithm below).

## Role-to-Noun Mapping

The orchestrator resolves persona and skill file lookups using **noun-based**
filenames, not role-literal filenames. A role like `plan` maps to the noun
`engineer`, so the persona file is `engineer.md`, not `plan.md`.

```
ROLE_TO_NOUN = {
  "plan":        "engineer",
  "implement":   "engineer",
  "update-plan": "engineer",
  "update-impl": "engineer",
  "commit":      "engineer",
  "review-plan": "supervisor",
  "review-code": "supervisor",
  "validate":    "qa-engineer",
  "approve":     "architect",
  "writeback":   "collator",
}
```

The `.get(key, fallback)` pattern preserves the old role-literal behaviour for
any role not yet in the table, which is a safe degradation path for custom
pipeline roles.

## Persona Injection Modes

Subagent prompts include a **role block** that tells the agent who it is
and what capabilities it has. Two modes are supported, selected by the
`FORGE_PROMPT_MODE` environment variable:

| Mode | Behaviour | Default |
|------|-----------|---------|
| `reference` | Compact summary from `.forge/cache/persona-pack.json`, plus a file_ref pointer to the full persona/skill definitions. | ✅ |
| `inline` | Legacy: inject the full verbatim persona and skill file contents. Kept for one version as a rollback path. | |

The pack is built by `/forge:rebuild` via
`forge/tools/build-persona-pack.cjs`. It compiles YAML frontmatter from
`$FORGE_ROOT/meta/personas/meta-*.md` and `$FORGE_ROOT/meta/skills/meta-*.md`
into `.forge/cache/persona-pack.json`.

### Helper: `compose_role_block(persona_noun)`

```
def compose_role_block(persona_noun):
    mode = os.environ.get("FORGE_PROMPT_MODE", "reference")

    if mode == "inline":
        # Legacy behaviour — full persona + skill prose inline.
        persona_content = read_file(f".forge/personas/{persona_noun}.md")
        skill_content   = read_file(f".forge/skills/{persona_noun}-skills.md")
        return f"{persona_content}\n\n{skill_content}"

    # Reference mode (default) — compact summary from the pack.
    pack = read_json(".forge/cache/persona-pack.json")
    persona = pack["personas"].get(persona_noun)
    skill   = pack["skills"].get(f"{persona_noun}-skills")

    if not persona:
        # Fail loud rather than silently degrade. Missing pack entry is a
        # regeneration bug and should be reported via /forge:report-bug.
        raise OrchestratorError(
            f"persona '{persona_noun}' not in persona-pack. "
            "Run /forge:rebuild to rebuild the pack."
        )

    lines = [
        f"You are acting as the {persona['role']}.",
        "",
        f"Persona: {persona['id']} — {persona['summary']}",
        "",
        "Your responsibilities:",
    ]
    for r in persona.get("responsibilities", []):
        lines.append(f"- {r}")
    if persona.get("outputs"):
        lines.append("")
        lines.append(f"Your outputs: {', '.join(persona['outputs'])}")

    if skill:
        lines.append("")
        lines.append("Skill capabilities you have available:")
        for c in skill.get("capabilities", []):
            lines.append(f"- {c}")

    lines.append("")
    lines.append(
        f"Full persona definition: {persona['file_ref']}. "
        + (f"Full skill definition: {skill['file_ref']}. " if skill else "")
        + "The summary above is authoritative. If insufficient, escalate — "
        + "do not read the full persona or skill file."
    )
    return "\n".join(lines)
```

**Rollback:** set `FORGE_PROMPT_MODE=inline`. No persisted state to revert.
The `inline` branch will be removed one version after `reference` ships.

> **Scope note (added FORGE-S27-T02, 2026-05-31):** `FORGE_PROMPT_MODE=inline` restores
> the **role block only** — full verbatim persona + skill file contents. It does NOT restore
> the full `MASTER_INDEX`: the overlay (`build-overlay.cjs`, line 462) is unconditional and
> delivers the same task-scoped slice in both modes. It also does NOT affect the workflow file,
> which is read verbatim from disk in both modes. There is currently no per-call escape hatch
> to the full `MASTER_INDEX` — a separate overlay-bypass flag would be required.

## Execution Algorithm

The orchestrator MUST follow this procedure exactly. Do not deviate.

```
# --- Utility helpers ---
def safe_parse_json(text):
    """Attempt to parse a JSON string; return dict or None on failure."""
    try:
        return json.loads(text.strip()) if text and text.strip() else None
    except Exception:
        return None

# --- Persona symbol lookup (emoji, name, tagline) ---
PERSONA_MAP = {
  "plan":        ("🌱", "Engineer",    "I plan what will be built before any code is written."),
  "implement":   ("🌱", "Engineer",    "I build what was planned. I do not move forward until the code is clean."),
  "update-plan": ("🌱", "Engineer",    "I address what the Supervisor found. No more, no less."),
  "update-impl": ("🌱", "Engineer",    "I address what the Supervisor found. No more, no less."),
  "commit":      ("🌱", "Engineer",    "I close out completed work with a clean, honest commit."),
  "review-plan": ("🌿", "Supervisor",  "I review before things move forward. I read the actual task prompt, not just the plan."),
  "review-code": ("🌿", "Supervisor",  "I review before things move forward. I read the actual code, not the report."),
  "validate":    ("🍵", "QA Engineer", "I validate against what was promised. The code compiling is not enough."),
  "approve":     ("🗻", "Architect",   "I hold the shape of the whole. I give final sign-off before commit."),
  "writeback":   ("🍃", "Collator",    "I gather what exists and arrange it into views."),
}

# --- Banner identity map (banner name per phase role) ---
# Maps each role to a banner in forge/tools/banners.cjs.
# Displayed by the orchestrator ONLY (badge before spawn, exit signal after return).
# Subagents do NOT display banners — the orchestrator owns phase announcements.
BANNER_MAP = {
  "plan":        "forge",
  "implement":   "forge",
  "update-plan": "forge",
  "update-impl": "forge",
  "commit":      "forge",
  "review-plan": "oracle",
  "review-code": "oracle",
  "validate":    "lumen",
  "approve":     "north",
  "writeback":   "drift",
}

for each task in dependency_sorted(tasks):
  # --- Pre-task status guard ---
  # If a task is already blocked or escalated from a prior sprint/run,
  # skip it entirely rather than attempting any phase.
  task_record = read_json(f".forge/store/tasks/{task.taskId}.json")
  if task_record and task_record.get("status") in ("blocked", "escalated"):
    print(f"  ⚠ {task.taskId}  — status is {task_record['status']}, skipping\n")
    emit_event(task, phase=None, action="task_skipped",
               notes=f"task status is {task_record['status']}")
    continue

  phases = resolve_pipeline(task)           # from config.pipelines or default
  iteration_counts = {}                     # keyed by phase command name
  retry_count = {}                           # keyed by phase command name (subagent retry tracking)
  i = 0

  # --- Detect execution cluster from env vars (see Model Resolution) ---
  opus_model   = env("ANTHROPIC_DEFAULT_OPUS_MODEL", "")
  sonnet_model = env("ANTHROPIC_DEFAULT_SONNET_MODEL", "")
  haiku_model  = env("ANTHROPIC_DEFAULT_HAIKU_MODEL", "")
  if opus_model and opus_model == sonnet_model == haiku_model:
    cluster = "single"
    resolved_model = opus_model   # all tiers same model
  elif opus_model:
    cluster = "tiered"
    resolved_model = None         # each tier resolves differently
  else:
    cluster = "unknown"
    resolved_model = env("CLAUDE_CODE_SUBAGENT_MODEL", "unknown")

  # --- Role-to-tier mapping for tiered cluster dispatch ---
  ROLE_TIER = {
    "review-plan": "opus",
    "review-code": "opus",
    "validate":    "opus",
    "approve":     "opus",
    "plan":        "sonnet",
    "implement":   "sonnet",
    "commit":      "haiku",
    "writeback":   "haiku",
  }

  # --- Clear progress log for this sprint ---
  progress_log_path = f".forge/store/events/{sprint_id}/progress.log"
  run_bash(f'forge_store({ command: "progress-clear", args: ["{sprint_id}')"] })

  while i < len(phases):
    phase = phases[i]

    # --- Resolve model for display and dispatch (see Model Resolution) ---
    if phase.model:                                   # per-phase override from config
      display_model = phase.model
      dispatch_model = phase.model                   # pass override to Agent tool
      if env(f"ANTHROPIC_DEFAULT_{phase.model.upper()}_MODEL"):
        resolved = env(f"ANTHROPIC_DEFAULT_{phase.model.upper()}_MODEL")
        display_model = f"{phase.model} → {resolved}"
    elif cluster == "single" and resolved_model:
      display_model = resolved_model
      dispatch_model = None                           # inherit parent model
    elif cluster == "tiered":
      tier = ROLE_TIER.get(phase.role, "sonnet")
      resolved = env(f"ANTHROPIC_DEFAULT_{tier.upper()}_MODEL", tier)
      display_model = f"{tier} → {resolved}" if resolved != tier else tier
      dispatch_model = tier                           # pass tier name, Claude Code resolves
    else:
      # Unknown cluster: no ANTHROPIC_DEFAULT_*_MODEL vars set.
      # Fall back to ROLE_TIER with canonical model defaults so subagents
      # run on a predictable model instead of inheriting the orchestrator's own.
      ROLE_TIER_DEFAULTS = {
        "opus":   "claude-opus-4-5",
        "sonnet": "claude-sonnet-4-6",
        "haiku":  "claude-haiku-4-5",
      }
      tier = ROLE_TIER.get(phase.role, "sonnet")
      canonical = ROLE_TIER_DEFAULTS[tier]
      display_model = f"{tier} → {canonical}"
      dispatch_model = canonical                      # pass full model id to Agent tool

    # --- Compute eventId before spawning so the subagent can name its sidecar ---
    start_ts = current_iso_timestamp()       # e.g. "20260415T141523000Z"
    event_id = f"{start_ts}_{task_id}_{phase.role}_{phase.action}"
    sidecar_path = f".forge/store/events/{sprint_id}/_{event_id}_usage.json"  # used by merge-sidecar

    # --- Compute agent name for progress IPC ---
    persona_noun = ROLE_TO_NOUN.get(phase.role, phase.role)
    iteration = iteration_counts.get(phase.command, 0) + 1
    agent_name = f"{task_id}:{persona_noun}:{phase.role}:{iteration}"

    # --- Announce phase with identity banner (badge) + task context ---
    # --quiet makes banners.cjs emit zero stdout (unconditional; no isTTY branch).
    # The badge is fully suppressed during the automated run_bash call — it does
    # not enter the LLM context window and is not shown on the human terminal.
    # The human-visible per-phase marker is the print() line below.
    #
    # Digest-compliance note (FORGE-S27-T03): every deterministic tool call this
    # loop body makes is already digest-compliant on its success path:
    #   store-cli write verbs (update-status, emit, merge-sidecar, set-summary,
    #   progress-clear) → silent on success.
    #   preflight-gate.cjs → silent on success (stderr only on failure).
    #   read-verdict.cjs   → one load-bearing token (e.g. "approved"); orchestrator
    #                        branches on it — must not be suppressed.
    #   banners.cjs --badge → 1-line ANSI badge; made zero-cost via --quiet below.
    #   build-overlay.cjs  → ~1185 chars captured into overlay_md and injected into
    #                        the subagent prompt as Project Context. This is payload
    #                        data, not a log — reducing it would break prompt assembly.
    #                        Reference-mode redesign is deferred to the T02/forge-compress
    #                        work; leave unchanged here.
    emoji, persona_name, tagline = PERSONA_MAP.get(phase.role, ("🌊", "Orchestrator", "I move tasks through their lifecycle."))
    banner_name = BANNER_MAP.get(phase.role, "forge")
    run_bash(f'forge_banner({ name: "--badge" }) {banner_name} --quiet')
    print(f"  → {task_id}  [{display_model}]\n")

    # --- Start progress Monitor before spawning subagent ---
    # The Monitor streams lines from the progress log as the subagent works.
    # New lines arrive as notifications while the Agent tool blocks on the subagent.
    start_monitor(
      command=f"tail -n +1 -F {progress_log_path} 2>/dev/null || true",
      description=f"Progress: {agent_name}",
      persistent=False
    )

    # --- Pre-flight gate check (see Phase Gates below) ---
    # Resolve FORGE_ROOT once (needed for meta/workflows fragment reads below).
    FORGE_ROOT = resolve_forge_root()
    preflight_result = run_bash(
      f'forge_preflight({ phase: "{phase.role}", task: "{task_id}'" })
    )
    if preflight_result.exit_code == 1:
      # Gate failed: halt the orchestrator loop for THIS task. Do not retry,
      # do not spawn. Structured failure JSON is on stdout; human-readable detail on stderr.
      # Parse the structured JSON for a user-friendly advisory.
      gate_failure = safe_parse_json(preflight_result.stdout)
      if gate_failure:
        reason_code   = gate_failure.get("reasonCode", "unknown")
        gate_detail   = gate_failure.get("detail", preflight_result.stderr.strip())
        gate_remedy   = gate_failure.get("remediation", "")
        print(f"  ✗ {task_id}  {phase.role}  — gate failed [{reason_code}]")
        print(f"    Detail:      {gate_detail}")
        print(f"    Remediation: {gate_remedy}")
        gate_notes = f"gate_failed [{reason_code}]: {gate_detail}"
      else:
        print(f"  ✗ {task_id}  {phase.role}  — gate failed\n{preflight_result.stderr}")
        gate_notes = f"gate_failed: {preflight_result.stderr}"
      append_progress(progress_log_path, f"❌ Gate failed for {phase.role}: {gate_notes}")
      emit_event(task, phase, action="gate_failed", notes=gate_notes)
      # ---- ESCALATION (mandatory hard stop — do NOT continue) ----
      run_bash(f'forge_store({ command: "update-status", args: ["task", "{task_id}", "status", "escalated')"] })
      emit_event(task, phase, eventId=event_id, iteration=iteration,
                 action="escalated", verdict="escalated",
                 notes=gate_notes)
      print(f"  ⚠ Task {task_id} escalated: {gate_notes}\n")
      print(f"  Review artifact: {artifact_path}\n")
      print(f"  Resume with: /{phase.command} {task_id} after addressing the issues.\n")
      break                                   # stop processing this task
    elif preflight_result.exit_code == 2:
      # Misconfiguration (unknown phase, malformed gates block). Fail loud.
      print(f"  ⚠ {task_id}  {phase.role}  — gate misconfigured\n{preflight_result.stderr}")
      # ---- ESCALATION (mandatory hard stop — do NOT continue) ----
      run_bash(f'forge_store({ command: "update-status", args: ["task", "{task_id}", "status", "escalated')"] })
      emit_event(task, phase, eventId=event_id, iteration=iteration,
                 action="escalated", verdict="escalated",
                 notes=f"gate_misconfigured: {preflight_result.stderr}")
      print(f"  ⚠ Task {task_id} escalated: gate_misconfigured: {preflight_result.stderr}\n")
      print(f"  Review artifact: {artifact_path}\n")
      print(f"  Resume with: /{phase.command} {task_id} after addressing the issues.\n")
      break

    # --- Invoke phase as subagent (fresh context per phase) ---
    emit_event(task, phase, eventId=event_id, iteration=iteration, action="start")

    # Symmetric Injection Assembly: Persona -> Skill -> Workflow
    # Mode is governed by FORGE_PROMPT_MODE (default: "reference").
    # See "Persona injection modes" below for the full helper definition.
    role_block = compose_role_block(persona_noun)

    # --- Compose prior-phase summary block (delta: last 3 phases only) ---
    # <!-- See _fragments/context-injection.md for canonical definition -->
    summary_block = compose_summary_block(task_id, record_type="task") if phase.context.prior_summaries != "none" else ""

    # --- Compose architecture context block (conditional on phase.context.architecture) ---
    # <!-- See _fragments/context-injection.md for canonical definition -->
    architecture_block = (
      compose_architecture_block(".forge/cache/context-pack.md", ".forge/cache/context-pack.json")
      if phase.context.architecture else ""
    )

    # --- Materialize project overlay (replaces MASTER_INDEX.md read in subagent) ---
    overlay_result = run_bash(
      f'node .forge/tools/build-overlay.cjs --task {task_id} --format md'
    )
    overlay_md = overlay_result.stdout if overlay_result.exit_code == 0 else ""

    # --- Load finalize fragment (token reporting contract) ---
    finalize_fragment = read_file(f"{FORGE_ROOT}/meta/workflows/_fragments/finalize.md") if file_exists(f"{FORGE_ROOT}/meta/workflows/_fragments/finalize.md") else ""

    # --- Compose review loop context block (review-role phases only) ---
    # Injected between summary_block and role_block so reviewers know their
    # position in the revision loop at the moment they are spawned.
    # `iteration` is the current attempt number (pre-spawn, not post-increment).
    # `phase.maxIterations` is the configured limit (default 3).
    if phase.role in ("review-plan", "review-code", "validate"):
      review_loop_context = (
        f"### Review Loop Context\n"
        f"- Iteration: {iteration} of {phase.maxIterations}\n"
        f"- Is final iteration: {iteration >= phase.maxIterations}\n\n"
      )
    else:
      review_loop_context = ""

    spawn_kwargs = dict(
      prompt=(
        f"Append progress entries to {progress_log_path} via store-cli "
        f"(agent: {agent_name}, banner: {banner_name}) — see _fragments/progress-reporting.md.\n\n"
        f"---\n\n"
        f"{architecture_block}"
        f"{summary_block}"
        f"{review_loop_context}"
        f"{role_block}\n\n"
        f"### Project Context\n"
        f"{overlay_md}\n\n"
        f"### Current Working Context\n"
        f"- Sprint Root: {sprint_root_path}\n"
        f"- Task Root:   {task_root_path}\n"
        f"- Store Root:  {store_root_path}\n\n"
        f"Read `.forge/workflows/{phase.workflow}` and follow it. Task ID: {task_id}.\n\n"
        f"{finalize_fragment}"
      ),
      description=f"{emoji} {persona_name} — {phase.name} for {task_id}",
    )
    if dispatch_model:
      spawn_kwargs["model"] = dispatch_model
    spawn_subagent(**spawn_kwargs)
    # Subagent reads all context from disk, does its work, writes artifacts/status to disk, then exits.

    # --- Stop progress Monitor ---
    stop_monitor(progress_log_path)

    # --- Subagent response validation (retry once, escalate on second failure) ---
    # The subagent must produce a usable result. Three failure classes:
    #   1. Empty response: subagent returned nothing or whitespace-only output
    #   2. Subagent error: subagent exited non-zero (crash, OOM, tool error)
    #   3. Timeout: subagent did not return within the session timeout
    #
    # On first failure: retry once with a simplified prompt that strips
    # non-essential context (summary block, architecture block) and adds
    # a direct instruction to produce a verdict or error report.
    # On second failure: escalate to human — do NOT continue the phase loop.

    if subagent_failed_or_empty(result):
      if retry_count.get(phase.command, 0) == 0:
        # First failure: retry with simplified prompt
        retry_count[phase.command] = 1
        print(f"  ⚠ {task_id}  {phase.role}  — subagent response empty or errored, retrying with simplified prompt\n")
        emit_event(task, phase, action="subagent_retry",
                   notes=f"first failure: {subagent_failure_reason(result)}")

        # Simplify: remove summary_block and architecture_block from prompt
        simplified_kwargs = dict(spawn_kwargs)
        simplified_kwargs["prompt"] = (
          f"### Progress Reporting\n"
          f"- Agent name: {agent_name}\n"
          f"- Progress log: {progress_log_path}\n"
          f"- Banner key: {banner_name}\n\n"
          f"Append progress entries as you work.\n\n"
          f"---\n\n"
          f"{review_loop_context}"
          f"{role_block}\n\n"
          f"### Current Working Context\n"
          f"- Sprint Root: {sprint_root_path}\n"
          f"- Task Root:   {task_root_path}\n"
          f"- Store Root:  {store_root_path}\n\n"
          f"Read `.forge/workflows/{phase.workflow}` and follow it. Task ID: {task_id}.\n\n"
          f"{overlay_md}\n\n"
          f"IMPORTANT: You MUST produce a result. If the workflow cannot complete, "
          f"write a verdict or error report to the expected artifact path and return."
        )
        spawn_subagent(**simplified_kwargs)
        stop_monitor(progress_log_path)

        # Re-validate the retry result
        if subagent_failed_or_empty(result):
          # Second failure: escalate
          print(f"  ✗ {task_id}  {phase.role}  — subagent failed after retry, escalating\n")
          emit_event(task, phase, action="subagent_escalated",
                     notes=f"second failure: {subagent_failure_reason(result)}")
          # ---- ESCALATION (mandatory hard stop — do NOT continue) ----
          run_bash(f'forge_store({ command: "update-status", args: ["task", "{task_id}", "status", "escalated')"] })
          emit_event(task, phase, eventId=event_id, iteration=iteration,
                     action="escalated", verdict="escalated",
                     notes=f"subagent failed after retry: {subagent_failure_reason(result)}")
          print(f"  ⚠ Task {task_id} escalated: subagent {phase.role} failed after retry — {subagent_failure_reason(result)}\n")
          print(f"  Resume with: /{phase.command} {task_id} after addressing the issues.\n")
          break
      else:
        # Already retried once — this is the second failure
        print(f"  ✗ {task_id}  {phase.role}  — subagent failed after retry, escalating\n")
        emit_event(task, phase, action="subagent_escalated",
                   notes=f"second failure: {subagent_failure_reason(result)}")
        # ---- ESCALATION (mandatory hard stop — do NOT continue) ----
        run_bash(f'forge_store({ command: "update-status", args: ["task", "{task_id}", "status", "escalated')"] })
        emit_event(task, phase, eventId=event_id, iteration=iteration,
                   action="escalated", verdict="escalated",
                   notes=f"subagent failed after retry: {subagent_failure_reason(result)}")
        print(f"  ⚠ Task {task_id} escalated: subagent {phase.role} failed after retry — {subagent_failure_reason(result)}\n")
        print(f"  Resume with: /{phase.command} {task_id} after addressing the issues.\n")
        break

    # --- Sidecar merge: merge token usage written by subagent via custodian ---
    # The subagent wrote the sidecar via forge_store({ command: "emit", args: ["{sprintId}", '{sidecar-json}'] }) --sidecar
    # Merge the sidecar into the canonical event and delete the sidecar file
    run: forge_store({ command: "merge-sidecar", args: ["{sprint_id}", "{event_id}"] })
    # merge-sidecar reads the sidecar, merges token fields into the canonical event, and deletes the sidecar
    # If the sidecar does not exist, merge-sidecar returns an error — treat as non-fatal (subagent may have skipped it)
    emit_event(task, phase, action="complete")

    # --- Phase-exit signal ---
    # Non-review phases always advance with a completion signal
    if phase.role not in ("review-plan", "review-code", "validate"):
      print(f"  ✓ {task_id}  {phase.role}  — completed\n")
      i += 1
      # State-ledger compaction: the [checkpoint] line IS the state ledger — it carries
      # the loop bookkeeping (task_id, sprint_id, phase_index, iteration_counts) that
      # /compact must preserve verbatim. Raw tool output and subagent return text is
      # shed here; do not retain it between phases. The durable state is on disk.
      print(f"[checkpoint] task={task_id} sprint={sprint_id} phase_index={i} iterations={iteration_counts}")
      /compact
      continue

    # --- Review phase: detect verdict via read-verdict.cjs (see Verdict Detection below) ---
    # Verdicts come from the STORE record (phase summaries / task.status), NOT from a
    # markdown review artifact — the orchestrator never constructs an artifact path.
    # stdout is one of: approved | revision | n/a | unknown. Never pattern-match a
    # **Verdict:** line — the closed vocabulary lives in the tool.
    verdict_result = run_bash(
      f'node .forge/tools/read-verdict.cjs --phase {phase.role} --task {task_id}'
    )
    verdict_token = verdict_result.stdout.strip()
    if verdict_token == "approved":
      verdict = "Approved"
    elif verdict_token == "revision":
      verdict = "Revision Required"
    else:
      # "n/a" / "unknown" (no verdict recorded) or exit 2 (record not found / bad args).
      # Never guess.
      print(f"  ⚠ {task_id}  {phase.role}  — verdict_malformed, escalating\n")
      emit_event(task, phase, action="verdict_malformed",
                 notes=f"read-verdict stdout='{verdict_token}' exit={verdict_result.exit_code}")
      # ---- ESCALATION (mandatory hard stop — do NOT continue) ----
      run_bash(f'forge_store({ command: "update-status", args: ["task", "{task_id}", "status", "escalated')"] })
      emit_event(task, phase, eventId=event_id, iteration=iteration,
                 action="escalated", verdict="escalated",
                 notes="verdict_malformed: no verdict recorded in the phase summary / record")
      print(f"  ⚠ Task {task_id} escalated: verdict_malformed — no verdict recorded for {phase.role}\n")
      print(f"  Inspect with: node \.forge/tools/read-verdict.cjs\" --phase {phase.role} --task {task_id}\n")
      print(f"  Resume with: /{phase.command} {task_id} after addressing the issues.\n")
      break

    if verdict == "Approved":
      print(f"  ✓ {task_id}  {phase.role}  — Approved\n")
      i += 1                                # advance to next phase
      # State-ledger compaction: the [checkpoint] line IS the state ledger — it carries
      # the loop bookkeeping (task_id, sprint_id, phase_index, iteration_counts) that
      # /compact must preserve verbatim. Raw tool output and subagent return text is
      # shed here; do not retain it between phases. The durable state is on disk.
      print(f"[checkpoint] task={task_id} sprint={sprint_id} phase_index={i} iterations={iteration_counts}")
      /compact

    elif verdict == "Revision Required":
      iteration_counts[phase.command] = iteration_counts.get(phase.command, 0) + 1
      print(f"  ↻ {task_id}  {phase.role}  — Revision Required (iteration {iteration_counts[phase.command]})\n")

      if iteration_counts[phase.command] >= phase.maxIterations: # default 3
        # ---- ESCALATION (mandatory hard stop — do NOT continue) ----
        run_bash(f'forge_store({ command: "update-status", args: ["task", "{task_id}", "status", "escalated')"] })
        emit_event(task, phase, eventId=event_id, iteration=iteration,
                   action="escalated", verdict="escalated",
                   notes="max iterations reached")
        print(f"  ⚠ Task {task_id} escalated: max iterations reached\n")
        print(f"  Inspect with: node \.forge/tools/read-verdict.cjs\" --phase {phase.role} --task {task_id}\n")
        print(f"  Resume with: /{phase.command} {task_id} after addressing the issues.\n")
        break
        break                               # stop processing this task

      # Route back to the revision target
      target = phase.on_revision or nearest_preceding_non_review(phases, i)
      i = index_of(phases, target)          # loop back
      # State-ledger compaction: the [checkpoint] line IS the state ledger — it carries
      # the loop bookkeeping (task_id, sprint_id, phase_index, iteration_counts) that
      # /compact must preserve verbatim. Raw tool output and subagent return text is
      # shed here; do not retain it between phases. The durable state is on disk.
      print(f"[checkpoint] task={task_id} sprint={sprint_id} phase_index={i} iterations={iteration_counts}")
      /compact

    # No `else:` branch needed — read-verdict.cjs already exhausts the
    # possibilities (approved | revision | verdict_malformed), and the
    # malformed case is handled above before this if/elif chain.
```

## Agent Naming Convention

Each subagent is assigned a structured name at spawn time:

```
{taskId}:{persona_noun}:{phase.role}:{iteration}
```

| Component | Source | Example |
|-----------|--------|---------|
| `taskId` | Task ID from manifest | `FORGE-S09-T01` |
| `persona_noun` | `ROLE_TO_NOUN` mapping | `engineer`, `supervisor`, `qa-engineer` |
| `phase.role` | Pipeline phase role | `plan`, `review-plan`, `implement` |
| `iteration` | 1-based revision count for this phase | `1`, `2`, `3` |

Examples:

- `FORGE-S09-T01:engineer:plan:1` — First plan attempt for T01
- `FORGE-S09-T01:supervisor:review-plan:1` — First plan review for T01
- `FORGE-S09-T01:engineer:update-impl:2` — Second implementation revision for T01

The agent name is passed in the subagent prompt and used in every progress log
entry the subagent writes. It provides identity and traceability for mid-task
feedback.

## Progress Reporting

<!-- See _fragments/progress-reporting.md for canonical definition -->
> See `_fragments/progress-reporting.md` for the full progress log format and `store-cli progress` command reference.

Log path: `.forge/store/events/{sprintId}/progress.log`. Format: `{ISO_TIMESTAMP}|{agent_name}|{banner_key}|{status}|{detail}`. Clear at task start: `store-cli progress-clear {sprintId}`.

## Phase-Exit Signals

After each subagent returns, the orchestrator prints a phase-exit signal:

| Outcome | Format |
|---------|--------|
| Non-review phase completed | `  ✓ {task_id}  {phase_role}  — completed` |
| Review verdict: Approved | `  ✓ {task_id}  {phase_role}  — Approved` |
| Review verdict: Revision Required | `  ↻ {task_id}  {phase_role}  — Revision Required (iteration {n})` |
| Escalated | `  ⚠ {task_id}  {phase_role}  — escalated to human` |

Examples:

```
  ✓ FORGE-S09-T01  plan  — completed
  ✓ FORGE-S09-T01  review-plan  — Approved
  ↻ FORGE-S09-T01  review-plan  — Revision Required (iteration 2)
  ⚠ FORGE-S09-T01  validate  — escalated to human
```

## Verdict Detection

After each review phase completes, the orchestrator MUST read the verdict
before branching. Do not infer the verdict from conversation context alone, and
**never construct or read a markdown artifact path** to find it — the verdict
lives in the **store record** (the phase summary written by `set-summary`, or
`task.status` for the approve phase).

**Read the verdict via `read-verdict.cjs`** — addressed by entity ID and phase
role, never by file path. The tool sources the verdict from the record and
enforces a closed vocabulary so typos, case drift, and reviewer prose cannot
cause silent misclassification:

```
result = run_bash(f'node .forge/tools/read-verdict.cjs --phase {phase.role} --task {task_id}')
# stdout "approved" → approved
# stdout "revision" → revision
# stdout "n/a" | "unknown" → no verdict recorded (treat as malformed; do NOT guess)
# exit 2 → record not found / invalid args (treat as malformed)
```

Branch on the **stdout token** (exit 1 bundles both `revision` and the
no-verdict cases, so the token is authoritative). Recognised verdict values:

- **approved** — written as `verdict: "approved"` in the phase summary (or `task.status == approved` for the approve phase).
- **revision** — `verdict: "revision"`.

Anything else — `n/a`, `unknown`, a missing summary, or a missing record —
must NOT be treated as approved or revision; halt the loop and escalate via
`verdict_malformed`. (In bug mode pass `--bug {bug_id}`; `read-verdict.cjs`
applies the bug-specific phase→summary map.)

## Escalation Procedure

> **NOTE:** The Escalation Procedure is inlined at every call site in the
> Execution Algorithm. This section remains as a reference. When adding new
> escalation points, inline the full procedure — do NOT call `escalate_to_human()`
> as a bare function name.

When escalating to the human:

1. Update task status via `forge_store({ command: "update-status", args: ["task", "{taskId}", "status", "escalated`"] })
2. Emit a final event with `verdict: "escalated"` and `notes` explaining the reason
3. Output a clear message:
   ```
   ⚠ Task {TASK_ID} escalated: {reason}
   Review artifact: {artifact_path}
   Resume with: /{phase.command} {TASK_ID} after addressing the issues.
   ```
4. Stop processing this task. Continue to the next task in the sprint.

## Phase Gates

Declarative pre-flight gates for each phase. The orchestrator evaluates these
via `forge/tools/preflight-gate.cjs` **before** every subagent spawn. A failing
gate halts the loop for this task — no retry, no fall-through to the subagent,
no silent recovery. Gates are data, not prose: the grammar is defined in
`forge/tools/parse-gates.cjs` and validated by its test suite.

Grammar (one directive per line):
- `artifact <path> [min=<bytes>]` — file must exist and meet size floor. Path
  templates: `{sprint}` → sprintId, `{task}` → task suffix, `{bug}` → bugId.
- `require <field> <op> <value>` — predicate must hold. Ops: `==`, `!=`,
  `in [v1, v2, ...]`. Fields are dotted paths against the store record, e.g.
  `task.status`.
- `forbid <field> <op> <value>` — predicate must NOT hold.
- `after <phase> = <approved|revision>` — predecessor phase's stored verdict
  must match (read from the record by `read-verdict.cjs`, not from markdown).

```gates phase=plan
forbid task.status == committed
forbid task.status == abandoned
forbid task.status == blocked
forbid task.status == escalated
```

```gates phase=implement
artifact {engineering}/sprints/{sprint}/{task}/PLAN.md min=200
after review-plan = approved
forbid task.status == committed
forbid task.status == blocked
forbid task.status == escalated
```

```gates phase=review-plan
artifact {engineering}/sprints/{sprint}/{task}/PLAN.md min=200
forbid task.status == blocked
forbid task.status == escalated
```

```gates phase=review-code
after review-plan = approved
forbid task.status == blocked
forbid task.status == escalated
```

```gates phase=validate
after review-code = approved
forbid task.status == blocked
forbid task.status == escalated
```

```gates phase=approve
after review-code = approved
forbid task.status == blocked
forbid task.status == escalated
```

```gates phase=commit
after approve = approved
forbid task.status == blocked
forbid task.status == escalated
```

Adjusting a gate is a data change — edit the block above, regenerate workflows
on the user side via `/forge:update`, and the new gate takes effect on the next
orchestrator run. No code change required to relax or tighten a gate.

## Write-Boundary Contract

You MAY write Forge-owned JSON (`task.json`, `sprint.json`, `bug.json`,
events sidecars, `COLLATION_STATE.json`, `progress.log`) directly with the
`Write` or `Edit` tools. You do NOT need to route every write through
`store-cli` — the probabilistic layer is free to bypass deterministic tools.

However, **every write to a Forge-owned path is schema-validated at the
filesystem boundary** by the `PreToolUse` hook at
`hooks/validate-write.js`. A malformed write is rejected with a message
naming the offending field and pointing at the relevant
`forge/schemas/<kind>.schema.json`. Fix the data and retry — do NOT try to
disable the hook.

`store-cli` is still the most convenient path (it handles ID allocation,
referential integrity, ghost-event semantics, and sidecar merging), but it
is one route among several. The schema invariant is preserved whichever
route you take.

**Emergency bypass.** For operator-driven repair, set
`FORGE_SKIP_WRITE_VALIDATION=1` for a single turn. The hook will let the
write through and append an audit line to the affected sprint's
`progress.log`.

<!-- See _fragments/iron-laws.md for Iron Laws section structure guidance (orchestrate uses orchestrator-special deferral to generic-skills.md § Orchestrator Iron Laws) -->
## Iron Laws

<!-- Shared orchestrator laws live in generic-skills.md § Orchestrator Iron Laws. -->
> See `generic-skills.md § Orchestrator Iron Laws` for the six universal laws that apply to all orchestrators.

**Additional law specific to this pipeline:**

**YOU MUST NOT silently work around a blocker.** If a phase fails, a subagent
returns empty, a gate fails, or a verdict cannot be parsed, the orchestrator
MUST either retry once (for recoverable failures) or escalate to the human.
Skipping the phase, fabricating a result, assuming success without evidence,
or continuing with a degraded response is NEVER acceptable. Every failure MUST
produce a visible signal (✗ or ⚠) and a structured event. Silent continuation
is a violation of the Iron Laws.

## Error Recovery

- Test/build failure: pass error to Engineer revision workflow, retry once
- Verdict "Revision Required": enter revision loop (up to max_iterations)
- Subagent empty/crash/timeout response: retry once with simplified prompt
  (strip summary and architecture blocks). Escalate on second failure.
  See Subagent Response Validation in the Execution Algorithm.
- Subagent non-zero exit code (not read-verdict): same as above — retry
  once, escalate on second failure. The crash reason is captured in the
  escalation event notes.
- Verdict malformed or missing: escalate to human immediately. Never guess.
- Revision loop exhaustion: escalate to human immediately. Never approve
  to unblock.
- Gate failure (preflight): escalate to human. No retry, no fall-through.
- Gate misconfiguration: escalate to human. No retry, no fall-through.
- Git hook failure: diagnose, fix, create new commit
- Merge conflict: escalate to human
- Task status is blocked or escalated: skip the task entirely. Do not
  attempt any phase on it.

## Event Emission

<!-- See _fragments/event-emission-schema.md for canonical contract -->
> See `_fragments/event-emission-schema.md` for the actor split (subagent
> writes judgement-only SUMMARY; orchestrator composes the canonical event
> from runtime telemetry + SUMMARY and emits it), and
> `_fragments/event-vocabulary.md` § Task pipeline / § Sprint grain for the
> canonical phase→`type` token tables. When setting `type` on a phase or
> sprint event, the token MUST come from those tables.

The **orchestrator** is the only actor that calls `store-cli emit` for phase
events. Phase subagents write `{PHASE}-SUMMARY.json` and return. After each
subagent returns, the orchestrator:

1. Captures the subagent's runtime attribution (`model`, `provider`, token
   usage) from the runtime stream.
2. Records bracketed wall times around the spawn call (`startTimestamp`,
   `endTimestamp`, `durationMinutes`).
3. Reads the SUMMARY for the judgement blob (`verdict`, `notes`, `findings`).
4. Composes the canonical event with `eventId`, `taskId`, `sprintId`, `role`,
   `action`, `phase`, `iteration` from its own task state and `tokenSource:
   "reported"` when the runtime surfaced usage.
5. Calls `forge_store({ command: "emit", args: ["{sprintId}", '{event-json}'] })`
   with the complete record.

Do not include hardcoded example `model` or `provider` strings in the
generated orchestrator prose — they are the seed of LLM hallucination.
Refer subagents to `.forge/schemas/event.schema.json` instead.

<!-- See _fragments/generation-instructions.md for Generation Instructions template (orchestrate uses orchestrator-special long-form prose — cannot be reduced to standard subsections) -->
## Generation Instructions
- Fill in concrete test/build/lint commands from .forge/config.json
- Reference generated workflows by exact filename in .forge/workflows/
- Include stack-specific gate checks
- Use the Execution Algorithm above verbatim — do not paraphrase or summarise it
- `spawn_subagent` = Agent tool call. Each phase invocation MUST use the Agent tool with
  the exact workflow filename and task ID in the prompt. Never invoke phases inline.
- **Model dispatch uses cluster detection.** The generated workflow must include
  the cluster detection block (reading `ANTHROPIC_DEFAULT_*_MODEL` env vars) and
  the ROLE_TIER mapping table. On single clusters, omit `model` on Agent spawns
  (subagents inherit the parent model). On tiered clusters, pass `model=tier`
  based on the role-to-tier mapping. Only override this for per-phase `model`
  fields from `config.pipelines`.
  Do NOT generate a "Model Assignments" table — the Model Resolution section
  above is the single source of truth.
- **Include the sidecar merge pattern.** After each subagent returns, run
  `forge_store({ command: "merge-sidecar", args: ["{sprintId}", "{eventId}"] })` to merge token fields from the
  sidecar into the canonical event and delete the sidecar. If the sidecar does not
  exist (merge-sidecar returns an error), treat as non-fatal and emit the event without token
  fields (graceful fallback — no error).
- **Include the role-to-noun mapping table.** The generated orchestrator MUST include
  a `ROLE_TO_NOUN` dictionary (or equivalent in the host language) that maps every
  pipeline phase role to a noun-based persona identifier. This table is used for
  persona and skill file lookups, not for display. Example:

  | Role | Noun | Persona File | Skill File |
  |------|------|-------------|------------|
  | `plan` | `engineer` | `.forge/personas/engineer.md` | `.forge/skills/engineer-skills.md` |
  | `implement` | `engineer` | `.forge/personas/engineer.md` | `.forge/skills/engineer-skills.md` |
  | `review-plan` | `supervisor` | `.forge/personas/supervisor.md` | `.forge/skills/supervisor-skills.md` |
  | `review-code` | `supervisor` | `.forge/personas/supervisor.md` | `.forge/skills/supervisor-skills.md` |
  | `validate` | `qa-engineer` | `.forge/personas/qa-engineer.md` | `.forge/skills/qa-engineer-skills.md` |
  | `approve` | `architect` | `.forge/personas/architect.md` | `.forge/skills/architect-skills.md` |
  | `commit` | `engineer` | `.forge/personas/engineer.md` | `.forge/skills/engineer-skills.md` |
  | `writeback` | `collator` | `.forge/personas/collator.md` | `.forge/skills/collator-skills.md` |

  Generated lookups must use `{persona_noun}.md` and `{persona_noun}-skills.md`,
  never `{phase.role}.md` or `{phase.role}-skills.md`.
- **Phase banners are orchestrator-owned.** The generated orchestrator MUST NOT include
  a "Your first action — run this banner command" instruction in subagent prompts.
  The orchestrator displays the badge before spawning and the exit signal after return;
  subagents do not display banners. Instead, include progress reporting instructions
  in the subagent prompt with the agent name, progress log path, and banner key.
- **Include the progress IPC pattern.** Each generated orchestrator MUST:
  1. Clear the progress log at task start: `forge_store({ command: "progress-clear", args: ["{sprintId}`"] })
  2. Compute the agent name before each spawn: `{taskId}:{persona_noun}:{phase.role}:{iteration}`
  3. Start a Monitor on the progress log before each subagent spawn
  4. Include progress reporting instructions in the subagent prompt (agent name,
     progress log path, banner key, and `store-cli progress` command examples)
  5. Stop the Monitor after the subagent returns
  6. Display phase-exit signals after each phase completes (see Phase-Exit Signals section)
- **Include phase-exit signals.** After each subagent returns (and after sidecar
  merge and event emission), the generated orchestrator MUST print the appropriate
  exit signal: `✓` for completed/approved, `↻` for revision required (with iteration
  count), `⚠` for escalated.
- **Include the context pack injection.** Before spawning each subagent, the
  generated orchestrator MUST read `.forge/cache/context-pack.md` (if it exists)
  and inline it into the subagent prompt under the heading
  `### Architecture context (summary — full docs available at paths listed below)`.
  If the pack is absent, omit this block silently — the subagent falls back to
  reading architecture docs directly. This is the mechanism that replaces per-phase
  `Read engineering/architecture/stack.md` calls with a single cached summary.
  Subagents instructed by this block should read full docs **only** when the
  summary is insufficient.
- **Include post-phase /compact calls with state-ledger discipline.** After each
  phase-exit signal (for every non-escalation outcome), the generated orchestrator
  MUST:
  1. Print a checkpoint line: `[checkpoint] task={task_id} sprint={sprint_id} phase_index={i} iterations={iteration_counts}`
  2. Run `/compact` to free orchestrator context before the next phase.

  The `[checkpoint]` line IS the state ledger. It carries the loop bookkeeping
  (task ID, sprint ID, current phase index, iteration counts) that `/compact` must
  preserve verbatim in its summary. It is not an optional debug breadcrumb — it is
  the one line the orchestrator must carry forward through each compaction boundary.

  Raw tool output (bash stdout, subagent return blobs, multi-KB phase responses)
  is shed at every `/compact` call. The generated orchestrator MUST NOT retain
  verbatim tool output or subagent return text between phases — only the
  checkpoint ledger line and on-disk artifact pointers survive compaction.

  The compact summary MUST contain: the checkpoint line verbatim, the task/sprint
  IDs, and the current phase index. The compact summary MUST NOT contain: raw
  subagent return text, bash stdout blobs, or multi-line phase responses.

  Do NOT compact on escalation (verdict_malformed or max-iterations break paths) —
  the human needs the full uncompacted context to diagnose and resume.

## Friction Emit

When the Orchestrator detects skill friction during orchestrate-task — a referenced skill is unused, fails on invocation, is missing from the registry, has gone stale relative to current architecture, or is redundant with another skill — emit a `friction` event so `/forge:rebuild --enrich` (phase 2) can act on the signal. This is the writer side of the channel whose reader landed in S13-T08; the reader is empty without these emits.

**Trigger conditions** (set `issue` to the matching token):

| Token              | When to emit                                                                     |
|--------------------|----------------------------------------------------------------------------------|
| `skill_unused`     | A skill listed in the persona's skill block was loaded but never consulted.      |
| `skill_failed`     | A skill was consulted but its guidance produced an error or required correction. |
| `skill_missing`    | The workflow needed guidance the available skills did not cover.                 |
| `skill_stale`      | A skill's guidance contradicts current architecture / supersedes its own advice. |
| `skill_redundant`  | Two skills provided overlapping or conflicting guidance for the same decision.   |

**Two flavours of friction in orchestrate-task:**

1. **Subagent-experienced friction** (the persona running plan / implement /
   validate / etc. detects skill friction). The subagent records the signal
   via `node .forge/tools/friction-emit.cjs --workflow {wf} --persona {p}
   --issue {token} [--subkind {token}] [--evidence '{...}']`, which appends a
   judgement-only record to `.forge/cache/FRICTION-{wf}.jsonl`. After the
   subagent returns, the orchestrator drains this file, stamps the
   subagent's captured runtime attribution (model, provider, usage, wall
   times, eventId) onto each record, and emits the resulting events via
   `store-cli emit` as event type `"friction"`. The orchestrator truncates
   the file only after all emits succeed.

2. **Orchestrator-experienced friction** (spawn failure, sidecar missing,
   FSM rejection, verdict malformed). The orchestrator emits inline using
   its own model/provider attribution (`persona: "orchestrator"`,
   `workflow: "orchestrate"`, `phase: "orchestrate"`). Same `store-cli emit`
   path; no example record is reproduced here because the orchestrator
   owns the field values — consult `.forge/schemas/event.schema.json` for
   the required shape.

The schema enforces `{workflow, persona, issue}` as required when
`type === "friction"`. `subkind` is the frozen enum
`skill_unused|skill_failed|skill_missing|skill_stale|skill_redundant` or
experimental `^x_[a-z_]+$`. Emit one record per distinct friction signal
— do not coalesce.

The generated `orchestrate_task.md` MUST carry this section verbatim —
`/forge:rebuild --enrich` (phase 2) greps for it.
