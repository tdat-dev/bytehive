//! The Company: an AI company the owner runs, one floor per project. Ported from
//! the Tauri app's src/company/*.ts, files on disk in the same format, so the
//! bundled MCP server and the owner's company carry over unchanged.

// Nothing calls in yet: the engine is the first caller.
#![allow(dead_code)]

pub mod lock;
pub mod store;
pub mod types;
