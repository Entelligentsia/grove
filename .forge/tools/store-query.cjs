#!/usr/bin/env node
'use strict';
// store-query.cjs — Forge store query engine CLI
// Implements exact-args, keyword, and NLP intent paths.
// Spawned by store-cli.cjs query/nlp/schema dispatch.

const path = require('path');

process.on('uncaughtException', (e) => {
  process.stderr.write(`Error: ${e.message}\n`);
  process.exit(1);
});

const { StoreFacade, loadForgeConfig, resetConfigCache } = require('./lib/store-facade.cjs');
const { parseIntentNLP, ENTITY_SYNONYMS, STATUS_MAP }    = require('./lib/store-nlp.cjs');
const { executeQuery, buildResult, kwMatches }            = require('./lib/store-query-exec.cjs');

// ── Argument parser ───────────────────────────────────────────────────────────

function parseArgs(argv) {
  const args = { _raw: argv.join(' ') };
  const intentWords = [];
  let i = 0;
  while (i < argv.length) {
    const a = argv[i];
    switch (a) {
      case '--sprint':            args.sprint   = argv[++i]; break;
      case '--task':              args.task     = argv[++i]; break;
      case '--bug':               args.bug      = argv[++i]; break;
      case '--feature':           args.feature  = argv[++i]; break;
      case '--status':            args.status   = argv[++i]; break;
      case '--keyword':           args.keyword  = argv[++i]; break;
      case '--type':              args.type     = argv[++i]; break;
      case '--mode':              args.mode     = argv[++i]; break;
      case '--with-blockers':     args.withBlockers    = true; break;
      case '--with-blocked-tasks':args.withBlockedTasks = true; break;
      case '--with-sprint':       args.withSprint      = true; break;
      case '--with-feature':      args.withFeature     = true; break;
      case '--no-excerpts':       args.noExcerpts      = true; break;
      case '--list-sprints':      args.listSprints     = true; break;
      case '--task-suffix':       args.taskSuffix      = argv[++i]; break;
      case '--sprint-suffix':     args.sprintSuffix    = argv[++i]; break;
      case '--help': case '-h':   args.help = true; break;
      default:
        if (!a.startsWith('-')) intentWords.push(a);
        break;
    }
    i++;
  }
  if (intentWords.length > 0) args.intent = intentWords.join(' ');
  return args;
}

function printHelp() {
  process.stdout.write(`Usage: store-query <command> [options]

Commands:
  query   Query the Forge store (exact args or NLP intent)
  nlp     Query the Forge store using NLP intent parser
  schema  Dump project schema and grammar reference

Query options:
  --sprint <id>           List tasks/bugs for a sprint
  --task <id>             Get task details
  --bug <id>              Get bug details
  --feature <id>          Get feature + related tasks
  --list-sprints          List all sprints
  --task-suffix <Tnn>     Match tasks whose id ends with -Tnn (case-insensitive)
  --sprint-suffix <Snn>   Match sprints whose id ends with -Snn or equals Snn
  --status <status>       Filter by status
  --keyword <term>        Search entity titles
  --type <entity>         Limit --keyword to sprints|tasks|bugs|features
  --with-blockers         Follow blockedBy FK on tasks
  --with-blocked-tasks    Follow blocksTask FK on bugs
  --with-sprint           Follow sprintId FK
  --with-feature          Follow featureId FK
  --no-excerpts           Omit INDEX.md excerpts
  --mode strict|nlp|off   Engine mode (default: nlp for intent, strict for flags)
`);
}

// ── Suffix-match query path ───────────────────────────────────────────────────

function cmdSuffixMatch(args, store, cfg) {
  const trace = [];
  const results = [];
  const relatedFiles = [];
  const includeExcerpts = !args.noExcerpts;

  if (args.taskSuffix) {
    const suffix = String(args.taskSuffix).toUpperCase();
    const tasks = store.listTasks().filter(t => {
      const id = String(t.taskId || '').toUpperCase();
      return id === suffix || id.endsWith(`-${suffix}`);
    });
    trace.push(`task-suffix "${suffix}": ${tasks.length} match(es)`);
    for (const t of tasks) results.push(buildResult(t, 'task', store, includeExcerpts));
  }

  if (args.sprintSuffix) {
    const suffix = String(args.sprintSuffix).toUpperCase();
    const sprints = store.listSprints().filter(s => {
      const id = String(s.sprintId || '').toUpperCase();
      return id === suffix || id.endsWith(`-${suffix}`);
    });
    trace.push(`sprint-suffix "${suffix}": ${sprints.length} match(es)`);
    for (const s of sprints) results.push(buildResult(s, 'sprint', store, includeExcerpts));
  }

  for (const r of results) {
    if (r.fileRefs?.md) relatedFiles.push(r.fileRefs.md);
    if (r.fileRefs?.json) relatedFiles.push(r.fileRefs.json);
  }

  return { query: args._raw, path: 'suffix', traversalTrace: trace, results, relatedFileRefs: [...new Set(relatedFiles)], config: { store: cfg.storePathRel, engineering: cfg.kbPath || 'engineering' } };
}

// ── Exact-args query path ─────────────────────────────────────────────────────

function cmdQueryExact(args, store, cfg) {
  const trace = [];
  const results = [];
  const relatedFiles = [];
  const includeExcerpts = !args.noExcerpts;
  const filter = {};
  if (args.status) filter.status = args.status;

  if (args.sprint === 'all' || args.listSprints) {
    const sprints = store.listSprints(filter);
    trace.push(`listed all sprints: ${sprints.length}`);
    for (const s of sprints) results.push(buildResult(s, 'sprint', store, includeExcerpts));
  } else if (args.sprint) {
    const sprint = store.getEntity('sprints', args.sprint);
    const tasks = store.listTasks({ sprintId: args.sprint, ...filter });
    const bugs  = store.listBugs({ sprintId: args.sprint, ...filter });
    trace.push(`sprint ${args.sprint}: ${tasks.length} tasks, ${bugs.length} bugs`);
    if (sprint) results.push(buildResult(sprint, 'sprint', store, includeExcerpts));
    for (const t of tasks) {
      results.push(buildResult(t, 'task', store, includeExcerpts));
      if (args.withBlockers && t.blockedBy) {
        for (const bugId of (Array.isArray(t.blockedBy) ? t.blockedBy : [t.blockedBy])) {
          const bug = store.getEntity('bugs', bugId);
          if (bug) { results.push(buildResult(bug, 'bug', store, includeExcerpts)); trace.push(`followed blockedBy → ${bugId}`); }
        }
      }
    }
    for (const b of bugs) results.push(buildResult(b, 'bug', store, includeExcerpts));
  } else if (args.task) {
    const task = store.getEntity('tasks', args.task);
    if (task) {
      results.push(buildResult(task, 'task', store, includeExcerpts));
      if (args.withSprint && task.sprintId) {
        const s = store.getEntity('sprints', task.sprintId);
        if (s) { results.push(buildResult(s, 'sprint', store, includeExcerpts)); trace.push(`followed sprintId → ${task.sprintId}`); }
      }
      if (args.withFeature && (task.featureId || task.feature_id)) {
        const fid = task.featureId || task.feature_id;
        const f = store.getEntity('features', fid);
        if (f) { results.push(buildResult(f, 'feature', store, includeExcerpts)); trace.push(`followed featureId → ${fid}`); }
      }
      trace.push(`task ${args.task} found`);
    } else {
      trace.push(`task ${args.task} not found`);
    }
  } else if (args.bug) {
    const bug = store.getEntity('bugs', args.bug);
    if (bug) {
      results.push(buildResult(bug, 'bug', store, includeExcerpts));
      if (args.withBlockedTasks && bug.blocksTask) {
        for (const tid of (Array.isArray(bug.blocksTask) ? bug.blocksTask : [bug.blocksTask])) {
          const t = store.getEntity('tasks', tid);
          if (t) { results.push(buildResult(t, 'task', store, includeExcerpts)); trace.push(`followed blocksTask → ${tid}`); }
        }
      }
      trace.push(`bug ${args.bug} found`);
    } else {
      trace.push(`bug ${args.bug} not found`);
    }
  } else if (args.feature) {
    const feat = store.getEntity('features', args.feature);
    if (feat) {
      results.push(buildResult(feat, 'feature', store, includeExcerpts));
      const tasks = store.listTasks({ featureId: args.feature, ...filter });
      trace.push(`feature ${args.feature}: ${tasks.length} tasks`);
      for (const t of tasks) results.push(buildResult(t, 'task', store, includeExcerpts));
    } else {
      trace.push(`feature ${args.feature} not found`);
    }
  }

  for (const r of results) {
    if (r.fileRefs?.md) relatedFiles.push(r.fileRefs.md);
    if (r.fileRefs?.json) relatedFiles.push(r.fileRefs.json);
  }

  return { query: args._raw, path: 'exact', traversalTrace: trace, results, relatedFileRefs: [...new Set(relatedFiles)], config: { store: cfg.storePathRel, engineering: cfg.kbPath || 'engineering' } };
}

// ── Keyword search path ───────────────────────────────────────────────────────

function cmdKeywordSearch(args, store, cfg) {
  const trace = [];
  const results = [];
  const relatedFiles = [];
  const includeExcerpts = !args.noExcerpts;
  const keyword = args.keyword;
  const targetType = args.type || null;

  const entityTypes = targetType
    ? [targetType]
    : ['sprints', 'tasks', 'bugs', 'features'];
  const singMap = { sprints: 'sprint', tasks: 'task', bugs: 'bug', features: 'feature' };

  for (const plural of entityTypes) {
    const singular = singMap[plural];
    let entities;
    switch (plural) {
      case 'sprints':  entities = store.listSprints();  break;
      case 'tasks':    entities = store.listTasks();    break;
      case 'bugs':     entities = store.listBugs();     break;
      case 'features': entities = store.listFeatures(); break;
      default:         entities = [];
    }
    const before = entities.length;
    const matched = entities.filter(e => kwMatches(e.title || '', keyword));
    if (matched.length > 0) {
      trace.push(`keyword "${keyword}" in ${plural}: ${before} → ${matched.length}`);
      for (const m of matched) results.push(buildResult(m, singular, store, includeExcerpts));
    }
  }

  for (const r of results) {
    if (r.fileRefs?.md) relatedFiles.push(r.fileRefs.md);
    if (r.fileRefs?.json) relatedFiles.push(r.fileRefs.json);
  }

  return { query: args._raw, path: 'keyword', traversalTrace: trace, results, relatedFileRefs: [...new Set(relatedFiles)], config: { store: cfg.storePathRel, engineering: cfg.kbPath || 'engineering' } };
}

// ── Schema command ────────────────────────────────────────────────────────────

function cmdSchema(cfg) {
  const { buildFieldValidators } = require('./lib/store-query-exec.cjs');
  // build validators inline since not exported — rebuild here
  const p = cfg.prefix;
  const esc = p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const fv = {
    sprints:  { sprintId: /^S\d+$/,  status: ['planning','active','completed','retrospective-done','blocked','partially-completed','abandoned'] },
    tasks:    { taskId: new RegExp(`^${esc}-S\\d+-T\\d+$`), sprintId: /^S\d+$/, featureId: /^FEAT-\d+$/, status: ['draft','planned','plan-approved','implementing','implemented','review-approved','approved','committed','plan-revision-required','code-revision-required','blocked','escalated','abandoned'] },
    bugs:     { bugId: new RegExp(`^${esc}-BUG-\\d+$`), sprintId: /^S\d+$/, severity: ['critical','major','minor'], status: ['reported','triaged','in-progress','fixed','verified'] },
    features: { featureId: /^FEAT-\d+$/, status: ['active','draft','shipped','retired'] },
  };
  const toEnum = spec => Array.isArray(spec) ? spec : (spec instanceof RegExp ? `pattern: ${spec}` : null);

  return {
    project: { prefix: cfg.prefix, name: cfg.projectName, kbPath: cfg.kbPath, storePath: cfg.storePathRel },
    entities: {
      sprints:  { idField: 'sprintId',  idPattern: fv.sprints.sprintId.toString(),  status: toEnum(fv.sprints.status), fks: [] },
      tasks:    { idField: 'taskId',    idPattern: fv.tasks.taskId.toString(),      status: toEnum(fv.tasks.status),   fks: ['sprintId','featureId','blockedBy'] },
      bugs:     { idField: 'bugId',     idPattern: fv.bugs.bugId.toString(),        status: toEnum(fv.bugs.status), severity: toEnum(fv.bugs.severity), fks: ['sprintId','blocksTask'] },
      features: { idField: 'featureId', idPattern: fv.features.featureId.toString(),status: toEnum(fv.features.status), fks: [] },
    },
    entitySynonyms: ENTITY_SYNONYMS,
    statusSynonyms: STATUS_MAP,
    grammar: {
      recency: ['last','latest','newest','recent','most recent','oldest','earliest','first'],
      limit:   ['top N','first N','last N'],
      count:   ['how many','count of','number of','count'],
      fkPhrases: ['with sprint','with feature','blocking','blocked','which sprint','sprint for','feature for'],
    },
  };
}

// ── Main ──────────────────────────────────────────────────────────────────────

function main() {
  const argv = process.argv.slice(2);
  const startMs = Date.now();

  if (argv.length === 0 || argv[0] === '--help' || argv[0] === '-h') {
    printHelp();
    process.exit(0);
  }

  const command = argv[0];
  const rest = argv.slice(1);
  const args = parseArgs(rest);

  if (args.help) { printHelp(); process.exit(0); }

  const cfg = loadForgeConfig();
  const store = new StoreFacade(cfg.storePathAbs);

  let result;

  switch (command) {
    case 'schema': {
      result = cmdSchema(cfg);
      break;
    }
    case 'nlp': {
      if (!args.intent) {
        process.stderr.write('Usage: store-query nlp "<natural language query>"\n');
        process.exit(1);
      }
      const plan = parseIntentNLP(args.intent);
      result = executeQuery(plan, store, { query: args.intent, noExcerpts: args.noExcerpts });
      result.query = args.intent;
      result.path  = 'intent-nlp';
      result.config = { store: cfg.storePathRel, engineering: cfg.kbPath || 'engineering' };
      break;
    }
    case 'query': {
      const mode = args.mode || 'auto';
      const hasExactFlags = args.sprint || args.task || args.bug || args.feature || args.listSprints;
      const hasKeyword = !!args.keyword;
      const hasSuffix = !!(args.taskSuffix || args.sprintSuffix);

      if (mode === 'strict' || mode === 'off') {
        if (args.intent && !hasExactFlags && !hasKeyword) {
          process.stderr.write('Mode is strict — intent strings not accepted. Use --sprint/--task/--bug/--feature flags.\n');
          process.exit(1);
        }
      }

      if (hasSuffix) {
        result = cmdSuffixMatch(args, store, cfg);
      } else if (hasExactFlags) {
        result = cmdQueryExact(args, store, cfg);
      } else if (hasKeyword) {
        result = cmdKeywordSearch(args, store, cfg);
      } else if (args.intent) {
        const plan = parseIntentNLP(args.intent);
        result = executeQuery(plan, store, { query: args.intent, noExcerpts: args.noExcerpts });
        result.query = args.intent;
        result.path  = 'intent-nlp';
        result.config = { store: cfg.storePathRel, engineering: cfg.kbPath || 'engineering' };
      } else {
        process.stderr.write('Provide an entity flag (--sprint, --task, --bug, --feature) or an intent string.\n');
        process.exit(1);
      }
      break;
    }
    default: {
      process.stderr.write(`Unknown command: ${command}\n`);
      printHelp();
      process.exit(1);
    }
  }

  // Attach meta timing block
  const totalMs = Date.now() - startMs;
  result.meta = {
    mode:         args.mode || 'auto',
    engineVersion: '1.0.0',
    totalTimeMs:   totalMs,
  };

  process.stdout.write(JSON.stringify(result, null, 2) + '\n');
}

main();
