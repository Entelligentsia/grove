'use strict';
// hooks/lib/update-msg.cjs — extracted from check-update.js (H-2c, FORGE-S25-T14)
//
// Provides update message construction and emission.
// Extracted to improve testability — buildUpdateMsg and emit can be tested
// without executing the full check-update session logic.
//
// Uses only Node.js built-ins — no npm dependencies required.

function buildUpdateMsg(remoteVersion, local) {
  return remoteVersion && remoteVersion !== local
    ? `Forge ${remoteVersion} available (you have ${local}). Run /forge:update to review changes and update.`
    : '';
}

function emit(forgeCtx, updateMsg) {
  if (!forgeCtx && !updateMsg) return;
  const combined = [forgeCtx, updateMsg].filter(Boolean).join(' ');
  const escaped = combined.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, ' ');
  process.stdout.write(`{"additionalContext":"${escaped}"}\n`);
}

module.exports = { buildUpdateMsg, emit };
