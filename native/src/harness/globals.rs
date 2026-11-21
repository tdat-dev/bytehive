//! The CLIs that read the bytehive MCP server only from their own config file:
//! ByteHive keeps its one entry there pointing at the installed app's server.
//!
//! Found 2026-11-21: OpenCode's and Antigravity's entries pointed at an old dev
//! copy (D:/bytehive/mcp/dist), Antigravity's with one project folder fixed in,
//! so their agents had an old board and no browser recording. OpenCode's own
//! `mcp add` wrote a v2 entry into the v1 file beside the old one, so the file
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
    entry: Value,
    /// The file when the CLI has none yet.
    fresh: Value,
}

fn targets(home: &Path, server: &str) -> Vec<Target> {
    vec![
        Target {
            cli: "opencode",
            file: home.join(".config").join("opencode").join("opencode.json"),
            key: "mcp",
            entry: json!({ "type": "local", "command": ["node", server], "enabled": true }),
            fresh: json!({ "$schema": "https://opencode.ai/config.json" }),
        },
        Target {
            cli: "agy",
            file: home.join(".gemini").join("antigravity").join("mcp_config.json"),
            key: "mcpServers",
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
    let Some(next) = set_member(&text, t.key, "bytehive", &t.entry)? else { return Ok(false) };
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
