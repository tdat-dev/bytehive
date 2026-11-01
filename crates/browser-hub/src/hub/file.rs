//! The hub file (~/.maestro/browser-hub.json): who publishes it and when
//! a hub takes it back from one that left.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};

pub fn random_token() -> String {
    let mut s = String::new();
    for _ in 0..2 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

/// The hub file of the installed Maestro: the extension's host always reads
/// this one, whichever build it was copied from.
pub fn hub_file() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    Some(PathBuf::from(home).join(".maestro").join("browser-hub.json"))
}

/// Whether this Maestro is the one the browser talks to. A test build (tauri
/// dev) is not, unless MAESTRO_DEV_BROWSER=1 says so: run beside the installed
/// app it took over the hub file and the browser host, and left both pointing
/// at nothing when it closed. The Supervisor edition is not either: it keeps
/// its own hub file and leaves the installed app talking to the extension.
pub fn owns_browser() -> bool {
    if false {
        return false;
    }
    !cfg!(debug_assertions) || std::env::var("MAESTRO_DEV_BROWSER").map(|v| v == "1").unwrap_or(false)
}

/// Where this Maestro publishes its hub: the installed one's file, or a test
/// build's own. Its agents are told which (MAESTRO_HUB_FILE).
pub fn own_hub_file() -> Option<PathBuf> {
    // The Supervisor edition publishes beside the installed app, never over it.
    if false {
        return hub_file().map(|f| f.with_file_name("browser-hub.supervisor.json"));
    }
    if owns_browser() {
        return hub_file();
    }
    hub_file().map(|f| f.with_file_name("browser-hub.dev.json"))
}

/// Write this hub's port and token where the extension's host and the agents look.
pub(crate) fn publish(port: u16, token: &str) {
    if let Some(f) = own_hub_file() {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&f, json!({ "port": port, "token": token, "pid": std::process::id() }).to_string());
    }
}

/// Whether the hub on port own should write the hub file, given what it says now: it
/// is missing or unreadable, or it names another hub that no longer answers.
/// Another hub that answers keeps it: that Maestro was opened last.
pub(crate) fn should_claim(raw: Option<&str>, own: u16, answers: impl Fn(u16) -> bool) -> bool {
    let named = raw.and_then(|r| serde_json::from_str::<Value>(r).ok()).and_then(|v| v["port"].as_u64()).and_then(|p| u16::try_from(p).ok());
    match named {
        None => true,
        Some(p) if p == own => false,
        Some(p) => !answers(p),
    }
}

/// Something listens on this local port.
pub(crate) fn answers(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_millis(500)).is_ok()
}

pub(crate) fn reply(id: &Value, text: String) -> String {
    json!({ "type": "result", "id": id, "result": { "content": [{ "type": "text", "text": text }] } }).to_string()
}

pub(crate) fn fail(id: &Value, error: String) -> String {
    json!({ "type": "error", "id": id, "error": error }).to_string()
}
