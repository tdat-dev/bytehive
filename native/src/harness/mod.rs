//! ByteHive's harness: the one set every agent it starts gets, whatever its CLI.
//!
//! Four pieces, each given the strongest way that CLI allows:
//!
//! - the laws (crate::laws): how to work with the board, the fleet and memory;
//! - the bytehive MCP server of the installed app (board, fleet, memory, the
//!   browser), never an old copy a config file still points at;
//! - the guards (crate::guard): commands that destroy work are refused, a turn
//!   that changed code runs a check before it ends;
//! - who the agent is (BYTEHIVE_AGENT and the rest, set in fleet.rs).
//!
//! Given per run where the CLI has a flag for it, so nothing of the owner's is
//! touched and the app's own copy always wins. Where a CLI reads a piece only
//! from its own config file, ByteHive keeps its one entry in that file right
//! (globals.rs). `coverage` says which way each CLI gets each piece, and the
//! tests fail if any CLI loses one. Measured 2026-11-21: docs/superpowers/specs/
//! 2026-11-21-agent-harness.md.

mod antigravity;
mod codex;
mod globals;
mod jsonedit;

use std::path::{Path, PathBuf};

pub use globals::sync as sync_globals;

/// How one CLI gets one piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// A flag or a file for this run only.
    PerRun,
    /// ByteHive's entry in the CLI's own config, kept right at every start.
    KeptInConfig,
    /// The bytehive MCP server's own instructions, read when it connects.
    FromMcp,
    /// The CLI offers no way that was found to work.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coverage {
    pub laws: Way,
    pub mcp: Way,
    pub guards: Way,
}

/// Which way each CLI gets each piece. Identity is every CLI's (fleet.rs env).
pub fn coverage(program: &str) -> Coverage {
    use Way::*;
    match program {
        "claude" => Coverage { laws: PerRun, mcp: PerRun, guards: PerRun },
        "codex" => Coverage { laws: PerRun, mcp: PerRun, guards: PerRun },
        // Its rules file is per run; --settings takes no MCP or hooks (measured).
        "agy" | "antigravity" => Coverage { laws: PerRun, mcp: KeptInConfig, guards: None },
        // Its TUI talks to a shared background service, which never sees a
        // per-run config (measured), so the MCP entry lives in opencode.json.
        "opencode" => Coverage { laws: FromMcp, mcp: KeptInConfig, guards: None },
        _ => Coverage { laws: None, mcp: None, guards: None },
    }
}

/// What a run of `program` needs: args to add, and a file to write before it starts.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub args: Vec<String>,
    pub file: Option<(PathBuf, String)>,
    /// A path, relative to the run folder, to keep out of `git status`.
    pub exclude: Option<String>,
}

/// What the plan is built from: the laws, the app's MCP server, the guards' args
/// and the harness folder, each None when this machine has no such thing.
pub struct Inputs<'a> {
    pub laws: &'a str,
    pub server: Option<&'a Path>,
    pub mcp_file: Option<&'a Path>,
    pub claude_guard: Vec<String>,
    pub codex_guard: Vec<String>,
}

pub fn plan(program: &str, run_dir: &str, i: &Inputs) -> Plan {
    match program {
        "claude" => {
            let mut args = vec!["--append-system-prompt".to_string(), i.laws.to_string()];
            args.extend(i.claude_guard.iter().cloned());
            if let Some(f) = i.mcp_file {
                // A per-run server named bytehive wins over the owner's global
                // one of the same name (measured: source "dynamic").
                args.extend(["--mcp-config".to_string(), f.to_string_lossy().into_owned()]);
            }
            Plan { args, ..Default::default() }
        }
        "codex" => {
            let mut args = i.codex_guard.clone();
            args.extend(codex::args(i.laws, i.server));
            Plan { args, ..Default::default() }
        }
        "agy" | "antigravity" => antigravity::plan(run_dir, i.laws),
        _ => Plan::default(),
    }
}

/// The plan for a real run: the app's own server and guards, files written.
pub fn for_run(program: &str, role: Option<&str>, run_dir: &str) -> Result<Plan, String> {
    let server = server();
    let mcp_file = server.as_deref().and_then(claude_mcp_file);
    let inputs = Inputs {
        laws: crate::laws::for_role(role),
        server: server.as_deref(),
        mcp_file: mcp_file.as_deref(),
        claude_guard: crate::guard::claude_args(),
        codex_guard: crate::guard::codex_args(),
    };
    let plan = plan(program, run_dir, &inputs);
    if let Some((path, text)) = &plan.file {
        write_file(path, text).map_err(|e| format!("ByteHive's rules for it could not be written: {e}"))?;
    }
    if let Some(rel) = plan.exclude.clone() {
        let dir = run_dir.to_string();
        std::thread::spawn(move || {
            if let Err(e) = crate::gitexclude::keep_out_line(&dir, &rel) {
                eprintln!("[bytehive] {e}");
            }
        });
    }
    if matches!(coverage(program).mcp, Way::KeptInConfig) {
        sync_globals();
    }
    Ok(plan)
}

/// The installed app's MCP server; None in tests, which never read the machine.
fn server() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    crate::company::server::mcp_server()
}

/// Claude's per-run MCP file in the app's data folder, written when it changed.
fn claude_mcp_file(server: &Path) -> Option<PathBuf> {
    let dir = crate::fleet::data_dir()?.join("harness");
    let file = dir.join("claude-mcp.json");
    let text = crate::company::launch::mcp_config(&server.to_string_lossy());
    write_file(&file, &text).map_err(|e| eprintln!("[bytehive] the agents' MCP file was not written: {e}")).ok()?;
    Some(file)
}

/// Write `text` unless the file already holds it.
fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|t| t == text) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
