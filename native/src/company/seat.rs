//! A fleet seat that is one of the company's people: what it starts with, from the
//! company's files (engine.ts personSpec).

use super::launch::{args, env, person_files};
use super::org::person_by_id;
use super::{roles, server, store};
use std::path::Path;

/// A seat's launch when it is one of the company's people: their files written
/// again, as personSpec does on every start, then their args and env. None,
/// logged, when the company, the person, their role or the app's MCP server can't
/// be found: the seat then starts as a plain one.
pub fn for_seat(spec: &crate::fleet::Spec) -> Option<(Vec<String>, Vec<(String, String)>)> {
    let person = spec.person.as_deref()?;
    let found = (|| {
        let root = store::root().ok_or("no home folder")?;
        let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
        let server = server::mcp_server().ok_or("the app's MCP server (mcp-server\\bytehive-mcp.mjs) is missing")?;
        seat_launch(&root, &home, person, &server)
    })();
    found.map_err(|e| eprintln!("[bytehive] {}'s Company launch: {e}", spec.name)).ok()
}

fn seat_launch(root: &Path, home: &str, person_id: &str, mcp_server: &Path) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    let company = store::load_company(root).map_err(|e| format!("{e:#}"))?;
    let person = person_by_id(&company, person_id).ok_or_else(|| format!("nobody with id {person_id} works here"))?;
    // A folder that never had roles gets the shipped ones, never over an edited one.
    roles::seed(root).map_err(|e| format!("the roles could not be written: {e}"))?;
    let roles = roles::load(root).map_err(|e| format!("the roles could not be read: {e}"))?;
    let role = roles.iter().find(|r| r.id == person.role).ok_or_else(|| format!("there is no role {}", person.role))?;
    let files = person_files(root, person, role, &company, mcp_server).map_err(|e| format!("their files could not be written: {e}"))?;
    Ok((args(role, &files), env(person, root, home)))
}

#[cfg(test)]
#[path = "seat_tests.rs"]
mod tests;
