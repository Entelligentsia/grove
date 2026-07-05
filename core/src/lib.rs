//! grove-core — the structural code-intelligence library behind the grove CLI
//! and MCP server.
//!
//! This crate hosts the tree-sitter AST engine, the grammar registry, grammar
//! fetching, and source ingest. It is `clap`-free: command-line concerns live in
//! the `grove` binary crate, which consumes this library via `grove_core::`.
//!
//! # Overview
//!
//! The consumer-facing surface is the [`ops`] module — a small set of structural
//! queries that work for **any** registered language (grammars load from the
//! [`registry`] as wasm, so nothing is compiled in):
//!
//! - [`ops::outline`] — the definitions in one file (its symbol skeleton).
//! - [`ops::symbols`] — find symbols across a directory, gitignore-aware.
//! - [`ops::source`] — the full source text of one symbol, by id or name.
//! - [`ops::check`] — the syntactic defects (ERROR / MISSING) in one file.
//! - [`ops::callers`] — every reference to a name, with its enclosing function.
//! - [`ops::map`] — a directory's definitions and their outgoing references.
//! - [`ops::definition`] / [`ops::definition_at`] — go-to-def by name or use site.
//!
//! [`init::provision_project`] is the grammar-provisioning entry point used by
//! `grove init`. The lower-level [`engine`], [`fetch`], and [`ingest`] modules are
//! public for hosts that need deeper access.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use grove_core::ops;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Every definition under `src/`, gitignore-aware.
//! let symbols = ops::symbols(Path::new("src"), None, None, false, false)?;
//! for s in &symbols {
//!     println!("{} {} — {}:{}", s.kind, s.name, s.file, s.line);
//! }
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod doctor;
pub mod engine;
pub mod explore;
pub mod fetch;
pub mod harness;
pub mod ingest;
pub mod init;
pub mod ops;
pub mod registry;
pub mod render;

// ---- Curated public surface ----
//
// Root re-exports so consumers can name the common return types and entry points
// directly (e.g. `grove_core::Symbol`) instead of reaching through module paths.
// The full module set above stays public for callers that need deeper access;
// internal helpers (`Loaded`, `CapturedQuery`, `Index`, `Sources`, `Spec`,
// `Catalog`, …) remain private to their modules and are not re-exported.

/// Core symbol/defect types extracted by the [`engine`].
pub use engine::{Defect, Symbol};
/// Return types of the [`ops`] structural queries.
pub use ops::{CallSite, FileMap, MapEntry, SourceResult};
/// The grammar-provisioning entry point behind `grove init` (see [`init`]).
pub use init::provision_project;
/// The explore subsystem configuration model (see [`explore`]).
pub use explore::{ExploreConfig, Provider, Steering};
/// The grove project config type and integration mode (see [`config`]).
pub use config::{active_mode, GroveConfig, Mode, ModeChoice};
/// Lock-file verification types returned by [`registry::verify_lock`].
pub use registry::{LockVerifyEntry, LockVerifyStatus};
