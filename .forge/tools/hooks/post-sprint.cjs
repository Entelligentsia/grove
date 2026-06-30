#!/usr/bin/env node
'use strict';

// Forge hook: post-sprint
// PostToolUse hook that fires after a sprint retrospective completes
// (`collate.cjs <SPRINT_ID> --purge-events` where the ID matches the
// `*-S\d+` shape) and invokes /forge:rebuild --enrich --phase 2.
//
// The sprint-ID shape gate ensures bug-fix invocations like
// `collate.cjs FORGE-B07 --purge-events` (which run after every successful
// bug fix) do NOT trigger sprint-level enhancement.
//
// Idempotency: a sprint-specific sentinel file under .forge/cache/ prevents
// a second firing for the same sprint. Different sprints fire independently.
//
// Fail-open: any internal error → stderr warning + exit 0. Hook failure
// must never block sprint completion.

process.on('uncaughtException', (err) => {
  try { process.stderr.write(`forge post-sprint: internal error (fail-open): ${err.message}\n`); } catch (_) {}
  process.exit(0);
});

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const { resolveForgePaths, logSwallowedError } = require('./lib/common.cjs');

let raw = '';
try { raw = fs.readFileSync(0, 'utf8'); } catch (_) { process.exit(0); }

let envelope;
try { envelope = JSON.parse(raw); } catch (_) { process.exit(0); }

if (!envelope || envelope.tool_name !== 'Bash') process.exit(0);

const exitCode = envelope.tool_response && envelope.tool_response.exitCode;
if (exitCode !== 0 && exitCode !== undefined) process.exit(0);

const command = (envelope.tool_input && envelope.tool_input.command) || '';

// Trigger: collate.cjs <PREFIX-S\d+> --purge-events. The capture group
// extracts the sprint ID for the sentinel and event payload. The
// `*-S\d+` shape rules out bug IDs like FORGE-B07.
const TRIGGER_RX = /\bcollate\.cjs\s+(\S*-S\d+)\s+--purge-events\b/;
const m = command.match(TRIGGER_RX);
if (!m) process.exit(0);

const detectedSprintId = m[1];

const paths = resolveForgePaths();
if (!paths) process.exit(0);

try { fs.mkdirSync(paths.cacheDir, { recursive: true }); } catch (_) {}
const sentinel = path.join(paths.cacheDir, `post-sprint-${detectedSprintId}-enhancement-triggered`);
if (fs.existsSync(sentinel)) process.exit(0);

const iso = new Date().toISOString().replace(/[:.]/g, '');
const eventId = `${iso}_enhance_trigger_phase2_${process.pid}_${process.hrtime.bigint()}`;

const now = new Date().toISOString();
const event = {
  eventId,
  taskId: 'enhancement',
  sprintId: 'enhancement',
  role: 'hook',
  action: 'enhancement-trigger',
  phase: 'post-sprint',
  iteration: 1,
  startTimestamp: now,
  endTimestamp: now,
  durationMinutes: 0,
  model: 'n/a',
  notes: JSON.stringify({
    triggerSource: 'post-sprint',
    targetPhase: 2,
    detectedSprintId,
  }),
};

try {
  if (paths.forgeRoot) {
    const eventsBucket = path.join(paths.eventsRoot, 'enhancement');
    fs.mkdirSync(eventsBucket, { recursive: true });
    const cli = path.join(paths.forgeRoot, 'tools', 'store-cli.cjs');
    if (fs.existsSync(cli)) {
      spawnSync('node', [cli, 'emit', 'enhancement', JSON.stringify(event)], {
        cwd: process.cwd(),
        timeout: 3000,
        encoding: 'utf8',
      });
    }
  }
} catch (emitErr) { logSwallowedError('post-sprint:emit', emitErr, process.env.CLAUDE_PLUGIN_DATA); }

try { fs.writeFileSync(sentinel, '', 'utf8'); } catch (sentinelErr) { logSwallowedError('post-sprint:sentinel', sentinelErr, process.env.CLAUDE_PLUGIN_DATA); }

try {
  process.stdout.write(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PostToolUse',
      additionalContext:
        `Forge: sprint ${detectedSprintId} retrospective has completed. Run /forge:rebuild --enrich --phase 2 to propose KB enhancements derived from the sprint's lessons (proposals are written for review, not auto-applied).`,
    },
  }) + '\n');
} catch (outputErr) { logSwallowedError('post-sprint:output', outputErr, process.env.CLAUDE_PLUGIN_DATA); }

process.exit(0);
