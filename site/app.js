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
    cargo: 'cargo install --git https://github.com/Entelligentsia/grove',
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
});
