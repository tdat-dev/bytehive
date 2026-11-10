//! What a member of the company is launched with (launch.ts), and the files it
//! reads at start (engine.ts personSpec, refreshFiles). Claude only for now; Codex
//! and OpenCode have different flags and come later.

use super::laws::{FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS};
use super::org::person_by_id;
use super::roles::{role_skills, write_new};
use super::store;
use super::types::{Company, Person, Rank, RoleDef};
use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};

/// A rank a newer build wrote gets the staff laws: the fewest rights.
fn laws(rank: &Rank) -> &'static str {
    match rank {
        Rank::Founder => FOUNDER_LAWS,
        Rank::Lead => LEAD_LAWS,
        Rank::Staff | Rank::Other(_) => STAFF_LAWS,
    }
}

pub fn compose_prompt(person: &Person, role: &RoleDef, c: &Company) -> String {
    // An empty name says nothing, as the TS's truthiness has it.
    let boss = person.boss.as_deref().and_then(|b| person_by_id(c, b)).map(|p| p.name.as_str()).filter(|n| !n.is_empty());
    let floor = c.floors.iter().find(|f| Some(&f.id) == person.floor.as_ref()).map(|f| f.name.as_str()).filter(|n| !n.is_empty());
    let place = [
        format!("Your name is {}.", person.name),
        boss.map_or_else(|| "You answer only to the person who owns the company.".into(), |b| format!("Your boss is {b}.")),
        floor.map(|f| format!("You work on the {f} floor.")).unwrap_or_default(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    format!("{}\n\n{place}\n\n{}", laws(&person.rank), role.prompt)
}

/// Where a Founder with no pane starts: the active project's folder, else the first
/// open project with a folder. It works through MCP tools, so the folder limits nothing.
pub fn founder_folder(active: Option<Option<&str>>, open: &[Option<&str>]) -> Option<String> {
    active.flatten().or_else(|| open.iter().flatten().next().copied()).map(String::from)
}

const NEEDS_NODE: &str = "ByteHive needs Node.js 22.13 or newer to run the Company.";

/// How `node --version` went.
pub enum NodeRun {
    Printed { printed: String, code: Option<i32> },
    /// It could not be run, or took too long.
    Failed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum NodeVerdict {
    Ok,
    /// Words for the owner: no job until Node is right.
    Refuse(String),
    /// Says nothing about Node: log it and go on.
    Log(String),
}

/// What `node --version` says about giving a job, as it prints v22.13.0. The MCP
/// server needs that, from mcp/package.json. Only a clear answer refuses: a version
/// too old, or no output from a run that failed. Anything else says nothing about Node.
pub fn node_verdict(r: &NodeRun) -> NodeVerdict {
    let (printed, code) = match r {
        NodeRun::Failed(why) => return NodeVerdict::Log(format!("the Node.js check failed: {why}")),
        NodeRun::Printed { printed, code } => (printed.trim(), *code),
    };
    if printed.is_empty() {
        if matches!(code, Some(0) | None) {
            let code = code.map_or("null".into(), |c| c.to_string());
            return NodeVerdict::Log(format!("the Node.js check printed nothing, exit code {code}"));
        }
        return NodeVerdict::Refuse(format!("{NEEDS_NODE} Node.js was not found. If you just installed it, restart ByteHive."));
    }
    let Some((major, minor)) = version(printed) else {
        return NodeVerdict::Log(format!("the Node.js check printed something else: {printed}"));
    };
    if major > 22 || (major == 22 && minor >= 13) { NodeVerdict::Ok } else { NodeVerdict::Refuse(format!("{NEEDS_NODE} Found {printed}.")) }
}

/// The major and minor of a text that starts like v22.13.
fn version(text: &str) -> Option<(u64, u64)> {
    let mut parts = text.strip_prefix('v')?.splitn(3, '.');
    let number = |s: &str| (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten();
    let (major, minor) = (number(parts.next()?)?, number(parts.next()?)?);
    parts.next()?;
    Some((major, minor))
}

/// The Founder's settings file: its hooks stay off, everyone else keeps the owner's.
pub const FOUNDER_SETTINGS: &str = r#"{"disableAllHooks":true}"#;

/// Where a person's launch files are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonFiles {
    pub prompt: PathBuf,
    pub mcp: PathBuf,
    /// Only a role with skills has a plugin.
    pub plugin: Option<PathBuf>,
    /// Only the Founder's.
    pub settings: Option<PathBuf>,
}

/// No --strict-mcp-config: the owner's other servers keep working. When they have
/// a `bytehive` of their own, Claude starts this one instead.
pub fn args(role: &RoleDef, files: &PersonFiles) -> Vec<String> {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let mut args = vec!["--append-system-prompt-file".into(), path(&files.prompt), "--mcp-config".into(), path(&files.mcp)];
    if let Some(dir) = &files.plugin {
        args.extend(["--plugin-dir".into(), path(dir)]);
    }
    if !role.deny_tools.is_empty() {
        args.extend(["--disallowedTools".into(), role.deny_tools.join(",")]);
    }
    if let Some(settings) = &files.settings {
        args.extend(["--settings".into(), path(settings)]);
    }
    args
}

/// Who the person is to the MCP server, and the company root when it is not the
/// usual one under `home`, so the server reads the same company.
pub fn env(person: &Person, root: &Path, home: &str) -> Vec<(String, String)> {
    let mut env = vec![("BYTEHIVE_AGENT_ID".to_string(), person.id.clone())];
    let root = root.to_string_lossy().into_owned();
    if root != store::company_root(home) {
        env.push(("BYTEHIVE_COMPANY_ROOT".into(), root));
    }
    env
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct McpFile<'a> {
    mcp_servers: Servers<'a>,
}

#[derive(Serialize)]
struct Servers<'a> {
    bytehive: Server<'a>,
}

#[derive(Serialize)]
struct Server<'a> {
    command: &'a str,
    args: [&'a str; 1],
}

/// Starts the server the app carries, so a member's tools match the app on any machine.
pub fn mcp_config(server: &str) -> String {
    let file = McpFile { mcp_servers: Servers { bytehive: Server { command: "node", args: [server] } } };
    serde_json::to_string_pretty(&file).expect("plain strings")
}

#[derive(Serialize)]
struct Manifest {
    name: String,
    version: &'static str,
}

pub fn plugin_manifest(person: &Person) -> String {
    let mut safe = String::new();
    for ch in person.id.chars() {
        if ch.is_ascii_alphanumeric() {
            safe.push(ch);
        } else if !safe.ends_with('-') {
            safe.push('-');
        }
    }
    let name = format!("bytehive-role-{}", safe.trim_matches('-').to_lowercase());
    serde_json::to_string_pretty(&Manifest { name, version: "1.0.0" }).expect("plain strings")
}

fn person_dir(root: &Path, person: &Person) -> PathBuf {
    root.join("people").join(&person.id)
}

fn write_prompt(root: &Path, p: &Person, role: &RoleDef, c: &Company) -> io::Result<PathBuf> {
    let path = person_dir(root, p).join("prompt.md");
    write_new(&path, &compose_prompt(p, role, c))?;
    Ok(path)
}

fn write_mcp(root: &Path, p: &Person, server: &Path) -> io::Result<PathBuf> {
    let path = person_dir(root, p).join("mcp.json");
    write_new(&path, &mcp_config(&server.to_string_lossy()))?;
    Ok(path)
}

/// A person's plugin from their role's skills. Gives back its folder, or None with no skills.
fn write_plugin(root: &Path, p: &Person, role: &RoleDef) -> io::Result<Option<PathBuf>> {
    let skills = role_skills(root, role);
    if skills.is_empty() {
        return Ok(None);
    }
    let base = person_dir(root, p).join("plugin");
    write_new(&base.join(".claude-plugin").join("plugin.json"), &plugin_manifest(p))?;
    for s in skills {
        write_new(&base.join("skills").join(&s.name).join("SKILL.md"), &s.text)?;
    }
    Ok(Some(base))
}

fn write_settings(root: &Path, p: &Person) -> io::Result<PathBuf> {
    let path = person_dir(root, p).join("settings.json");
    write_new(&path, FOUNDER_SETTINGS)?;
    Ok(path)
}

/// Writes a person's prompt, MCP config, plugin and (the Founder's) settings, and
/// gives back where. The Founder has no shell and needs no hooks.
pub fn person_files(root: &Path, person: &Person, role: &RoleDef, c: &Company, mcp_server: &Path) -> io::Result<PersonFiles> {
    let prompt = write_prompt(root, person, role, c)?;
    let mcp = write_mcp(root, person, mcp_server)?;
    let plugin = write_plugin(root, person, role)?;
    let settings = if person.rank == Rank::Founder { Some(write_settings(root, person)?) } else { None };
    Ok(PersonFiles { prompt, mcp, plugin, settings })
}

/// What a person's prompt says about their boss changes with the floor, their MCP
/// config follows the app, and the owner can edit a skill: whoever restarts reads
/// these files again. Gives back whose could not be written.
pub fn refresh_files(root: &Path, people: &[Person], roles: &[RoleDef], c: &Company, mcp_server: &Path) -> Vec<String> {
    let mut failed = Vec::new();
    for p in people {
        let Some(role) = roles.iter().find(|r| r.id == p.role) else { continue };
        let name = &p.name;
        if let Err(e) = write_prompt(root, p, role, c) {
            failed.push(format!("{name}'s prompt was not updated: {e}."));
        }
        if let Err(e) = write_mcp(root, p, mcp_server) {
            failed.push(format!("{name}'s MCP config was not updated: {e}."));
        }
        if let Err(e) = write_plugin(root, p, role) {
            failed.push(format!("{name}'s skills were not updated: {e}."));
        }
        if p.rank == Rank::Founder
            && let Err(e) = write_settings(root, p)
        {
            failed.push(format!("{name}'s settings were not updated: {e}."));
        }
    }
    failed
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
