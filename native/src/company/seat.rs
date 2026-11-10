//! A fleet seat that is one of the company's people: what it starts with, from the
//! company's files (engine.ts personSpec).

use super::launch::{args, env, person_files};
use super::org::person_by_id;
use super::{roles, server, store};
use std::path::Path;
use std::sync::Mutex;

/// What a seat starts with.
type Launch = Result<(Vec<String>, Vec<(String, String)>), String>;

/// Launches made ready off the UI thread, by seat id, for the next start to take.
static PREPARED: Mutex<Vec<(String, Launch)>> = Mutex::new(Vec::new());

/// Make a seat's launch ready: the company read, the person's files written, the
/// args worked out. Blocking: run it off the UI thread before the seat starts.
pub fn prepare(seat: &str, person: &str) {
    let launch = launch_for(person);
    let mut ready = PREPARED.lock().unwrap_or_else(|p| p.into_inner());
    ready.retain(|(id, _)| id != seat);
    ready.push((seat.to_string(), launch));
}

/// The launch made ready for this seat, once.
fn take_prepared(seat: &str) -> Option<Launch> {
    let mut ready = PREPARED.lock().unwrap_or_else(|p| p.into_inner());
    let at = ready.iter().position(|(id, _)| id == seat)?;
    Some(ready.remove(at).1)
}

fn launch_for(person: &str) -> Launch {
    let root = store::root().ok_or("there is no home folder to find the company in")?;
    let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
    let server = server::mcp_server().ok_or("the app's MCP server (mcp-server\\bytehive-mcp.mjs) is missing. Reinstall ByteHive.")?;
    seat_launch(&root, &home, person, &server)
}

/// A seat's launch when it is one of the company's people: their files written
/// again, as personSpec does on every start, then their args and env. An error
/// is the reason, for the owner: the seat stays stopped rather than start as a
/// plain agent. A start the Company made ready off the UI thread (`prepare`)
/// only takes its launch; a Resume from the queue does the work here.
pub fn for_seat(spec: &crate::fleet::Spec) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    let person = spec.person.as_deref().ok_or("it is not one of the company's people")?;
    if let Some(ready) = take_prepared(&spec.id) {
        return ready;
    }
    launch_for(person)
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
