//! Codex: the laws and the app's MCP server as `-c` overrides for this run.
//!
//! `developer_instructions` is a developer message Codex adds to every turn
//! (tested 2026-11-13). The MCP overrides replace whatever the owner's
//! config.toml says for bytehive (an old dev copy here) with the installed one.
//! Codex starts MCP servers with only a short list of the agent's variables, so
//! env_vars names the ones that say who the agent is (measured with
//! `codex mcp get bytehive --json`). Codex starts through `cmd /c`: no value may
//! hold a double quote, a single quote (the TOML quote here) or a cmd
//! metacharacter, or the agent starts without that piece.

use std::path::Path;

/// The variables the bytehive server reads to know the agent (fleet.rs, browser hub).
const IDENTITY: [&str; 5] = ["BYTEHIVE_AGENT", "BYTEHIVE_WORKSPACE", "BYTEHIVE_RUN_DIR", "BYTEHIVE_HUB_FILE", "BYTEHIVE_AGENT_ID"];

fn safe(value: &str) -> bool {
    !value.chars().any(|c| "\"'%^&|<>!\r\n".contains(c))
}

pub fn args(laws: &str, server: Option<&Path>) -> Vec<String> {
    let mut out = Vec::new();
    if safe(laws) {
        out.extend(["-c".to_string(), format!("developer_instructions='{laws}'")]);
    }
    let path = server.map(|s| s.to_string_lossy().replace('\\', "/"));
    if let Some(path) = path.filter(|p| safe(p) && !p.contains(char::is_whitespace)) {
        let names = IDENTITY.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(",");
        out.extend([
            "-c".to_string(),
            "mcp_servers.bytehive.command='node'".to_string(),
            "-c".to_string(),
            format!("mcp_servers.bytehive.args=['{path}']"),
            "-c".to_string(),
            format!("mcp_servers.bytehive.env_vars=[{names}]"),
        ]);
    }
    out
}
