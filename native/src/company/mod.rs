//! The Company: an AI company the owner runs, one floor per project. Ported from
//! the Tauri app's src/company/*.ts, files on disk in the same format, so the
//! bundled MCP server and the owner's company carry over unchanged.

// Nothing calls in yet: the engine is the first caller.
#![allow(dead_code)]

pub mod alerts;
pub mod drive;
pub mod engine;
pub mod fleet_host;
pub mod host;
pub mod launch;
pub mod laws;
pub mod lock;
pub mod needs;
pub mod org;
pub mod reduce;
pub mod requests;
pub mod roles;
pub mod seat;
pub mod server;
pub mod store;
pub mod trust;
pub mod types;
pub mod watch;
