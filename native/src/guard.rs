//! ByteHive's guards for the Claude agents it starts (guard/guard.mjs): hooks
//! given per run through `--settings`, so the owner's own Claude settings are
//! never touched. Refuses commands that destroy work, and keeps a turn that
//! changed code from ending before a check ran. Needs Node; without it the
//! agent starts as before.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const SCRIPT: &str = include_str!("guard/guard.mjs");

/// Whether `node` is on PATH, looked up once without starting a process.
fn has_node() -> bool {
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| {
        std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("node.exe").is_file() || d.join("node").is_file()))
    })
}

/// The settings JSON that wires the script in.
fn settings_json(script: &Path) -> String {
    let path = script.to_string_lossy().replace('\\', "/");
    let cmd = |kind: &str| format!("node \"{path}\" {kind}");
    serde_json::json!({
        "hooks": {
            "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": cmd("pre"), "timeout": 10 }] }],
            "Stop": [{ "hooks": [{ "type": "command", "command": cmd("stop"), "timeout": 15 }] }],
        }
    })
    .to_string()
}

/// Write `text` unless the file already holds it.
fn keep(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|t| t == text) {
        return Ok(());
    }
    std::fs::write(path, text)
}

/// The script and its settings under `dir`, written when they changed; the settings path.
fn install(dir: &Path) -> std::io::Result<PathBuf> {
    let hooks = dir.join("hooks");
    std::fs::create_dir_all(&hooks)?;
    let script = hooks.join("guard.mjs");
    keep(&script, SCRIPT)?;
    let settings = hooks.join("guard-settings.json");
    keep(&settings, &settings_json(&script))?;
    Ok(settings)
}

/// `--settings <file>` for a Claude agent, or nothing when the guards can't run here.
pub fn claude_args() -> Vec<String> {
    // A test run never writes into the owner's data folder; install() is tested on its own.
    if cfg!(test) {
        return Vec::new();
    }
    static SETTINGS: OnceLock<Option<PathBuf>> = OnceLock::new();
    let settings = SETTINGS.get_or_init(|| {
        if !has_node() {
            return None;
        }
        let dir = crate::fleet::data_dir()?;
        install(&dir).map_err(|e| eprintln!("[bytehive] the agent guards were not written: {e}")).ok()
    });
    match settings {
        Some(p) => vec!["--settings".into(), p.to_string_lossy().into_owned()],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_script_and_settings_that_point_at_it() {
        let dir = tempfile::tempdir().unwrap();
        let settings = install(dir.path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        let pre = v["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap();
        assert!(pre.starts_with("node \"") && pre.ends_with("guard.mjs\" pre") && !pre.contains('\\'), "{pre}");
        assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "Bash");
        assert!(v["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap().ends_with(" stop"));
        assert_eq!(std::fs::read_to_string(dir.path().join("hooks/guard.mjs")).unwrap(), SCRIPT);
    }

    #[test]
    fn leaves_files_alone_that_already_say_the_same() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path()).unwrap();
        let script = dir.path().join("hooks/guard.mjs");
        let before = std::fs::metadata(&script).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        install(dir.path()).unwrap();
        assert_eq!(std::fs::metadata(&script).unwrap().modified().unwrap(), before);
    }

    /// The script's own tests (guard.test.mjs), when Node is here to run them.
    #[test]
    fn the_guard_script_passes_its_tests() {
        if !has_node() {
            return;
        }
        let test = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/guard/guard.test.mjs");
        let out = std::process::Command::new("node").arg("--test").arg(&test).output().unwrap();
        assert!(out.status.success(), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
}
