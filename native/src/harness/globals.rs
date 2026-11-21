//! The CLIs that read the bytehive MCP server only from their own config file:
//! ByteHive keeps its one entry there pointing at the installed app's server.
//!
//! Found 2026-11-21: OpenCode's and Antigravity's entries pointed at an old dev
//! copy (D:/bytehive/mcp/dist), Antigravity's with one project folder fixed in,
//! so their agents had an old board and no browser recording. OpenCode's own
//! `mcp add` wrote its entry beside the old one, so the file
//! is edited here instead, one entry, nothing else (jsonedit.rs). The first
//! change keeps a copy of the file as it was (`.bytehive-bak`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};

use super::jsonedit::set_member;

struct Target {
    cli: &'static str,
    file: PathBuf,
    key: &'static str,
    /// The object under `key` that holds the servers, when it is one level down.
    inner: Option<&'static str>,
    entry: Value,
    /// The file when the CLI has none yet.
    fresh: Value,
}

fn targets(home: &Path, server: &str) -> Vec<Target> {
    vec![
        // OpenCode 2 reads mcp.servers.<name> only (Config.MCP {timeout, servers});
        // a 1.x entry straight under mcp is not loaded at all (measured 2.2.5:
        // "No MCP servers configured" with three 1.x entries in the file).
        Target {
            cli: "opencode",
            file: home.join(".config").join("opencode").join("opencode.json"),
            key: "mcp",
            inner: Some("servers"),
            entry: json!({ "type": "local", "command": ["node", server] }),
            fresh: json!({ "$schema": "https://opencode.ai/config.json" }),
        },
        Target {
            cli: "agy",
            file: home.join(".gemini").join("antigravity").join("mcp_config.json"),
            key: "mcpServers",
            inner: None,
            entry: json!({ "command": "node", "args": [server] }),
            fresh: json!({}),
        },
    ]
}

/// Keep one target right: Ok(true) when the file was changed.
fn keep(t: &Target, installed: bool) -> Result<bool, String> {
    let text = match std::fs::read_to_string(&t.file) {
        Ok(text) => text,
        Err(_) if installed => serde_json::to_string_pretty(&t.fresh).expect("json"),
        Err(_) => return Ok(false),
    };
    let edit = match t.inner {
        None => set_member(&text, t.key, "bytehive", &t.entry)?,
        // One level down: the inner object is written whole, with the other
        // servers in it as they were.
        Some(inner) => {
            let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("not plain JSON: {e}"))?;
            let mut servers = match &v[t.key][inner] {
                Value::Object(m) => m.clone(),
                Value::Null => Default::default(),
                _ => return Err(format!("{}.{inner} is not an object", t.key)),
            };
            servers.insert("bytehive".into(), t.entry.clone());
            set_member(&text, t.key, inner, &Value::Object(servers))?
        }
    };
    let Some(next) = edit else { return Ok(false) };
    if t.file.is_file() {
        let bak = t.file.with_extension("json.bytehive-bak");
        if !bak.exists() {
            std::fs::copy(&t.file, &bak).map_err(|e| e.to_string())?;
        }
    } else if let Some(dir) = t.file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // A plain write: Antigravity's file can be a link, which a rename would replace.
    std::fs::write(&t.file, next).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Once per app run: every config-file CLI on this machine points at the app's server.
pub fn sync() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        if cfg!(test) {
            return;
        }
        let (Some(server), Ok(home)) = (crate::company::server::mcp_server(), std::env::var("USERPROFILE")) else { return };
        for t in targets(Path::new(&home), &server.to_string_lossy().replace('\\', "/")) {
            match keep(&t, crate::presets::installed(t.cli)) {
                Ok(true) => eprintln!("[bytehive] {}: the bytehive MCP entry now points at {}", t.cli, server.display()),
                Ok(false) => {}
                Err(e) => eprintln!("[bytehive] {}'s config was left as it is: {e}", t.cli),
            }
        }
    });
}

#[cfg(test)]
#[path = "globals_tests.rs"]
mod tests;
