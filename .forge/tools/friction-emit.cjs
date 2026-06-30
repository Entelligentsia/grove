#!/usr/bin/env node
'use strict';
// friction-emit.cjs — Plan-11 / Slice 2 / #23.
//
// Subagent-side friction recorder. Appends ONE judgement-only record to
// `.forge/cache/FRICTION-{workflow}.jsonl`. The orchestrator drains this
// file at phase-end, stamps runtime attribution (model/provider/usage/
// timestamps/eventId) onto each record, and emits the resulting events
// via store-cli.
//
// This tool intentionally:
//   - requires only judgement fields (workflow, persona, issue)
//   - REFUSES any runtime-attribution flag a subagent might try to pass
//     (model, provider, eventId, timestamps, tokens, …) — those belong
//     to the orchestrator, not the LLM.
//
// CLI:
//   node friction-emit.cjs \
//     --workflow implement \
//     --persona  engineer \
//     --issue    skill_unused \
//     [--subkind  skill_unused] \
//     [--evidence '{"trajectory_excerpt":"...","tool_errors":["..."]}']

const fs = require('node:fs');
const path = require('node:path');
const { ensureDir } = require('./lib/fsutil.cjs');

const REQUIRED = ['workflow', 'persona', 'issue'];
const OPTIONAL = ['subkind', 'evidence'];
const FORBIDDEN_RUNTIME = [
  'model', 'provider', 'eventId',
  'startTimestamp', 'endTimestamp', 'timestamps',
  'inputTokens', 'outputTokens', 'tokens', 'tokenSource',
  'iteration', 'durationMinutes',
];

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) {
      throw new Error(`unexpected positional argument: ${a}`);
    }
    const key = a.slice(2);
    const val = argv[i + 1];
    if (val === undefined || val.startsWith('--')) {
      throw new Error(`flag --${key} requires a value`);
    }
    out[key] = val;
    i++;
  }
  return out;
}

function fail(msg) {
  process.stderr.write(`friction-emit: ${msg}\n`);
  process.exit(2);
}

function main(argv) {
  let args;
  try {
    args = parseArgs(argv);
  } catch (err) {
    fail(err.message);
  }

  for (const key of Object.keys(args)) {
    if (FORBIDDEN_RUNTIME.includes(key)) {
      fail(
        `--${key} is a runtime-attribution field. The LLM never writes runtime ` +
        `facts; the orchestrator stamps these at drain time. Refused.`,
      );
    }
    if (!REQUIRED.includes(key) && !OPTIONAL.includes(key)) {
      fail(`unknown flag --${key}`);
    }
  }

  for (const key of REQUIRED) {
    if (!args[key] || args[key].length === 0) {
      fail(`--${key} is required`);
    }
  }

  const record = {
    type:     'friction',
    workflow: args.workflow,
    persona:  args.persona,
    issue:    args.issue,
  };
  if (args.subkind) record.subkind = args.subkind;
  if (args.evidence !== undefined) {
    try {
      record.evidence = JSON.parse(args.evidence);
    } catch (err) {
      fail(`--evidence is not valid JSON: ${err.message}`);
    }
  }

  const cacheDir = path.join(process.cwd(), '.forge', 'cache');
  ensureDir(cacheDir);
  const target = path.join(cacheDir, `FRICTION-${args.workflow}.jsonl`);
  fs.appendFileSync(target, `${JSON.stringify(record)}\n`, 'utf8');
  process.stdout.write(`${target}\n`);
}

if (require.main === module) {
  main(process.argv.slice(2));
}

module.exports = { _parseArgs: parseArgs, FORBIDDEN_RUNTIME, REQUIRED, OPTIONAL };
