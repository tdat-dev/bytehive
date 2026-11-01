//! The headless core every ByteHive UI talks to: spawn CLI agents on a PTY,
//! read their output, write their input, mirror them to remote terminals.
//!
//! No UI framework here, no Tauri: the desktop webview app, the native shell
//! and (later) the mobile remote all share this one implementation.
//!
//! ## The contract, in brief
//!
//! - [`Registry`] owns every running agent by id. Dropping an agent kills its
//!   whole process tree (a kill-on-close Job on Windows).
//! - [`Agent::tap`] mirrors live output to one extra reader. A tap that lags
//!   past its bound is dropped, never queued forever; it re-taps and replays.
//! - Each agent keeps a bounded scrollback a newly attached UI replays, so a
//!   fresh window paints immediately without the PTY resending history.
//! - Errors are [`anyhow::Result`]; each UI maps them to its own error shape.

pub mod agent;
pub mod claude;
pub mod codex;
pub mod command_spec;
pub mod error;
pub mod git;
pub mod gitfiles;
#[cfg(windows)]
pub mod job;
pub mod land;
pub mod launch;
pub mod muse;
pub mod opencode;
pub mod planusage;
#[cfg(windows)]
pub mod pipe_session;
mod record;
pub mod pty_session;
pub mod registry;
pub mod transcript;
pub mod usage;

pub use agent::{Agent, Sink};
pub use command_spec::CommandSpec;
pub use error::CommandError;
pub use registry::{lock, Registry};
pub use transcript::TranscriptChunk;
