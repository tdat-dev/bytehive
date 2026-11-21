//! The environment every terminal starts with is the one Windows gives a fresh
//! login, even when ByteHive itself was opened from inside an agent.
//!
//! A terminal inherits ByteHive's whole environment. Opened from a Claude
//! session (Claude Desktop, a Claude Code shell, a ByteHive agent testing a
//! build), ByteHive carried about 60 of that session's variables into every
//! agent: NO_COLOR=1 took the colour out of every CLI,
//! CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1 the titles, GIT_EDITOR=true and
//! GIT_TERMINAL_PROMPT=0 git's editor and prompts, plus that session's ids and
//! socket. Measured on 2026-11-21 from the running app's own environment.
//!
//! So at start, before any thread: when a launch from an agent is seen, the
//! process environment becomes Windows' own (CreateEnvironmentBlock, which reads
//! the system and user variables from the registry, the same as Explorer), and
//! only ByteHive's own settings (BYTEHIVE_*, MAESTRO_*) and Rust's debug
//! switches come along. A normal launch is left as it is.

use std::collections::BTreeMap;

/// Variables only an agent session sets: any one means ByteHive was opened
/// from inside one.
const AGENT_MARKERS: [&str; 6] = ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_SESSION_ID", "AI_AGENT", "BYTEHIVE_AGENT", "CODEX_THREAD_ID"];

/// ByteHive's own identity for one agent: set per agent at spawn, never
/// carried from the session that opened the app.
const PER_AGENT: [&str; 3] = ["BYTEHIVE_AGENT", "BYTEHIVE_WORKSPACE", "BYTEHIVE_RUN_DIR"];

pub fn launched_by_agent(vars: &[(String, String)]) -> bool {
    vars.iter().any(|(k, _)| AGENT_MARKERS.iter().any(|m| k.eq_ignore_ascii_case(m)))
}

/// Kept from the launching environment: the app's settings and Rust's switches.
fn carried(name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    if PER_AGENT.contains(&up.as_str()) {
        return false;
    }
    up.starts_with("BYTEHIVE_") || up.starts_with("MAESTRO_") || up == "RUST_LOG" || up == "RUST_BACKTRACE"
}

/// The environment to run with: Windows' own, plus what is carried. Names are
/// matched without case, as Windows does.
pub fn plan(current: &[(String, String)], windows: &[(String, String)]) -> Vec<(String, String)> {
    let mut out: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (k, v) in windows {
        out.insert(k.to_ascii_uppercase(), (k.clone(), v.clone()));
    }
    for (k, v) in current.iter().filter(|(k, _)| carried(k)) {
        out.insert(k.to_ascii_uppercase(), (k.clone(), v.clone()));
    }
    out.into_values().collect()
}

/// Called first thing in main, while the process has one thread.
pub fn restore() {
    let current: Vec<(String, String)> = std::env::vars_os().filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?))).collect();
    if !launched_by_agent(&current) {
        return;
    }
    let Some(windows) = windows_env() else { return };
    let next = plan(&current, &windows);
    let keep: std::collections::HashSet<String> = next.iter().map(|(k, _)| k.to_ascii_uppercase()).collect();
    // SAFETY: main calls this before GPUI, the hub or any other thread starts,
    // so nothing reads the environment while it changes.
    unsafe {
        for (k, _) in &current {
            if !keep.contains(&k.to_ascii_uppercase()) {
                std::env::remove_var(k);
            }
        }
        for (k, v) in &next {
            std::env::set_var(k, v);
        }
    }
}

/// The environment Windows builds for this user from the registry, with no
/// inheritance from this process. None when Windows will not give it.
#[cfg(windows)]
fn windows_env() -> Option<Vec<(String, String)>> {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{TOKEN_DUPLICATE, TOKEN_IMPERSONATE, TOKEN_QUERY};
    use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: plain Win32 calls; the block is read up to its double NUL and
    // handed back to DestroyEnvironmentBlock, the token is closed.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_IMPERSONATE, &mut token) == 0 {
            return None;
        }
        let mut block: *mut core::ffi::c_void = std::ptr::null_mut();
        let made = CreateEnvironmentBlock(&mut block, token, 0);
        CloseHandle(token);
        if made == 0 || block.is_null() {
            return None;
        }
        let mut vars = Vec::new();
        let mut p = block as *const u16;
        loop {
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            if len == 0 {
                break;
            }
            let entry = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            p = p.add(len + 1);
            // "=C:=C:\..." entries are the shell's per-drive folders, not variables.
            if let Some((k, v)) = entry.split_once('=').filter(|(k, _)| !k.is_empty()) {
                vars.push((k.to_string(), v.to_string()));
            }
        }
        DestroyEnvironmentBlock(block);
        Some(vars)
    }
}

#[cfg(not(windows))]
fn windows_env() -> Option<Vec<(String, String)>> {
    None
}

#[cfg(test)]
#[path = "cleanenv_tests.rs"]
mod tests;
