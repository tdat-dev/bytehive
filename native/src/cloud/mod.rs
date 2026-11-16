//! ByteHive Remote's session store, the machine side: sealed copies of every
//! session go to the relay so any device of the account can read them
//! (docs/superpowers/specs/2026-11-13-remote-session-store.md).
#![allow(dead_code)]

pub mod codex;
pub mod crypto;
pub mod items;
pub mod opencode;
pub mod relay;
pub mod secret;
pub mod sync;

#[cfg(test)]
#[path = "e2e_tests.rs"]
mod e2e;
