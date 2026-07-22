#!/usr/bin/env node
// postinstall: download the prebuilt grove binary matching this package's
// version from the GitHub Release, verify its sha256, and extract it into
// vendor/. The bin shim (bin/grove.js) execs whatever lands there.
"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { execFileSync } = require("child_process");
const { Readable } = require("stream");
// Use undici's own `fetch`, not Node's global one: the global `fetch` is
// powered by whichever undici version ships inside that Node binary, and
// passing an `EnvHttpProxyAgent` built by a *different* (newer) standalone
// undici as its dispatcher can break on a handler-protocol mismatch between
// the two versions. Importing both from the same package keeps them in sync.
const { fetch, EnvHttpProxyAgent } = require("undici");
// Honors HTTP_PROXY/HTTPS_PROXY/ALL_PROXY + NO_PROXY from the environment for
// every fetch below — the standard proxy env vars work the same way curl/wget
// (install.sh) and Homebrew already do for this project.
const dispatcher = new EnvHttpProxyAgent();

const REPO = "Entelligentsia/grove";
const { version } = require("./package.json");

const TARGETS = {
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function fail(msg) {
  console.error(`grove: ${msg}`);
  process.exit(1);
}

async function download(url, dest) {
  // fetch follows redirects by default, so the GitHub Release → CDN hop the
  // old https.get handled manually is transparent here.
  const res = await fetch(url, {
    dispatcher,
    headers: { "User-Agent": "grove-npm-installer" },
  });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} for ${url}`);
  }
  const out = fs.createWriteStream(dest);
  await new Promise((resolve, reject) => {
    Readable.fromWeb(res.body)
      .pipe(out)
      .on("finish", resolve)
      .on("error", reject);
  });
}

async function getText(url) {
  const res = await fetch(url, {
    dispatcher,
    headers: { "User-Agent": "grove-npm-installer" },
  });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} for ${url}`);
  }
  return res.text();
}

async function main() {
  const key = `${process.platform}-${process.arch}`;
  const target = TARGETS[key];
  if (!target) {
    fail(`no prebuilt for ${key}. Install from source: cargo install --git https://github.com/${REPO}`);
  }

  const isWin = process.platform === "win32";
  const ext = isWin ? "zip" : "tar.gz";
  const asset = `grove-${target}.${ext}`;
  const base = `https://github.com/${REPO}/releases/download/v${version}`;
  const url = `${base}/${asset}`;

  const vendor = path.join(__dirname, "vendor");
  fs.mkdirSync(vendor, { recursive: true });
  const archive = path.join(vendor, asset);

  console.error(`grove: downloading ${asset} (v${version})`);
  await download(url, archive);

  // Verify checksum when the sidecar is present.
  try {
    const expected = (await getText(`${url}.sha256`)).trim().split(/\s+/)[0];
    const actual = crypto.createHash("sha256").update(fs.readFileSync(archive)).digest("hex");
    if (expected && expected !== actual) {
      fail(`checksum mismatch: expected ${expected}, got ${actual}`);
    }
  } catch (e) {
    console.error(`grove: skipping checksum verification (${e.message})`);
  }

  // System tar extracts both .tar.gz and .zip (bsdtar on Windows 10+, GNU/bsd tar on unix).
  execFileSync("tar", ["-xf", archive, "-C", vendor], { stdio: "inherit" });
  fs.unlinkSync(archive);

  const binNames = isWin ? ["grove.exe", "grove-explore.exe"] : ["grove", "grove-explore"];
  for (const binName of binNames) {
    const bin = path.join(vendor, binName);
    if (!fs.existsSync(bin)) fail(`binary ${binName} not found after extract`);
    if (!isWin) fs.chmodSync(bin, 0o755);
    console.error(`grove: installed ${bin}`);
  }
}

main().catch((e) => fail(e.message));
