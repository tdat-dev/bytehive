//! A worktree and branch of its own for an agent (worktree.rs, worktree.ts in
//! today's app). It sits at `<drive>:\.bytehive-worktrees\<repo>-<hash>\<branch>-<hash>`,
//! outside the repo, so the repo's own tools never see it.

use std::os::windows::process::CommandExt as _;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One full checkout at a time: several at once fight over the repo's lock
/// and slow every one of them down.
static ADD_LOCK: Mutex<()> = Mutex::new(());

/// Lowercase, anything else one dash, none at the ends.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// branchName: `bytehive/<name>-<last six of its id>`.
pub fn branch_name(agent: &str, id: &str) -> String {
    let base = slug(agent);
    let short: String = id.chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
    format!("bytehive/{}-{short}", if base.is_empty() { "agent" } else { &base })
}

pub(crate) fn git(args: &[&str], cwd: &str) -> Result<String, String> {
    let out = Command::new("git").args(args).current_dir(cwd).creation_flags(CREATE_NO_WINDOW).output().map_err(|e| format!("git not available: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if why.is_empty() { format!("git {} failed", args.first().unwrap_or(&"")) } else { why });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A Rust repo's worktrees build into the repo's own target folder, so each
/// one doesn't grow its own (about 10 GB for this app).
fn share_cargo_target(parent: &Path, repo_root: &str) {
    let cfg = parent.join(".cargo").join("config.toml");
    let has_rust = ["Cargo.toml", "src-tauri/Cargo.toml"].iter().find(|p| Path::new(repo_root).join(p).exists());
    let Some(manifest) = has_rust else { return };
    if cfg.exists() {
        return;
    }
    let root = repo_root.replace('\\', "/");
    let target = match manifest.split_once('/') {
        Some((dir, _)) => format!("{root}/{dir}/target"),
        None => format!("{root}/target"),
    };
    let body = format!("# Added by ByteHive: every worktree builds into the repo's own target folder.\n[build]\ntarget-dir = \"{target}\"\n");
    let _ = std::fs::create_dir_all(parent.join(".cargo"));
    let _ = std::fs::write(cfg, body);
}

/// Make the worktree on a new branch from the repo's HEAD. Its folder, or
/// what git said.
pub fn add(repo_root: &str, branch: &str) -> Result<String, String> {
    let _serial = ADD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let path = agent_core::git::worktree_path_for(repo_root, branch);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("could not make {}: {e}", parent.display()))?;
        share_cargo_target(parent, repo_root);
    }
    let path = path.to_string_lossy().into_owned();
    git(&["worktree", "add", "-b", branch, &path, "HEAD"], repo_root)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::branch_name;

    #[test]
    fn names_the_branch_after_the_agent() {
        assert_eq!(branch_name("Ana", "f41bcb48-122c-49fb-9cff-cd89bbea7220"), "bytehive/ana-ea7220");
        assert_eq!(branch_name("Agent 13!", "abc"), "bytehive/agent-13-abc");
        assert_eq!(branch_name("", "123456"), "bytehive/agent-123456");
    }
}
