//! Supervisor intelligence, headless: transcript parsers per CLI plus the
//! task/stuck/done derivations every UI shares. Ported from the TypeScript
//! models with the same shapes, caps, and verdicts.

pub mod ask;
pub mod ask_risk;
pub mod clash;
pub mod claude;
pub mod claude_user;
pub mod codex;
pub mod codex_cell;
pub mod codex_chat;
pub mod codex_patch;
pub mod describe;
pub mod diff_file;
pub mod digest;
pub mod done_check;
pub mod ledgers;
pub mod muse;
pub mod muse_chat;
pub mod opencode;
pub mod opencode_chat;
pub mod review_text;
pub mod screens;
pub mod stuck;
pub mod task_state;
pub mod text;
pub mod typing;
pub mod types;

pub use types::Chat;
