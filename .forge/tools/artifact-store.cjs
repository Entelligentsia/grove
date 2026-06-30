'use strict';

// ── ArtifactStore — backend-agnostic artifact provider ───────────────────────
//
// Mirrors the store.cjs `Store`/`FSImpl` pattern (ADR
// `doc/decisions/artifact-resolution-abstraction.md`, issue #111 Phase 3):
// a backend-agnostic facade delegating to a swappable, SYNCHRONOUS impl,
// default-wired to the filesystem, impl exported for substitution.
//
//   class ArtifactStore { read|write|exists|url|list|delete(handle) → impl }
//   class FsArtifactImpl { engineering/ files }
//   module.exports = new ArtifactStore(new FsArtifactImpl())
//   module.exports.FsArtifactImpl = FsArtifactImpl   // swap for S3Impl / CmsImpl / DbBlobImpl
//
// A `handle` is the logical address (entityType, entityId, artifactKind) — never
// a path. The fs impl resolves it from the store record's `path` (the locator)
// plus the canonical kind registry, so callers never construct paths.
//
// SYNC CONSTRAINT (load-bearing): in-process callers (store-cli.cjs,
// preflight-gate.cjs, collate.cjs) invoke this without `await`, so every method
// is synchronous — same constraint that blocks the store's async InstantDbImpl
// (see doc/decisions/instantdb-store-backend.md). A future remote impl must
// either stay sync or be reached only through the forge-cli subprocess surface.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const { resolveArtifactFilename } = require('./lib/artifact-kinds.cjs');

// ── Locator helpers ({ backend, ref }) ───────────────────────────────────────

// Derive the backend-agnostic locator from a store record. Prefers an explicit
// `record.locator`; otherwise treats the legacy `record.path` as an fs locator
// (the back-compat alias maintained during migration).
function toLocator(record) {
  if (record && record.locator && record.locator.backend) return record.locator;
  if (record && typeof record.path === 'string' && record.path.length > 0) {
    return { backend: 'fs', ref: record.path };
  }
  return null;
}

// For an fs locator ref, return the entity *directory* (strip a trailing filename).
function fsRefToDir(ref) {
  const norm = String(ref).replace(/\\/g, '/').replace(/\/+$/, '');
  return /\.[a-zA-Z0-9]+$/.test(norm) ? norm.replace(/\/[^/]*$/, '') : norm;
}

// ── Default dir resolution (store record path) ───────────────────────────────
// Reads a record's `path` via store-cli (out-of-process) so this module has no
// hard dependency on store internals. Returns the entity directory or null.
function readStorePath(entity, entityId, toolDir, projectRoot) {
  const cliPath = path.join(toolDir, 'store-cli.cjs');
  try {
    const result = execFileSync('node', [cliPath, 'read', entity, entityId, '--json'], {
      cwd: projectRoot, encoding: 'utf8', timeout: 10_000,
    });
    const record = JSON.parse(result);
    const loc = toLocator(record);
    if (loc && loc.backend === 'fs') return fsRefToDir(loc.ref);
  } catch (_) { /* store unavailable / record not found — fall through */ }
  return null;
}

// Resolve entity directory from the store record's path, falling back to
// ID-only construction. (Moved verbatim from artifact.cjs; re-exported there.)
function resolveEntityDir(entity, entityId, engineeringPath, toolDir, projectRoot) {
  switch (entity) {
    case 'bug': {
      const storePath = readStorePath('bug', entityId, toolDir, projectRoot);
      if (storePath) return storePath;
      return path.join(engineeringPath, 'bugs', entityId);
    }
    case 'sprint': {
      const storePath = readStorePath('sprint', entityId, toolDir, projectRoot);
      if (storePath) return storePath;
      return path.join(engineeringPath, 'sprints', entityId);
    }
    case 'task': {
      const storePath = readStorePath('task', entityId, toolDir, projectRoot);
      if (storePath) return storePath;
      const match = entityId.match(/^(.+-S\d+)-T\d+$/);
      if (!match) return null;
      const sprintId = match[1];
      const sprintPath = readStorePath('sprint', sprintId, toolDir, projectRoot);
      if (sprintPath) return path.join(sprintPath, entityId);
      return path.join(engineeringPath, 'sprints', sprintId, entityId);
    }
    default:
      return null;
  }
}

// ── Facade ───────────────────────────────────────────────────────────────────

class ArtifactStore {
  // The constructor impl is the default backend (`fs`). Additional backends are
  // added with register(name, impl) — Phase 4. Each call routes to the impl for
  // the handle's `backend` (default 'fs'), so adding a backend requires
  // implementing the method surface only — no call-site or prompt changes.
  constructor(impl) {
    this.impl = impl;                 // default (fs) backend
    this._backends = new Map([['fs', impl]]);
  }

  // Register an additional backend impl (e.g. an S3/CMS/DB provider). Returns
  // `this` for chaining; re-registering the same name replaces the impl.
  register(backend, impl) {
    this._backends.set(backend, impl);
    return this;
  }

  _implFor(handle) {
    const backend = (handle && handle.backend) || 'fs';
    const impl = this._backends.get(backend);
    if (!impl) {
      throw new Error(
        `No ArtifactStore backend registered for "${backend}". ` +
        `Registered: ${[...this._backends.keys()].join(', ')}. ` +
        `Add one with artifactStore.register("${backend}", impl).`
      );
    }
    return impl;
  }

  read(handle)            { return this._implFor(handle).read(handle); }
  write(handle, content)  { return this._implFor(handle).write(handle, content); }
  exists(handle)          { return this._implFor(handle).exists(handle); }
  url(handle)             { return this._implFor(handle).url(handle); }
  list(handle)            { return this._implFor(handle).list(handle); }
  delete(handle)          { return this._implFor(handle).delete(handle); }
}

// ── Filesystem impl ───────────────────────────────────────────────────────────

class FsArtifactImpl {
  // opts: { projectRoot, engineeringPath, toolDir, resolveDir }
  //   resolveDir(entity, entityId) → dir (relative to projectRoot) | null.
  //   Injectable for testing; defaults to the store-record resolver above.
  constructor(opts = {}) {
    this.projectRoot = opts.projectRoot || process.cwd();
    this.engineeringPath = opts.engineeringPath || 'engineering';
    this.toolDir = opts.toolDir || __dirname;
    this._resolveDir = opts.resolveDir
      || ((entity, entityId) => resolveEntityDir(entity, entityId, this.engineeringPath, this.toolDir, this.projectRoot));
  }

  _absDir(handle) {
    const dir = this._resolveDir(handle.entity, handle.entityId);
    if (!dir) {
      throw new Error(
        `Cannot resolve ${handle.entity} directory for "${handle.entityId}". ` +
        `Expected ID pattern: task=PREFIX-SNN-TNN, bug=PREFIX-BNN[-slug], sprint=PREFIX-SNN.`
      );
    }
    return path.resolve(this.projectRoot, dir);
  }

  _absFile(handle) {
    return path.join(this._absDir(handle), resolveArtifactFilename(handle.entity, handle.kind));
  }

  read(handle) {
    const file = this._absFile(handle);
    if (!fs.existsSync(file)) {
      const err = new Error(`Artifact not found: ${path.relative(this.projectRoot, file)}`);
      err.code = 'ENOENT';
      throw err;
    }
    return fs.readFileSync(file, 'utf8');
  }

  write(handle, content) {
    const dir = this._absDir(handle);
    fs.mkdirSync(dir, { recursive: true });
    const file = path.join(dir, resolveArtifactFilename(handle.entity, handle.kind));
    fs.writeFileSync(file, content, 'utf8');
    return { bytes: Buffer.byteLength(content, 'utf8'), ref: path.relative(this.projectRoot, file) };
  }

  exists(handle) {
    try { return fs.existsSync(this._absFile(handle)); }
    catch (_) { return false; }
  }

  url(handle) {
    return 'file://' + this._absFile(handle);
  }

  // Entity-level listing: handle without `kind`. Returns existing filenames.
  list(handle) {
    const dir = this._absDir(handle);
    if (!fs.existsSync(dir)) return [];
    return fs.readdirSync(dir).filter((f) => f.endsWith('.md') || f.endsWith('.json'));
  }

  delete(handle) {
    const file = this._absFile(handle);
    if (fs.existsSync(file)) { fs.unlinkSync(file); return true; }
    return false;
  }
}

// ── Reference non-fs backend (Phase 4) ───────────────────────────────────────
//
// A complete, synchronous, in-memory implementation of the ArtifactStore method
// surface. It exists as the canonical *reference* for adding a backend: a real
// S3/CMS/DB provider implements these same six methods and is wired with
// `artifactStore.register('<backend>', new XxxArtifactImpl(...))` — no prompt or
// call-site changes. (A networked impl is sync-bound for in-process callers per
// the ADR; it is reachable async-internally only through the forge-cli
// subprocess surface. This in-memory impl is fully functional and dependency-free.)
class MemArtifactImpl {
  constructor() { this.files = new Map(); }
  _key(h) { return `${h.entity}/${h.entityId}/${resolveArtifactFilename(h.entity, h.kind)}`; }
  read(handle) {
    const k = this._key(handle);
    if (!this.files.has(k)) { const e = new Error(`Artifact not found: mem:${k}`); e.code = 'ENOENT'; throw e; }
    return this.files.get(k);
  }
  write(handle, content) {
    const k = this._key(handle);
    this.files.set(k, content);
    return { bytes: Buffer.byteLength(content, 'utf8'), ref: `mem:${k}` };
  }
  exists(handle) { return this.files.has(this._key(handle)); }
  url(handle)    { return `mem://${this._key(handle)}`; }
  list(handle)   {
    const prefix = `${handle.entity}/${handle.entityId}/`;
    return [...this.files.keys()].filter((k) => k.startsWith(prefix)).map((k) => k.slice(prefix.length));
  }
  delete(handle) { return this.files.delete(this._key(handle)); }
}

module.exports = new ArtifactStore(new FsArtifactImpl());
module.exports.ArtifactStore = ArtifactStore;
module.exports.FsArtifactImpl = FsArtifactImpl;
module.exports.MemArtifactImpl = MemArtifactImpl;
module.exports.toLocator = toLocator;
module.exports.fsRefToDir = fsRefToDir;
module.exports.resolveEntityDir = resolveEntityDir;
module.exports.readStorePath = readStorePath;
