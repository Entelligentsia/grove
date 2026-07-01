document.addEventListener('DOMContentLoaded', () => {
  // Helper for bulletproof clipboard copy (works across HTTP/HTTPS and all browser contexts)
  function copyToClipboard(text) {
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text);
    } else {
      return new Promise((resolve, reject) => {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        textarea.style.position = 'fixed';
        textarea.style.left = '-999999px';
        textarea.style.top = '-999999px';
        document.body.appendChild(textarea);
        textarea.focus();
        textarea.select();
        try {
          const successful = document.execCommand('copy');
          document.body.removeChild(textarea);
          if (successful) resolve();
          else reject(new Error('execCommand copy failed'));
        } catch (err) {
          document.body.removeChild(textarea);
          reject(err);
        }
      });
    }
  }

  // 1. Installation Tabs Data & Switcher
  const installCommands = {
    npm: 'npm install -g @entelligentsia/grove',
    brew: 'brew install Entelligentsia/grove/grove',
    curl: 'curl -fsSL https://raw.githubusercontent.com/Entelligentsia/grove/main/install.sh | sh',
    cargo: 'cargo install grove-cst-cli',
    skill: 'npx skills add Entelligentsia/grove'
  };

  const installTabs = document.querySelectorAll('.install-tab-btn');
  const installCmdEl = document.getElementById('install-cmd-text');
  const copyBtn = document.getElementById('btn-copy-cmd');

  let currentInstallCmd = installCommands.npm;

  if (installTabs && installCmdEl) {
    installTabs.forEach(tab => {
      tab.addEventListener('click', () => {
        installTabs.forEach(t => t.classList.remove('active'));
        tab.classList.add('active');
        const key = tab.getAttribute('data-tab');
        if (installCommands[key]) {
          currentInstallCmd = installCommands[key];
          installCmdEl.textContent = currentInstallCmd;
        }
      });
    });
  }

  if (copyBtn) {
    copyBtn.addEventListener('click', () => {
      copyToClipboard(currentInstallCmd).then(() => {
        const orig = copyBtn.textContent;
        copyBtn.textContent = '✓ Copied!';
        copyBtn.style.background = '#34d399';
        setTimeout(() => {
          copyBtn.textContent = orig;
          copyBtn.style.background = '';
        }, 2000);
      }).catch(err => {
        console.error('Copy failed:', err);
      });
    });
  }

  // 2. Tool Explorer Data & Tabs
  const toolsData = {
    outline: {
      name: 'grove outline <file>',
      desc: 'Generates a file definition skeleton (kind · name · parent · signature · id). Gives the agent structural overview without reading whole files.',
      cmd: 'grove outline src/server.py',
      output: `class  Server            12:0   py:src/server.py#Server
  def  __init__          14:4   py:src/server.py#Server.__init__
  def  handle_request    31:4   py:src/server.py#Server.handle_request
def    main              88:0   py:src/server.py#main`
    },
    symbols: {
      name: 'grove symbols <dir> --name <n>',
      desc: 'Repo-wide symbol search with exact byte matching and stable handles. Pass forward across turns.',
      cmd: 'grove symbols src/ --name handle_request',
      output: `[
  {
    "symbol_id": "py:src/server.py#Server.handle_request",
    "name": "handle_request",
    "kind": "function",
    "range": { "start_line": 31, "end_line": 45 }
  }
]`
    },
    source: {
      name: 'grove source <id>',
      desc: 'Extracts one symbol body by exact bytes. Eliminates whole-file context dumps.',
      cmd: 'grove source "py:src/server.py#Server.handle_request"',
      output: `def handle_request(self, req: Request) -> Response:
    """Processes incoming HTTP requests structurally."""
    handler = self.router.match(req.path)
    return handler(req)`
    },
    check: {
      name: 'grove check <file>',
      desc: 'Post-edit syntax verification. Leverages Tree-sitter error-tolerant parsing to locate ERROR / MISSING nodes immediately.',
      cmd: 'grove check src/server.py',
      output: `✓ src/server.py parsed cleanly (0 ERROR / MISSING nodes)`
    },
    callers: {
      name: 'grove callers <name> -d <dir>',
      desc: 'Locates all call sites of a symbol across the repository, each tied to its enclosing function.',
      cmd: 'grove callers handle_request -d src/',
      output: `src/app.py:102:4 -> called inside main()`
    },
    map: {
      name: 'grove map <dir>',
      desc: 'Cold-start orientation. Tiered repository dependency graph showing definitions and outgoing references.',
      cmd: 'grove map src/',
      output: `src/
  ├── server.py (defs: Server, main | refs: Request, Response)
  └── router.py (defs: Router | refs: Match)`
    },
    definition: {
      name: 'grove definition <name>',
      desc: 'Go-to-def structural resolution by symbol name or cursor usage position.',
      cmd: 'grove definition Server',
      output: `Defined at py:src/server.py#Server (line 12, col 0)`
    }
  };

  const toolNav = document.getElementById('tool-nav');
  const toolNameEl = document.getElementById('tool-name');
  const toolDescEl = document.getElementById('tool-desc');
  const toolCmdEl = document.getElementById('tool-cmd');
  const toolOutputEl = document.getElementById('tool-output');

  if (toolNav) {
    Object.keys(toolsData).forEach((key, index) => {
      const btn = document.createElement('button');
      btn.className = `tool-nav-btn ${index === 0 ? 'active' : ''}`;
      btn.textContent = key;
      btn.addEventListener('click', () => {
        document.querySelectorAll('.tool-nav-btn').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        
        const data = toolsData[key];
        toolNameEl.textContent = data.name;
        toolDescEl.textContent = data.desc;
        toolCmdEl.textContent = data.cmd;
        toolOutputEl.textContent = data.output;
      });
      toolNav.appendChild(btn);
    });
  }

  // 3. Rolling real-prompt metric roller
  const ARM_COLORS = { baseline: '#0072B2', grove: '#E69F00', lsp: '#009E73' };
  const ARM_LABELS = { baseline: 'bash-tools', grove: 'grove', lsp: 'lsp' };
  const ARMS = ['baseline', 'grove', 'lsp'];
  const REPO_LANG = { tokio: 'rust', typescript: 'typescript', bitcoin: 'cpp', redis: 'c', rails: 'ruby', laravel: 'php' };
  const RUNG_LEVEL = { L1: 'Locate a symbol', L2: 'Trace a call', L3: 'Follow a flow', L4: 'Map a subsystem', L5: 'Architecture trace' };
  const pad2 = n => (n < 10 ? '0' : '') + n;

  // Real per-task data from is-grep-enough (site/data): 6 curated tasks.
  const rollerTasks = [
    { repo: 'tokio', gh: 'tokio-rs/tokio', rung: 'L5',
      prompt: '"I\'m planning a change to how a task that is parked waiting on both a socket read and a timer gets woken and rescheduled — walk me through the full journey from the future being polled to the runtime re-queuing it."',
      arms: { baseline: { context: 4400290, turns: 5, tool_calls: 126, wall: 243, cost: 1.02, grounding: 0.95, completeness: 1 },
              grove:    { context: 555024,  turns: 39, tool_calls: 38,  wall: 211, cost: 0.62, grounding: 0.97, completeness: 1 },
              lsp:      { context: 1171195, turns: 28, tool_calls: 27,  wall: 249, cost: 0.92, grounding: 0.97, completeness: 0.9 } } },
    { repo: 'typescript', gh: 'microsoft/TypeScript', rung: 'L5',
      prompt: '"I\'m planning a change to what happens around producing output and reported errors for a source file, so I need to understand the full journey from source text to emitted JavaScript and diagnostics."',
      arms: { baseline: { context: 2427126, turns: 4,  tool_calls: 114, wall: 214, cost: 0.62, grounding: 0.96, completeness: 1 },
              grove:    { context: 569597,  turns: 33, tool_calls: 32,  wall: 165, cost: 0.50, grounding: 0.97, completeness: 1 },
              lsp:      { context: 1101933, turns: 42, tool_calls: 41,  wall: 219, cost: 0.89, grounding: 0.98, completeness: 1 } } },
    { repo: 'bitcoin', gh: 'bitcoin/bitcoin', rung: 'L5',
      prompt: '"I\'m planning a change to how a transaction submitted to the node reaches its peers, so I need the full journey from the RPC that accepts it to the bytes announced out to the network."',
      arms: { baseline: { context: 1670994, turns: 2,  tool_calls: 53, wall: 252, cost: 0.41, grounding: 0.80, completeness: 0.8 },
              grove:    { context: 546658,  turns: 27, tool_calls: 26, wall: 375, cost: 0.53, grounding: 0.97, completeness: 1 },
              lsp:      { context: 1355451, turns: 47, tool_calls: 46, wall: 327, cost: 0.90, grounding: 0.97, completeness: 1 } } },
    { repo: 'redis', gh: 'redis/redis', rung: 'L1',
      prompt: '"I\'m trying to reason about Redis\'s per-value memory footprint and how a stored value is tagged and tracked — walk me through the in-memory container that holds a single value."',
      arms: { baseline: { context: 126393, turns: 7,  tool_calls: 6,  wall: 59, cost: 0.16, grounding: 0.96, completeness: 1 },
              grove:    { context: 267978, turns: 13, tool_calls: 12, wall: 82, cost: 0.22, grounding: 0.95, completeness: 1 },
              lsp:      { context: 379118, turns: 13, tool_calls: 12, wall: 78, cost: 0.27, grounding: 0.96, completeness: 1 } } },
    { repo: 'rails', gh: 'rails/rails', rung: 'L2',
      prompt: '"To predict when a relation that hasn\'t run yet actually fires its database query, I need to understand where lazy evaluation turns into an executed query on an Active Record relation."',
      arms: { baseline: { context: 1839631, turns: 10, tool_calls: 42, wall: 211, cost: 0.48, grounding: 0.96, completeness: 1 },
              grove:    { context: 1047014, turns: 64, tool_calls: 63, wall: 348, cost: 0.81, grounding: 0.96, completeness: 1 },
              lsp:      { context: 479999,  turns: 16, tool_calls: 15, wall: 135, cost: 0.49, grounding: 0.97, completeness: 1 } } },
    { repo: 'laravel', gh: 'laravel/framework', rung: 'L5',
      prompt: '"I\'m planning a change to how a dispatched event reaches its handlers, so I need the full journey from firing the event to each listener being resolved and invoked."',
      arms: { baseline: { context: 363422, turns: 3,  tool_calls: 17, wall: 194, cost: 0.32, grounding: 0.98, completeness: 1 },
              grove:    { context: 732508, turns: 43, tool_calls: 42, wall: 262, cost: 0.63, grounding: 0.97, completeness: 1 },
              lsp:      { context: 430910, turns: 17, tool_calls: 16, wall: 133, cost: 0.43, grounding: 0.84, completeness: 0.7 } } }
  ];

  const rollerMetrics = [
    { key: 'context',      icon: '🧠', word: 'Context',   sub: 'fewer is leaner',   dir: 'lo', fmt: fmtTokens },
    { key: 'turns',        icon: '🔁', word: 'Turns',     sub: 'fewer is leaner',   dir: 'lo', fmt: v => '' + v },
    { key: 'tool_calls',   icon: '🛠️', word: 'Tools',     sub: 'fewer is leaner',   dir: 'lo', fmt: v => '' + v },
    { key: 'wall',         icon: '⚡', word: 'Speed',     sub: 'faster is better',  dir: 'lo', fmt: v => v + 's' },
    { key: 'cost',         icon: '💲', word: 'Cost',      sub: 'cheaper is better', dir: 'lo', fmt: v => '$' + v.toFixed(2) },
    { key: 'grounding',    icon: '🎯', word: 'Grounding', sub: 'higher is truer',   dir: 'hi', fmt: v => v.toFixed(2) },
    { key: 'completeness', icon: '🧩', word: 'Coverage',  sub: 'higher is better',  dir: 'hi', fmt: v => v.toFixed(2) }
  ];

  function fmtTokens(v) {
    if (v >= 1e6) return (v / 1e6).toFixed(2) + 'M';
    if (v >= 1e3) return Math.round(v / 1e3) + 'K';
    return '' + v;
  }

  // Rank arms for a metric → medal per arm ('gold'|'silver'|'bronze'), tie-aware.
  const MEDALS = { gold: '🥇', silver: '🥈', bronze: '🥉' };
  function rankArms(task, metric) {
    const vals = ARMS.map(a => ({ arm: a, v: task.arms[a][metric.key] }));
    vals.sort((x, y) => metric.dir === 'lo' ? x.v - y.v : y.v - x.v);
    const spread = Math.abs(vals[0].v - vals[2].v);
    const isQuality = metric.key === 'grounding' || metric.key === 'completeness';
    const tie = isQuality && spread < 0.02; // near-equal quality → call it a tie
    const out = {};
    const names = ['gold', 'silver', 'bronze'];
    vals.forEach((x, i) => { out[x.arm] = tie ? 'tie' : names[i]; });
    return out;
  }

  // Combined per-task winner: ½ quality + ½ (cheaper billed cost), normalized within task.
  function taskWinner(task) {
    const q = a => (task.arms[a].grounding + task.arms[a].completeness) / 2;
    const c = a => task.arms[a].cost;
    const qs = ARMS.map(q), cs = ARMS.map(c);
    const qmin = Math.min(...qs), qmax = Math.max(...qs);
    const cmin = Math.min(...cs), cmax = Math.max(...cs);
    const score = a => {
      const qn = qmax > qmin ? (q(a) - qmin) / (qmax - qmin) : 1;
      const cn = cmax > cmin ? (cmax - c(a)) / (cmax - cmin) : 1;
      return 0.5 * qn + 0.5 * cn;
    };
    return ARMS.slice().sort((x, y) => score(y) - score(x))[0];
  }

  const roller = document.getElementById('metric-roller');
  if (roller) {
    const el = id => document.getElementById(id);
    const mdots = el('rl-mdots'), psegs = el('rl-psegs'), pfill = el('rl-pprog');
    let pi = 0, mi = 0, paused = false;
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    rollerMetrics.forEach((m, i) => {
      const d = document.createElement('button');
      d.className = 'roll-dot'; d.title = m.word;
      d.addEventListener('click', () => { mi = i; renderMetric(); });
      mdots.appendChild(d);
    });
    rollerTasks.forEach((t, i) => {
      const seg = document.createElement('button');
      seg.title = t.repo + ' ' + t.rung;
      seg.addEventListener('click', () => { pi = i; mi = 0; renderTask(); });
      psegs.appendChild(seg);
    });

    function renderTask() {
      const t = rollerTasks[pi];
      el('rl-rung').textContent = t.rung;
      el('rl-level').textContent = RUNG_LEVEL[t.rung] || '';
      el('rl-repo').textContent = t.repo;
      el('rl-gh').textContent = t.gh;
      const lang = REPO_LANG[t.repo];
      if (lang) { el('rl-langicon').src = 'assets/langs/' + lang + '.svg'; el('rl-langicon').alt = lang; }
      el('rl-count').textContent = pad2(pi + 1) + ' / ' + pad2(rollerTasks.length);
      el('rl-prompt').textContent = t.prompt.replace(/^["']|["']$/g, '');
      const w = taskWinner(t);
      const badge = el('rl-winner');
      badge.textContent = ARM_LABELS[w];
      badge.style.background = ARM_COLORS[w];
      const notes = { baseline: 'fewest turns, cheapest billed', grove: 'leanest context at tied quality', lsp: 'best semantic economy' };
      el('rl-winnote').textContent = notes[w];
      pfill.style.width = ((pi + 1) / rollerTasks.length) * 100 + '%';
      renderMetric();
    }

    function renderMetric() {
      const t = rollerTasks[pi], m = rollerMetrics[mi];
      el('rl-micon').textContent = m.icon;
      el('rl-mword').textContent = m.word;
      el('rl-msub').textContent = m.sub;
      const medals = rankArms(t, m);
      const max = Math.max(...ARMS.map(a => t.arms[a][m.key]));
      ARMS.forEach(a => {
        const v = t.arms[a][m.key];
        el('rl-val-' + a).textContent = m.fmt(v);
        el('rl-bar-' + a).style.height = (max > 0 ? (v / max) * 100 : 0) + '%';
        const med = medals[a];
        el('rl-medal-' + a).innerHTML = med === 'tie'
          ? '<span class="tie-pill">tie</span>'
          : (MEDALS[med] || '');
        el('rl-val-' + a).closest('.bar-col').classList.toggle('win', med === 'gold');
      });
      Array.from(mdots.children).forEach((d, i) => d.classList.toggle('active', i === mi));
    }

    renderTask();

    if (!reduce) {
      roller.addEventListener('mouseenter', () => { paused = true; });
      roller.addEventListener('mouseleave', () => { paused = false; });
      setInterval(() => {
        if (paused) return;
        mi += 1;
        if (mi >= rollerMetrics.length) { mi = 0; pi = (pi + 1) % rollerTasks.length; renderTask(); }
        else renderMetric();
      }, 5000);
    }
  }
});
