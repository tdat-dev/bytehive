pub mod fs;
pub mod screenshot;
pub mod watch;

// The headless core lives in crates/agent-core now; these re-exports keep the
// old `crate::core::…` paths working so the Tauri commands move untouched.
pub use agent_core::agent;
pub use agent_core::command_spec;
#[cfg(windows)]
pub use agent_core::job;
#[cfg(windows)]
pub use agent_core::pipe_session;
pub use agent_core::pty_session;
pub use agent_core::registry;
