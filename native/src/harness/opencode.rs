//! OpenCode: its own config folder for the run, with ByteHive's laws as the
//! global AGENTS.md.
//!
//! OpenCode always reads `<config dir>/AGENTS.md` into every request, and on
//! this machine that file is 121 KB: a one-word run cost 56,840 input tokens,
//! 27,968 with a config folder holding the laws instead (measured 2026-11-21,
//! same model). `OPENCODE_CONFIG_DIR` moves the whole config folder, so ByteHive
//! keeps one under its data folder: the owner's config files copied in at each
//! launch, the owner's folders (plugins' node_modules, skills, rules) joined
//! by junctions, and the laws as AGENTS.md. `--standalone` gives the agent a
//! server of its own: the shared background service never sees a per-run
//! variable (measured), and the agent's own server also passes who it is on
//! to the bytehive MCP server.

use std::io;
use std::path::{Path, PathBuf};

use super::Plan;

/// The owner's files that are never copied: a background service's password,
/// the owner's own AGENTS.md, and backups.
fn skipped(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "agents.md" || lower == "service.json" || lower.contains(".bak") || !(lower.ends_with(".json") || lower.ends_with(".jsonc"))
}

/// The run's args and variables, given the prepared folder.
pub fn plan(dir: Option<&Path>) -> Plan {
    let Some(dir) = dir else { return Plan::default() };
    Plan { args: vec!["--standalone".into()], env: vec![("OPENCODE_CONFIG_DIR".into(), dir.to_string_lossy().into_owned())], ..Default::default() }
}

/// The text of the folder's AGENTS.md.
pub fn agents_md(laws: &str) -> String {
    format!("# ByteHive\n\n{laws}\n")
}

/// The owner's OpenCode config folder: theirs if they moved it, else the usual one.
pub fn owners_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("OPENCODE_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(std::env::var_os("USERPROFILE")?).join(".config"),
    };
    Some(base.join("opencode"))
}

/// Make `lean` the run's config folder: the laws, the owner's config files from
/// `owner`, and junctions to the owner's folders. Writes only what changed.
pub fn prepare(lean: &Path, owner: &Path, laws: &str) -> io::Result<()> {
    std::fs::create_dir_all(lean)?;
    super::write_file(&lean.join("AGENTS.md"), &agents_md(laws))?;
    let Ok(entries) = std::fs::read_dir(owner) else { return Ok(()) };
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(n) = name.to_str() else { continue };
        let kind = e.file_type()?;
        let to = lean.join(n);
        if kind.is_dir() || (kind.is_symlink() && e.path().is_dir()) {
            if std::fs::symlink_metadata(&to).is_err() {
                junction(&to, &e.path())?;
            }
        } else if kind.is_file() && !skipped(n) {
            let text = std::fs::read(e.path())?;
            if std::fs::read(&to).ok().as_deref() != Some(&text[..]) {
                std::fs::write(&to, text)?;
            }
        }
    }
    Ok(())
}

/// A directory junction `link` to `target`: no admin rights needed, unlike a symlink.
#[cfg(windows)]
fn junction(link: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).creation_flags(CREATE_NO_WINDOW).output()?;
    if out.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!("mklink /J {}: {}", link.display(), String::from_utf8_lossy(&out.stderr).trim())))
}

#[cfg(not(windows))]
fn junction(link: &Path, target: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}
