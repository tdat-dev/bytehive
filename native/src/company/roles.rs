//! The roles ByteHive ships with (roles.ts). A copy lands in the company's roles
//! folder the first time, and that copy wins, so a role the owner edits stays edited.

use super::store::parse_owned;
use super::types::{Look, Rank, RoleDef};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;

// SHIPPED: every file under native/assets/company/roles, built in by build.rs.
include!(concat!(env!("OUT_DIR"), "/roles_gen.rs"));

/// A skill a role brings: its folder name and its SKILL.md.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub text: String,
}

/// A Windows checkout gives these files CRLF. What ships must not depend on where it was built.
pub(crate) fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn shipped(rel: &str) -> Option<&'static str> {
    SHIPPED.iter().find(|(p, _)| *p == rel).map(|(_, text)| *text)
}

/// A role.json with its prompt beside it, as one RoleDef.
fn with_prompt(json: &str, prompt: String) -> Option<RoleDef> {
    let mut def: Value = parse_owned(json).ok()?;
    def.as_object_mut()?.insert("prompt".into(), Value::String(prompt));
    serde_json::from_value(def).ok()
}

/// The roles the app ships, in the order of their folders.
pub fn default_roles() -> Vec<RoleDef> {
    SHIPPED
        .iter()
        .filter_map(|(path, json)| {
            let dir = path.strip_suffix("/role.json").filter(|d| !d.contains('/'))?;
            with_prompt(json, lf(shipped(&format!("{dir}/prompt.md"))?))
        })
        .collect()
}

/// The skills the app ships for a role, as its role.json lists them.
pub fn skill_files(role: &RoleDef) -> Vec<Skill> {
    role.skills
        .iter()
        .filter_map(|name| {
            let text = shipped(&format!("{}/skills/{name}/SKILL.md", role.id))?;
            Some(Skill { name: name.clone(), text: lf(text) })
        })
        .collect()
}

fn skill_path(root: &Path, role: &str, name: &str) -> std::path::PathBuf {
    root.join("roles").join(role).join("skills").join(name).join("SKILL.md")
}

/// A role's skills from the owner's roles folder, the copy they edit. One that
/// isn't there, like a skill added to a role in an update, comes from the app.
pub fn role_skills(root: &Path, role: &RoleDef) -> Vec<Skill> {
    let shipped = skill_files(role);
    role.skills
        .iter()
        .filter_map(|name| {
            let text = fs::read_to_string(skill_path(root, &role.id, name))
                .ok()
                .or_else(|| shipped.iter().find(|s| &s.name == name).map(|s| s.text.clone()))?;
            (!text.is_empty()).then(|| Skill { name: name.clone(), text })
        })
        .collect()
}

/// Creates the folders on the way and writes the file whole.
pub(crate) fn write_new(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, text)
}

/// role.json as the app writes it: the role without its prompt, keys in the role's order.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RoleFile<'a> {
    id: &'a str,
    title: &'a str,
    department: &'a str,
    rank: &'a Rank,
    cli: &'a str,
    deny_tools: &'a [String],
    look: &'a Look,
    skills: &'a [String],
}

fn role_json(r: &RoleDef) -> io::Result<String> {
    let file = RoleFile { id: &r.id, title: &r.title, department: &r.department, rank: &r.rank, cli: &r.cli, deny_tools: &r.deny_tools, look: &r.look, skills: &r.skills };
    Ok(serde_json::to_string_pretty(&file)?)
}

/// Writes each shipped role the folder does not have yet, never over one it has.
pub fn seed(root: &Path) -> io::Result<()> {
    for role in default_roles() {
        let dir = root.join("roles").join(&role.id);
        if fs::read(dir.join("role.json")).is_ok() {
            continue;
        }
        // The shipped text as it is, so the key order the owner reads is the folder's.
        let json = shipped(&format!("{}/role.json", role.id)).map(|t| lf(t).trim_end().to_string());
        write_new(&dir.join("role.json"), &json.map_or_else(|| role_json(&role), Ok)?)?;
        write_new(&dir.join("prompt.md"), &role.prompt)?;
        for s in skill_files(&role) {
            write_new(&skill_path(root, &role.id, &s.name), &s.text)?;
        }
    }
    Ok(())
}

/// Every role in the folder, by folder name. A role folder missing a file, or with
/// one that does not read, is not offered.
pub fn load(root: &Path) -> io::Result<Vec<RoleDef>> {
    let mut entries: Vec<_> = fs::read_dir(root.join("roles"))?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    Ok(entries
        .iter()
        .filter_map(|e| {
            let json = fs::read_to_string(e.path().join("role.json")).ok()?;
            let prompt = fs::read_to_string(e.path().join("prompt.md")).ok()?;
            with_prompt(&json, prompt)
        })
        .collect())
}

/// Writes the roles a plan drafted, now the owner has approved them (engine.ts
/// saveRoles). Gives back which could not be.
pub fn save(root: &Path, roles: &[RoleDef]) -> Vec<String> {
    roles
        .iter()
        .filter_map(|role| {
            let dir = root.join("roles").join(&role.id);
            let wrote = role_json(role)
                .and_then(|json| write_new(&dir.join("role.json"), &json))
                .and_then(|()| write_new(&dir.join("prompt.md"), &role.prompt));
            wrote.err().map(|e| format!("Role {} was not saved: {e}.", role.id))
        })
        .collect()
}

#[cfg(test)]
#[path = "roles_tests.rs"]
mod tests;
