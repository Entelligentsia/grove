export const meta = {
  name: 'wfl:init',
  description: 'Code-orchestrated /forge:init LLM half — parallel discovery fan-out → config-writer (Phase 1 Collect), parallel KB-doc fan-out → index → context (Phase 2 Discover), deterministic materialize (Phase 3), content-register (Phase 4). JS holds the phase index, verify gates, retry caps, and fan-out; subagents execute the phase rulebooks. Args: { forgeRoot, kbFolder, startPhase, createClaudeMd, isoTimestamp, rawArguments }.',
  whenToUse: "Run the LLM-orchestrated half of /forge:init after `4ge init claude .` has bootstrapped the project structure. Dispatch by name: workflow('wfl:init', { forgeRoot, kbFolder, startPhase, createClaudeMd, isoTimestamp, rawArguments }).",
  phases: [
    { title: 'Collect',     detail: 'parallel() 5 discovery agents scan codebase domains; config-writer agent merges findings and writes config + init-progress.json; verify-phase gate with one retry cap' },
    { title: 'Discover',    detail: 'gate+scaffold agent verifies phase 1; parallel() 7 KB-doc agents generate architecture docs with JS-held retry-once; sequential index + context agents close the phase' },
    { title: 'Materialize', detail: 'single haiku agent runs deterministic substitute-placeholders + generation-manifest + build-overlay; Phase 3 verify failure is a hard halt (no retry — rebuild/restart)' },
    { title: 'Register',    detail: 'single haiku agent runs content-register steps; CLAUDE.md creation gated on args.createClaudeMd === true; returns pendingActions for Tomoshibi' },
    { title: 'Report',      detail: 'return structured result { ok, lastPhase, stack, skillMatches, counts, confidence, pendingActions, failure? } for the command wrapper to render' },
  ],
};

// wfl:init — code-orchestrated LLM half of /forge:init
//
// Why a script: init is a deterministic 4-phase FSM with mechanical verify
// gates (verify-phase.cjs --phase N), bounded retries (max 1), fan-out steps
// (5 discovery scans, 7 KB docs), and escalate-don't-continue semantics.
// JS holds the phase index, the verify-gate routing, the retry counters, and
// the fan-out; subagents only execute one phase rulebook each.
//
// WORKFLOW-API CONTRACT (forge#112 — field failure; enforced by
// wfl-drivers-parse.test.cjs):
//   - Exactly ONE export: the meta literal. The body runs at top level in the
//     harness's async context (top-level await/return valid; args is a global).
//   - phase(title) takes ONLY a title — a callback second arg is silently
//     discarded. Phase bodies run inline after the phase() call.
//   - parallel() takes thunks: parallel([() => agent(...), ...]).
//   - agent(prompt, opts) — model goes in opts.model; structured results
//     REQUIRE opts.schema (otherwise agent() returns plain text and .ok
//     reads are undefined).
//
// CLI-FIRST BOOTSTRAP ADR (doc/decisions/cli-first-bootstrap.md):
//   `4ge init claude .` runs first (deterministic, zero tokens): scaffolds
//   .forge/ as a complete vendored Forge root (tools, schemas, hooks, init/,
//   .base-pack/, meta/) and installs THIS FILE into .claude/workflows/.
//   All rulebook reads use vendored .forge/init/... paths; discovery prompts
//   fall back to direct project analysis if a prompt file is absent.
//
// SIDE-EFFECT OWNERSHIP: this script has NO filesystem/shell access. Each
// per-phase subagent owns rulebook execution (artifact writes, checkpoint
// writes, verify runs). The JS driver holds ONLY control flow.
//
// Timestamps: the Workflow sandbox blocks timestamp minting (Date.now,
// Math.random, and the zero-arg Date constructor are all unavailable).
// The command wrapper supplies args.isoTimestamp.
//
// MODEL TIERING: generation/discovery → sonnet; deterministic gates and
// registration → haiku. No opus (init has no review/approve gates).

// ─────────────────────────────────────────────────────────────────────────────
// Schemas
// ─────────────────────────────────────────────────────────────────────────────

const DISCOVERY_SCHEMA = {
  type: 'object',
  properties: {
    domain:      { type: 'string', description: 'discovery domain name (stack|routing|processes|database|testing)' },
    findings:    { type: 'object', description: 'domain-specific structured findings' },
    confidence:  { type: 'number', description: '0–1 confidence in completeness of scan' },
    warnings:    { type: 'array',  items: { type: 'string' }, description: 'ambiguities or partial coverage notes' },
  },
  required: ['domain', 'findings', 'confidence'],
};

const KB_DOC_SCHEMA = {
  type: 'object',
  properties: {
    id:         { type: 'string',  description: 'KB doc id matching the phase-2 table row' },
    ok:         { type: 'boolean', description: 'true if doc written successfully' },
    confidence: { type: 'number',  description: '0–1 confidence in doc completeness' },
    error:      { type: 'string',  description: 'error message if ok=false' },
  },
  required: ['id', 'ok', 'confidence'],
};

const PHASE_RESULT_SCHEMA = {
  type: 'object',
  properties: {
    verifyExit:  { type: 'number',  description: 'exit code from verify-phase.cjs (0=pass, non-zero=fail)' },
    verifyError: { type: 'string',  description: 'stderr/stdout from failed verify run' },
    stack:       { type: 'string',  description: 'technology stack summary (Phase 1 output)' },
    skillMatches: {
      type: 'array',
      items: { type: 'string' },
      description: 'skill IDs matching project tech stack (from skill-recommendations.md)',
    },
    confidence: { type: 'number', description: '0–1 confidence' },
    ok: { type: 'boolean', description: 'true if phase completed and verify passed' },
  },
  required: ['verifyExit', 'ok'],
};

const OK_SCHEMA = {
  type: 'object',
  properties: {
    ok:    { type: 'boolean', description: 'true if all steps completed' },
    error: { type: 'string',  description: 'error message if ok=false' },
  },
  required: ['ok'],
};

const REGISTER_SCHEMA = {
  type: 'object',
  properties: {
    ok:             { type: 'boolean', description: 'true if all register steps completed' },
    error:          { type: 'string',  description: 'error message if ok=false' },
    pendingActions: { type: 'array', items: { type: 'string' }, description: 'orchestrator-owned follow-ups, e.g. ["refresh-kb-links"]' },
  },
  required: ['ok'],
};

// ─────────────────────────────────────────────────────────────────────────────
// Model tiering
// ─────────────────────────────────────────────────────────────────────────────

const ROLE_TIER = {
  'discovery':   'sonnet',
  'config':      'sonnet',
  'kb-doc':      'sonnet',
  'index':       'sonnet',
  'context':     'sonnet',
  'gate':        'haiku',
  'materialize': 'haiku',
  'register':    'haiku',
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

function halt(lastPhase, reason, extra) {
  return { ok: false, lastPhase, failure: reason, ...extra };
}

const VENDORED = 'Use only .forge/tools/ paths for all tool invocations (vendored-tools world).';

// ─────────────────────────────────────────────────────────────────────────────
// Main workflow body (top-level async context; top-level return is valid)
// ─────────────────────────────────────────────────────────────────────────────

const {
  forgeRoot,
  kbFolder       = 'engineering',
  startPhase     = 1,
  createClaudeMd = null,
  isoTimestamp,
  rawArguments   = '',
} = args || {};

// ─────────────────────────────────────────────────────────────────────────────
// Phase 1 — Collect (startPhase <= 1)
// ─────────────────────────────────────────────────────────────────────────────
let phase1Result;
if (startPhase <= 1) {
  phase('Collect');

  // Fan-out: 5 parallel discovery agents
  const DOMAINS = ['stack', 'routing', 'processes', 'database', 'testing'];
  const discoveryResults = await parallel(
    DOMAINS.map((domain) => () =>
      agent(`
You are a codebase discovery agent for the "${domain}" domain.
1. Read \`.forge/init/discovery/discover-${domain}.md\` if it exists and follow
   its instructions against the current project.
2. If that file does NOT exist, read \`.forge/init/phases/phase-1-collect.md\`
   Step 2 for context, then perform a best-effort "${domain}" discovery of the
   project directly (read manifests, source files, configs) and add a warning
   noting the missing discovery prompt file.
Set domain="${domain}" in your structured output. ${VENDORED}
`, { model: ROLE_TIER['discovery'], label: `discover:${domain}`, phase: 'Collect', schema: DISCOVERY_SCHEMA })
    )
  );

  // Config-writer agent: merge findings, write config, verify Phase 1
  const configResult = await agent(`
You are the Forge init config-writer agent. You have received the following
discovery findings from 5 parallel discovery agents:

${JSON.stringify(discoveryResults.filter(Boolean), null, 2)}

Execute the Phase 1 Collect rulebook steps:
1. Read \`.forge/init/phases/phase-1-collect.md\` for the full step list.
2. Call \`node .forge/tools/manage-config.cjs\` to write the config. If kbFolder
   is non-default, set paths.engineering="${kbFolder}". Set mode=full.
3. Compute skill-recommendation matches from .forge/meta/skill-recommendations.md
   and the output of \`node .forge/tools/list-skills.js\`. Do NOT install —
   report matches in skillMatches only.
4. Write .forge/init-progress.json: { "lastPhase": 1, "timestamp": "${isoTimestamp}" }.
5. Run: node .forge/tools/verify-phase.cjs --phase 1
6. In your structured output set verifyExit=<exit code>,
   verifyError=<stderr if non-zero>, stack=<one-line stack summary>,
   skillMatches=[<matched skill ids>], confidence=<0-1>, ok=(verifyExit===0).
${VENDORED}
kbFolder="${kbFolder}", isoTimestamp="${isoTimestamp}".
`, { model: ROLE_TIER['config'], label: 'config-writer', phase: 'Collect', schema: PHASE_RESULT_SCHEMA });

  // Verify routing: one retry on failure
  if (configResult && configResult.verifyExit !== 0) {
    const retryResult = await agent(`
Phase 1 of Forge init verify failed. Error:
${configResult.verifyError || '(no error text)'}

Read the error carefully. Fix the config by re-running
\`node .forge/tools/manage-config.cjs\` with the correct values, then
re-run \`node .forge/tools/verify-phase.cjs --phase 1\`.
In your structured output set verifyExit=<exit code>, ok=(verifyExit===0).
${VENDORED}
`, { model: ROLE_TIER['config'], label: 'config-writer:retry', phase: 'Collect', schema: PHASE_RESULT_SCHEMA });
    if (!retryResult || retryResult.verifyExit !== 0) {
      return halt(1, 'Phase 1 verify failed after retry', {
        verifyError: retryResult ? retryResult.verifyError : 'retry agent returned null',
      });
    }
    phase1Result = { ...configResult, ...retryResult };
  } else if (configResult) {
    phase1Result = configResult;
  } else {
    return halt(1, 'config-writer agent returned null');
  }

  if (!phase1Result.ok) {
    return halt(1, 'Phase 1 failed', {
      verifyError: phase1Result.verifyError,
      stack: phase1Result.stack,
    });
  }
} else {
  phase1Result = { ok: true, lastPhase: 1, skipped: true };
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 2 — Discover (startPhase <= 2)
// ─────────────────────────────────────────────────────────────────────────────
let phase2Result;
const KB_DOC_IDS = [
  'architecture/stack',
  'architecture/processes',
  'architecture/routing',
  'architecture/database',
  'architecture/testing',
  'business-domain/domain-model',
  'business-domain/domain-concepts',
];

if (startPhase <= 2) {
  phase('Discover');

  // Gate+scaffold agent: verify Phase 1 passed, mkdir scaffold
  const gateResult = await agent(`
You are the Forge init Phase 2 gate agent. Execute these steps in order:
1. Run: node .forge/tools/verify-phase.cjs --phase 1
   If exit non-zero, set ok=false and error=<stderr> in your output and stop.
2. Read \`.forge/init/phases/phase-2-discover.md\` Step 2 (scaffold mkdir
   commands) and execute them. Create the KB directory structure under ${kbFolder}/.
3. Set ok=true.
${VENDORED}
`, { model: ROLE_TIER['gate'], label: 'phase2-gate', phase: 'Discover', schema: OK_SCHEMA });

  if (!gateResult || !gateResult.ok) {
    return halt(2, 'Phase 2 gate failed — Phase 1 verify did not pass', {
      verifyError: gateResult ? gateResult.error : 'gate agent returned null',
    });
  }

  // Fan-out: 7 parallel KB-doc agents
  const kbDocPrompt = (docId) => `
You are a Forge KB-doc generation agent. Your doc id is: "${docId}".
1. Read \`.forge/init/phases/phase-2-discover.md\` for the full doc spec for "${docId}".
2. If \`.forge/init/generation/generate-kb-doc.md\` exists, read it for the
   generation rulebook; otherwise follow the doc spec from step 1 directly.
3. Generate the doc using all available project context (including
   .forge/config.json written in Phase 1).
4. Write the doc to the correct path under ${kbFolder}/.
5. Set id="${docId}", ok=<true if written>, confidence=<0-1>, error=<if not ok>.
${VENDORED}
`;

  const kbDocResults = await parallel(
    KB_DOC_IDS.map((docId) => () =>
      agent(kbDocPrompt(docId), { model: ROLE_TIER['kb-doc'], label: `kb-doc:${docId}`, phase: 'Discover', schema: KB_DOC_SCHEMA })
    )
  );

  // JS-held retry-once for any failed KB-doc
  const failedDocs = KB_DOC_IDS.filter((docId, i) => !kbDocResults[i] || !kbDocResults[i].ok);
  if (failedDocs.length > 0) {
    const retryResults = await parallel(
      failedDocs.map((docId) => () =>
        agent(`
KB-doc "${docId}" failed on first attempt. Retry it:
${kbDocPrompt(docId)}
`, { model: ROLE_TIER['kb-doc'], label: `kb-doc-retry:${docId}`, phase: 'Discover', schema: KB_DOC_SCHEMA })
      )
    );
    const stillFailed = failedDocs.filter((docId, i) => !retryResults[i] || !retryResults[i].ok);
    if (stillFailed.length > 0) {
      return halt(2, `KB-doc generation failed after retry for: ${stillFailed.join(', ')}`, {
        failedDocs: stillFailed.join(', '),
      });
    }
  }

  // Sequential index agent (after all leaf docs — real data dependency)
  const indexResult = await agent(`
You are the Forge init index agent. All 7 KB architecture docs have been generated.
Read \`.forge/init/phases/phase-2-discover.md\` Step 4 (index files).
Generate the 3 INDEX files:
  - ${kbFolder}/architecture/INDEX.md
  - ${kbFolder}/business-domain/INDEX.md
  - ${kbFolder}/INDEX.md
Set ok=true when done, or ok=false with error=<message>.
${VENDORED}
`, { model: ROLE_TIER['index'], label: 'index', phase: 'Discover', schema: OK_SCHEMA });

  if (!indexResult || !indexResult.ok) {
    return halt(2, 'Index agent failed', { error: indexResult ? indexResult.error : 'null result' });
  }

  // Context agent: project-context.json + calibration + init-progress.json + verify
  const contextResult = await agent(`
You are the Forge init context agent. KB docs and index files are complete.
Execute \`.forge/init/phases/phase-2-discover.md\` Steps 5–6:
5. Write project-context.json (combined structured context from all discovery
   findings). Write calibration baseline.
6. Write .forge/init-progress.json: { "lastPhase": 2, "timestamp": "${isoTimestamp}" }.
   Run: node .forge/tools/verify-phase.cjs --phase 2 --kb-path "${kbFolder}"
Set verifyExit=<exit code>, verifyError=<stderr if non-zero>, ok=(verifyExit===0).
${VENDORED}
`, { model: ROLE_TIER['context'], label: 'context', phase: 'Discover', schema: PHASE_RESULT_SCHEMA });

  // Verify routing: one retry on failure
  if (contextResult && contextResult.verifyExit !== 0) {
    const verifyRetry = await agent(`
Phase 2 of Forge init verify failed. Error:
${contextResult.verifyError || '(no error text)'}
Read the error, fix the missing or malformed outputs, re-run
\`node .forge/tools/verify-phase.cjs --phase 2 --kb-path "${kbFolder}"\`.
Set verifyExit=<exit code>, ok=(verifyExit===0).
${VENDORED}
`, { model: ROLE_TIER['context'], label: 'context:retry', phase: 'Discover', schema: PHASE_RESULT_SCHEMA });
    if (!verifyRetry || verifyRetry.verifyExit !== 0) {
      return halt(2, 'Phase 2 verify failed after retry', {
        verifyError: verifyRetry ? verifyRetry.verifyError : 'retry returned null',
      });
    }
    phase2Result = verifyRetry;
  } else if (contextResult) {
    phase2Result = contextResult;
  } else {
    return halt(2, 'context agent returned null');
  }

  if (!phase2Result.ok) {
    return halt(2, 'Phase 2 failed', { verifyError: phase2Result.verifyError });
  }
} else {
  phase2Result = { ok: true, lastPhase: 2, skipped: true };
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 3 — Materialize (startPhase <= 3)
// ─────────────────────────────────────────────────────────────────────────────
let phase3Result;
if (startPhase <= 3) {
  phase('Materialize');

  // Single haiku agent — deterministic shell steps
  const materializeResult = await agent(`
You are the Forge init materialize agent. Read
\`.forge/init/phases/phase-3-materialize.md\` for the rulebook, then execute
these deterministic steps:
1. Run: node .forge/tools/build-init-context.cjs
2. Run: node .forge/tools/substitute-placeholders.cjs
3. Record generation-manifest entries for all materialized assets.
4. Run a build-overlay smoke check: node .forge/tools/build-overlay.cjs --check
5. Write .forge/init-progress.json: { "lastPhase": 3, "timestamp": "${isoTimestamp}" }
6. Run: node .forge/tools/verify-phase.cjs --phase 3
Set verifyExit=<exit code>, verifyError=<stderr if non-zero>, ok=(verifyExit===0).
${VENDORED}
IMPORTANT: If verify-phase exits non-zero, report it faithfully — do NOT retry.
Phase 3 verify failure is a hard halt. The rulebook says: rebuild or restart init.
`, { model: ROLE_TIER['materialize'], label: 'materialize', phase: 'Materialize', schema: PHASE_RESULT_SCHEMA });

  // Phase 3: hard halt on verify failure (no retry)
  if (!materializeResult || materializeResult.verifyExit !== 0) {
    const verifyError = materializeResult ? materializeResult.verifyError : 'materialize agent returned null';
    return halt(3, [
      'Phase 3 (Materialize) verify failed. This is a hard halt — no retry.',
      'Per the phase-3-materialize.md rulebook: you must rebuild or restart /forge:init.',
      'Run: /forge:init (or `4ge init claude .` to re-scaffold, then /forge:init).',
      `Verify error: ${verifyError}`,
    ].join(' '), { verifyError, rebuild: true, restart: true });
  }

  phase3Result = materializeResult;
} else {
  phase3Result = { ok: true, lastPhase: 3, skipped: true };
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 4 — Register (startPhase <= 4)
// ─────────────────────────────────────────────────────────────────────────────
let phase4Result;
if (startPhase <= 4) {
  phase('Register');

  const registerResult = await agent(`
You are the Forge init register agent. Execute the content-register steps:
1. Read \`.forge/init/phases/phase-4-register.md\` for the full step list.
2. Execute steps 1–10 and step 12 verbatim.
3. Step 13 (CLAUDE.md file creation): ${createClaudeMd === true
      ? 'createClaudeMd=true — execute this step (create the CLAUDE.md file).'
      : 'createClaudeMd is not true — SKIP step 13 (the prompt was hoisted to the wrapper).'}
4. Step 11 (Tomoshibi forge:refresh-kb-links): DO NOT execute. This is
   orchestrator-owned. Set pendingActions=["refresh-kb-links"] and the
   wrapper (init.md) will run it post-workflow.
5. Delete .forge/init-progress.json (phase 4 complete — no resume needed).
6. Set ok=true, pendingActions=["refresh-kb-links"].
${VENDORED}
createClaudeMd=${JSON.stringify(createClaudeMd)}, isoTimestamp="${isoTimestamp}".
`, { model: ROLE_TIER['register'], label: 'register', phase: 'Register', schema: REGISTER_SCHEMA });

  if (!registerResult || !registerResult.ok) {
    return halt(4, registerResult ? (registerResult.error || 'Phase 4 failed') : 'register agent returned null');
  }
  phase4Result = registerResult;
} else {
  phase4Result = { ok: true, lastPhase: 4, skipped: true };
}

// ─────────────────────────────────────────────────────────────────────────────
// Report
// ─────────────────────────────────────────────────────────────────────────────
phase('Report');

return {
  ok: true,
  lastPhase: 4,
  stack:          phase1Result && phase1Result.stack,
  skillMatches:   phase1Result && phase1Result.skillMatches,
  counts: {
    kbDocs:    KB_DOC_IDS.length,
    workflows: 4,  // wfl-run-task, wfl-run-sprint, wfl-fix-bug, wfl-init (installed by the CLI half)
    commands:  3,  // run-task, run-sprint, fix-bug (init.md is the wrapper, not a base-pack command)
  },
  confidence:     phase1Result && phase1Result.confidence,
  pendingActions: (phase4Result && phase4Result.pendingActions) || ['refresh-kb-links'],
};
