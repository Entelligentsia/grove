#!/usr/bin/env node
// Thin launcher: exec the prebuilt binary that install.js placed in vendor/.
"use strict";

const path = require("path");
const { spawnSync } = require("child_process");

const binName = process.platform === "win32" ? "grove-explore.exe" : "grove-explore";
const bin = path.join(__dirname, "..", "vendor", binName);

const r = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
if (r.error) {
  console.error(`grove-explore: failed to run binary (${r.error.message}). Try reinstalling.`);
  process.exit(1);
}
process.exit(r.status === null ? 1 : r.status);
