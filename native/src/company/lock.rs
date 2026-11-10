//! Two ByteHive apps running at once, the installed one and a dev build say, must
//! not both run the company: each would read the requests and start everyone. The
//! one that runs it keeps engine.lock fresh, and the other stays out while it does.
//! A silent lock is not a free one: an app that went quiet still has its panes open,
//! and a new owner would start everyone a second time. So a stale lock is taken only
//! from a dead process. A live one gets ten minutes, and then the owner is told.

use super::store::{missing, parse_owned, write_whole};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

const LOCK: &str = "engine.lock";

/// `at` is the last heartbeat, in ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineLock {
    pub pid: u32,
    pub at: i64,
}

/// What a read of the lock gave. Unknown is a read that failed, which says nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockRead {
    Held(EngineLock),
    Free,
    Unknown,
}

/// What this app does on a beat. Take-over is a run the owner has to hear about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockMove {
    Run,
    TakeOver,
    StayOut,
    Skip,
}

/// No heartbeat for this long: whoever held the lock has stopped, or is dead.
pub const LOCK_STALE_MS: i64 = 15_000;

/// No heartbeat from a living app for this long: it is stuck, and the company won't wait.
pub const LOCK_ABANDONED_MS: i64 = 600_000;

pub fn parse_lock(text: &str) -> Option<EngineLock> {
    parse_owned(text).ok()
}

/// Another app's lock has gone quiet, so it matters whether that app is alive. A
/// heartbeat from far ahead of the clock counts too: it would never age.
pub fn lock_silent(lock: &EngineLock, me: u32, now: i64) -> bool {
    lock.pid != me && (now - lock.at).abs() >= LOCK_STALE_MS
}

/// What this app does about the lock. Its own pid is a window that reloaded.
/// `awake_since` is when this app last woke or saw its clock jump: silence only counts
/// from there, or an hour's sleep or a moved clock would look like a stuck app.
pub fn lock_move(read: &LockRead, me: u32, now: i64, holder_alive: bool, awake_since: i64) -> LockMove {
    let lock = match read {
        LockRead::Unknown => return LockMove::Skip,
        LockRead::Free => return LockMove::Run,
        LockRead::Held(l) if l.pid == me => return LockMove::Run,
        LockRead::Held(l) => l,
    };
    if !lock_silent(lock, me, now) {
        return LockMove::StayOut;
    }
    if !holder_alive {
        return LockMove::Run;
    }
    if (now - lock.at).abs().min(now - awake_since) >= LOCK_ABANDONED_MS { LockMove::TakeOver } else { LockMove::StayOut }
}

/// A missing file is no lock, and so is one a crash filled with zeros: it reads as
/// binary every time, and only a new lock replaces it. Any other failure says nothing
/// about who holds it: the heartbeat skips that beat and comes round again in moments.
pub fn read(root: &Path) -> LockRead {
    read_result(std::fs::read(root.join(LOCK)))
}

pub(crate) fn read_result(read: io::Result<Vec<u8>>) -> LockRead {
    match read {
        Ok(bytes) if bytes.iter().take(8000).any(|&b| b == 0) => LockRead::Free,
        Ok(bytes) => parse_lock(&String::from_utf8_lossy(&bytes)).map_or(LockRead::Free, LockRead::Held),
        Err(e) if missing(&e) => LockRead::Free,
        Err(_) => LockRead::Unknown,
    }
}

/// Swapped in whole, or another app could read half a lock as none. The temp file
/// has the pid in its name: two apps can be writing at the same moment.
pub fn write(root: &Path, lock: &EngineLock) -> anyhow::Result<()> {
    write_whole(root, &format!("{LOCK}.{}.tmp", lock.pid), LOCK, &serde_json::to_string(lock)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
