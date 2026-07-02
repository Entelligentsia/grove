//! grove — Phase 1: one engine, two faces.
//!
//! The same operations are exposed as a human CLI (`grove outline …`) and as an
//! MCP server (`grove serve`). Both call `ops`; neither owns engine logic.

use grove_core::{ops, registry, fetch, ingest};

mod init;
mod mcp;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "grove", version, about = "Structural sight for coding agents (Phase 1)")]
struct Cli {
    /// Emit JSON instead of the human table.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List the definitions in one file (kind · name · parent · signature · id).
    Outline {
        file: PathBuf,
        /// Only this kind (e.g. class, function, method).
        #[arg(long)]
        kind: Option<String>,
        /// JSON detail: 0 terse · 1 default (no byte offsets) · 2 full.
        #[arg(long, default_value_t = 1)]
        detail: u8,
    },
    /// Find symbols across a directory, optionally filtered.
    Symbols {
        dir: PathBuf,
        /// Only this kind (e.g. function, struct, method).
        #[arg(long)]
        kind: Option<String>,
        /// Only definitions whose name equals this exactly (case-insensitive).
        #[arg(long)]
        name: Option<String>,
        /// Substring matching for --name (case-insensitive) instead of exact equality.
        #[arg(long = "name-contains", alias = "name-substr")]
        name_contains: bool,
        /// Include references, not just definitions.
        #[arg(long)]
        refs: bool,
    },
    /// Print the full source of a symbol, by id or by file + name.
    Source {
        /// A symbol id from outline/symbols, e.g. rust:src/lib.rs#parse@41
        id_or_file: String,
        /// If the first arg is a file, the symbol name to find in it.
        name: Option<String>,
    },
    /// Report ERROR / MISSING nodes — a post-edit syntax check.
    Check { file: PathBuf },
    /// Find references to a symbol across a directory (structural + textual).
    Callers {
        /// The symbol name to find references to.
        name: String,
        /// Directory to search.
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
    },
    /// Compact structural map: definitions and their references, no source bodies.
    Map {
        dir: PathBuf,
        /// Only definitions of this kind (e.g. function, class, method).
        #[arg(long)]
        kind: Option<String>,
        /// Only definitions whose name equals this exactly (case-insensitive).
        #[arg(long)]
        name: Option<String>,
        /// Substring matching for --name (case-insensitive) instead of exact equality.
        #[arg(long = "name-contains", alias = "name-substr")]
        name_contains: bool,
    },
    /// Find where a symbol is defined (go-to-def), by name or by position.
    ///
    /// `name` is an exact-name lookup (may return several candidates). `--at` is
    /// the precise mode: it resolves the identifier under a usage site,
    /// scope-aware (a local/parameter shadows a same-named global) and following
    /// imports across files, falling back to name lookup when it can't resolve.
    Definition {
        /// The symbol name to resolve (omit when using --at).
        name: Option<String>,
        /// Directory to search.
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        /// Resolve the identifier at a usage site: file:line:col (1-based).
        /// Scope-aware + cross-file; prefer this when you have a position.
        #[arg(long)]
        at: Option<String>,
    },
    /// Set up grove in a project: fetch grammars + grove.lock, and wire up the
    /// chosen integration (MCP server, the cross-harness skill, or both).
    Init {
        /// Project directory (default: current).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Which integration to set up: mcp (default), skill, or both.
        #[arg(long = "as", value_enum, default_value_t = init::Target::Mcp)]
        target: init::Target,
        /// Show what would be detected/written without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Download grammars from the hosted registry into the OS cache.
    Fetch {
        /// Languages to fetch (default: all in the catalog).
        langs: Vec<String>,
        /// Re-download even if already cached.
        #[arg(long)]
        force: bool,
    },
    /// Build registry artifacts from curated source specs (registry maintainer step).
    Ingest {
        /// Only these languages (default: all in the sources file).
        only: Vec<String>,
        /// Curated sources spec.
        #[arg(long, default_value = "registry-sources.json")]
        sources: PathBuf,
        /// Output registry directory.
        #[arg(long, default_value = "registry")]
        out: PathBuf,
    },
    /// Build the hosted catalog (index.json) for a registry directory.
    Index {
        /// Registry directory to index (default: the resolved registry root).
        dir: Option<PathBuf>,
        /// Output path (default: <dir>/index.json).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Serve grammar.wasm as release assets from this base URL (GitHub Releases).
        #[arg(long)]
        release_base: Option<String>,
    },
    /// Show the resolved registry location and search order.
    Registry,
    /// List the languages available in the registry.
    Languages,
    /// Write grove.lock pinning each registry grammar's version + content hash.
    Lock,
    /// Run as an MCP server over stdio (the agent-facing face).
    Serve {
        /// Project directory used to locate .grove/explore.json (default: current dir).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Force explore mode even if .grove/explore.json is absent.
        #[arg(long = "explore")]
        explore: bool,
        /// Force standard structural mode (ignore .grove/explore.json if present).
        #[arg(long = "standard")]
        standard: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Outline { file, kind, detail } => {
            let syms = ops::outline(&file, kind.as_deref())?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&ops::project(&syms, detail))?);
            } else {
                for s in &syms {
                    let owner = s.parent.clone().unwrap_or_default();
                    println!("{:<10} {:<26} {:<18} {}:{:<4} {}", s.kind, s.name, owner, s.line, s.col, s.signature);
                }
                eprintln!("\n{} definitions · {}", syms.len(), ops::rel(&file));
            }
        }
        Cmd::Symbols { dir, kind, name, name_contains, refs } => {
            let syms = ops::symbols(&dir, kind.as_deref(), name.as_deref(), refs, name_contains)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&syms)?);
            } else {
                for s in &syms {
                    let mark = if s.is_definition { "def" } else { "ref" };
                    println!("{:<3} {:<10} {:<28} {}", mark, s.kind, s.name, s.id);
                }
                eprintln!("\n{} symbols", syms.len());
            }
        }
        Cmd::Source { id_or_file, name } => {
            let res = ops::source(&id_or_file, name.as_deref())?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                if !res.other_candidates.is_empty() {
                    eprintln!("(also matched: {})", res.other_candidates.join(", "));
                }
                println!("{}", res.source);
            }
        }
        Cmd::Check { file } => {
            let defects = ops::check(&file)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&defects)?);
            } else if defects.is_empty() {
                println!("ok · no syntax errors · {}", ops::rel(&file));
            } else {
                for d in &defects {
                    println!("{:<8} {}:{:<4} `{}`", d.kind, d.line, d.col, d.text);
                }
                eprintln!("\n{} defect(s) · {}", defects.len(), ops::rel(&file));
                std::process::exit(1);
            }
        }
        Cmd::Callers { name, dir } => {
            let sites = ops::callers(&dir, &name)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&sites)?);
            } else {
                for s in &sites {
                    let inf = s.in_function.as_deref().unwrap_or("<top-level>");
                    // Lead with `path:line:col` so the location is greppable and,
                    // for a directory-wide query, you can tell which file it's in (#29).
                    // [S]/[T] = structural (tree-sitter) vs textual (grep) provenance (#33).
                    let tag = if s.source == "structural" { "S" } else { "T" };
                    println!("{}:{}:{}   {:<28} [{}] {}", s.file, s.line, s.col, inf, tag, s.text);
                }
                eprintln!("\n{} reference(s) of `{}` (S=structural, T=textual)", sites.len(), name);
            }
        }
        Cmd::Map { dir, kind, name, name_contains } => {
            let maps = ops::map(&dir, kind.as_deref(), name.as_deref(), name_contains)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&maps)?);
            } else {
                for fm in &maps {
                    println!("{}", fm.file);
                    for e in &fm.entries {
                        let parent = e.parent.as_deref().unwrap_or("");
                        if e.references.is_empty() {
                            println!("  {:<10} {:<26} {:<18} {:<4} {}", e.kind, e.name, parent, e.row, e.signature);
                        } else {
                            println!("  {:<10} {:<26} {:<18} {:<4} {}  → {}", e.kind, e.name, parent, e.row, e.signature, e.references.join(", "));
                        }
                    }
                }
                let total: usize = maps.iter().map(|fm| fm.entries.len()).sum();
                eprintln!("\n{} definitions across {} files", total, maps.len());
            }
        }
        Cmd::Definition { name, dir, at } => {
            let (resolved, defs) = match at {
                Some(pos) => {
                    let (file, row, col) = ops::parse_pos(&pos)?;
                    ops::definition_at(&file, row, col, &dir)?
                }
                None => {
                    let name = name.ok_or_else(|| anyhow::anyhow!("provide a name or --at"))?;
                    (name.clone(), ops::definition(&dir, &name)?)
                }
            };
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&defs)?);
            } else {
                for s in &defs {
                    let owner = s.parent.clone().unwrap_or_default();
                    // `definition` searches a whole dir, so results can span
                    // files — lead with `file:line:col` so the caller knows
                    // which file each hit is in without a follow-up `symbols`.
                    let loc = format!("{}:{}:{}", s.file, s.line, s.col);
                    println!("{:<10} {:<26} {:<18} {:<28} {}", s.kind, s.name, owner, loc, s.signature);
                }
                eprintln!("\n{} definition(s) of `{}`", defs.len(), resolved);
            }
        }
        Cmd::Init { path, target, dry_run } => init::run(&path, target, dry_run)?,
        Cmd::Fetch { langs, force } => fetch::run(&langs, force)?,
        Cmd::Ingest { only, sources, out } => ingest::run(&sources, &out, &only)?,
        Cmd::Index { dir, output, release_base } => {
            let (out, n) = registry::write_index(dir, output, release_base.as_deref())?;
            println!("wrote {} ({n} grammars)", out.display());
        }
        Cmd::Registry => {
            println!("registry root: {}\n", registry::root().display());
            println!("search order (first existing wins):");
            for c in registry::search_path() {
                let mark = if c.exists { "✓" } else { "·" };
                println!("  {mark} {:<26} {}", c.source, c.path.display());
            }
        }
        Cmd::Languages => {
            let manifests = registry::manifests();
            if cli.json {
                let v: Vec<_> = manifests
                    .iter()
                    .map(|m| serde_json::json!({"name": m.name, "version": m.version, "extensions": m.extensions}))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                for m in &manifests {
                    println!("{:<12} {:<10} .{}", m.name, m.version, m.extensions.join(" ."));
                }
                eprintln!("\n{} language(s) in registry", manifests.len());
            }
        }
        Cmd::Lock => {
            let lock = registry::write_lock(std::path::Path::new("grove.lock"))?;
            println!("wrote grove.lock ({} grammars)", lock);
        }
        Cmd::Serve { path, explore, standard } => mcp::serve(&path, explore, standard)?,
    }
    Ok(())
}
