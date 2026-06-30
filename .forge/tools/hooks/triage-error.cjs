#!/usr/bin/env node
// Forge PostToolUse hook — error triage
//
// Fires after every Bash tool call. If the command is Forge-related and
// exits non-zero, injects an additionalContext prompt asking Claude to offer
// the user the option to file a bug via /forge:report-bug.
//
// Uses only Node.js built-ins — no npm dependencies required.

'use strict';

// FORGE_PATTERNS — inline copy of forge command recognition patterns.
// Canonical source: hooks/lib/common.cjs:FORGE_COMMAND_PATTERNS (H-1d, FORGE-S25-T08).
// This file intentionally does NOT require hooks/lib/common.cjs because
// build-payload.cjs bundles hooks/*.cjs but excludes hooks/lib/ (forge-cli
// bundle gap). When adding a new forge command, update BOTH this list AND
// hooks/lib/common.cjs:FORGE_COMMAND_PATTERNS.
const FORGE_PATTERNS = [
  /manage-config/,
  /\.forge\//,
  /CLAUDE_PLUGIN_ROOT/,
  /FORGE_ROOT/,
  /MANAGE_CONFIG/,
  /engineering\/tools\//,
  /forge:init/,
  /forge:health/,
  /forge:rebuild/,
  /forge:update/,
  /forge:add-pipeline/,
  /forge:add-task/,
  /forge:plan/,
  /forge:implement/,
  /forge:approve/,
  /forge:commit/,
  /forge:review/,
  /forge:new-sprint/,
  /forge:plan-sprint/,
  /forge:run-task/,
  /forge:run-sprint/,
  /forge:fix-bug/,
  /forge:retro/,
  /forge:check-agent/,
  /forge:report-bug/,
  // forge:enhance removed in v1.0 (T03) — absorbed into forge:rebuild --enrich
  // forge:collate removed from user-facing surface in v1.0 (T03) — internal tool only
  /forge:validate/,
  // forge:calibrate removed in v1.0 (T03) — absorbed into forge:health --fix (T04)
  // forge:materialize removed in v1.0 (T03) — fast-mode eliminated in T01
  /forge:remove/,
  /forge:search/,
  /forge:repair/,
  /forge:store-custodian/,
  /forge:config/,
  /forge:ask/,
  /forge:refresh-kb-links/,
  /store-cli\.cjs/,
  /validate-store\.cjs/,
];

function isForgeRelated(command) {
  return FORGE_PATTERNS.some(p => p.test(command));
}

// The hook must never exit non-zero — a hook failure would surface as noise to
// the user and defeat the purpose of silent triage. Wrap everything so that any
// unexpected error causes a clean no-op exit instead of a crash report.
process.on('uncaughtException', () => process.exit(0));

let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { raw += chunk; });
process.stdin.on('end', () => {
  try {
    const event = JSON.parse(raw);

    if (event.tool_name !== 'Bash') return;

    const command = event.tool_input?.command || '';
    const exitCode = event.tool_response?.exitCode;

    // Only act on non-zero exits for Forge-related commands.
    if (!isForgeRelated(command)) return;
    if (exitCode === 0 || exitCode === undefined) return;

    const stderr = event.tool_response?.stderr || '';
    const output = event.tool_response?.output || '';
    const errorSnippet = (stderr || output).split('\n').slice(0, 3).join(' ').trim();

    const context =
      `FORGE_ERROR_TRIAGE: A Forge command just failed (exit ${exitCode}). ` +
      (errorSnippet ? `First error line: "${errorSnippet}". ` : '') +
      `Tell the user what went wrong, then ask: ` +
      `"Would you like to file this as a Forge bug to help improve the tool? ` +
      `Run /forge:report-bug and I will pre-fill the report from this conversation."`;

    process.stdout.write(JSON.stringify({
      hookSpecificOutput: {
        hookEventName: 'PostToolUse',
        additionalContext: context,
      },
    }) + '\n');
  } catch {
    // Swallow all errors — this hook must never become the problem.
  }
});
