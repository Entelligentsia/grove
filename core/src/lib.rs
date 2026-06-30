//! grove-core — the structural code-intelligence library behind the grove CLI
//! and MCP server.
//!
//! This crate hosts the tree-sitter AST engine, the grammar registry, grammar
//! fetching, and source ingest. It is `clap`-free: command-line concerns live in
//! the `grove` binary crate, which consumes this library via `grove_core::`.

pub mod engine;
pub mod ops;
pub mod registry;
pub mod fetch;
pub mod ingest;
