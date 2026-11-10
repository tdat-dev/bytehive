//! A fleet seat that is one of the company's people: what it starts with, from the
//! company's files (engine.ts personSpec).

use super::launch::{args, env, person_files};
use super::org::person_by_id;
use super::{roles, server, store};
use std::path::Path;

/// A seat's launch when it is one of the company's people: their files written
/// again, as personSpec does on every start, then their args and env. An error
/// is the reason, for the owner: the seat stays stopped rather than start as a
/// plain agent. Runs on the UI thread (a few small files): Task 9 measures it.
pub fn for_seat(spec: &crate::fleet::Spec) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    let person = spec.person.as_deref().ok_or("it is not one of the company's people")?;
    let root = store::root().ok_or("there is no home folder to find the company in")?;
    let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
    let server = server::mcp_server().ok_or("the app's MCP server (mcp-server\\bytehive-mcp.mjs) is missing. Reinstall ByteHive.")?;
    seat_launch(&root, &home, person, &server)
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
