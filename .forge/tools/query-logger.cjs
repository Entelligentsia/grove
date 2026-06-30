#!/usr/bin/env node
'use strict';
// query-logger.cjs — PostToolUse hook: logs store-cli query invocations
// Reads TOOL_INPUT env var, appends entry to .forge/store/query-log.jsonl

const fs = require('fs');
const path = require('path');

try {
  const toolInput = process.env.TOOL_INPUT || '';
  if (!/store-cli(?:\.cjs)?["']?\s+query/.test(toolInput)) process.exit(0);

  const logEntry = {
    timestamp: new Date().toISOString(),
    tool: 'store-cli',
    command: 'query',
    input: toolInput.substring(0, 500),
  };

  let storeRel = '.forge/store';
  try {
    const cfgPath = path.join(process.cwd(), '.forge', 'config.json');
    if (fs.existsSync(cfgPath)) {
      const cfg = JSON.parse(fs.readFileSync(cfgPath, 'utf8'));
      if (cfg.paths?.store) storeRel = cfg.paths.store;
    }
  } catch {}

  const logPath = path.join(process.cwd(), storeRel, 'query-log.jsonl');
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  fs.appendFileSync(logPath, JSON.stringify(logEntry) + '\n');
} catch {
  // Silent fail — hooks must not crash the session
}
