//! Claude opens on "do you trust this folder?" the first time it runs somewhere,
//! skip-permissions or not, with "No, exit" selected. A member of the company has
//! nobody standing by its pane, so it would wait there for good, and a stray Enter
//! would close it. The owner said yes when they approved the hire: this writes
//! that answer down the way Claude does.
//!
//! Claude keeps it under `projects[<folder>].hasTrustDialogAccepted` in
//! `.claude.json`. The folder has forward slashes and, in a git repo, is the main
//! checkout, which all of its worktrees share. Running Claudes rewrite that file
//! all the time, so this takes the lock they take (a `.lock` folder beside it) and
//! swaps the file in whole.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};

use crate::error::{run_blocking, CommandError};
use crate::worktree::git;

/// How long to wait for a Claude that is saving.
const LOCK_WAIT: Duration = Duration::from_secs(5);
/// A lock older than this was left by a Claude that died (Claude's own limit).
const LOCK_STALE: Duration = Duration::from_secs(10);
const LOCK_POLL: Duration = Duration::from_millis(50);
/// Tries at the swap: Windows refuses it while anyone has the file open.
const SWAP_TRIES: u32 = 10;

fn failed(what: &str, e: impl std::fmt::Display) -> CommandError {
    CommandError::Failed(format!("{what}: {e}"))
}

/// Where Claude keeps its config, like Claude finds it.
fn config_file() -> Option<PathBuf> {
    let dir = std::env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .or_else(|| std::env::var("USERPROFILE").ok())
        .or_else(|| std::env::var("HOME").ok())?;
    Some(Path::new(&dir).join(".claude.json"))
}

/// The folder as Claude writes it. `common` is what git calls the repo's shared
/// `.git` folder: its parent is the main checkout, whichever worktree `dir` is.
fn project_key(dir: &str, common: Option<&str>) -> String {
    let main = common.and_then(|c| c.trim_end_matches('/').strip_suffix("/.git"));
    main.unwrap_or(dir).replace('\\', "/").trim_end_matches('/').to_string()
}

/// The config with this folder trusted, or None when it already is.
fn with_trust(text: &str, key: &str) -> Result<Option<String>, CommandError> {
    let mut config: Value = serde_json::from_str(text).map_err(|e| failed("Claude's config cannot be read", e))?;
    let root = config.as_object_mut().ok_or_else(|| failed("Claude's config", "not an object"))?;
    let projects = root.entry("projects").or_insert_with(|| json!({}));
    let project = projects
        .as_object_mut()
        .ok_or_else(|| failed("Claude's config", "projects is not an object"))?
        .entry(key)
        .or_insert_with(|| json!({}));
    if project["hasTrustDialogAccepted"] == json!(true) {
        return Ok(None);
    }
    project
        .as_object_mut()
        .ok_or_else(|| failed("Claude's config", "a project is not an object"))?
        .insert("hasTrustDialogAccepted".into(), json!(true));
    serde_json::to_string_pretty(&config).map(Some).map_err(|e| failed("Claude's config", e))
}

/// Claude's config lock, held until dropped.
struct ConfigLock(PathBuf);

impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn stale(lock: &Path, limit: Duration) -> bool {
    fs::metadata(lock)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age > limit)
}

/// Whether a refused lock folder means somebody has it. Windows says
/// PermissionDenied while a folder is on its way out, which is a Claude letting go.
fn lock_contended(kind: ErrorKind) -> bool {
    kind == ErrorKind::AlreadyExists || (cfg!(windows) && kind == ErrorKind::PermissionDenied)
}

fn take_lock(lock: PathBuf, wait: Duration, limit: Duration) -> Result<ConfigLock, CommandError> {
    let until = Instant::now() + wait;
    loop {
        match fs::create_dir(&lock) {
            Ok(()) => return Ok(ConfigLock(lock)),
            Err(e) if !lock_contended(e.kind()) => return Err(failed("Claude's config lock", e)),
            Err(_) if stale(&lock, limit) && fs::remove_dir(&lock).is_ok() => {}
            Err(_) if Instant::now() >= until => return Err(failed("Claude's config", "another Claude is holding it")),
            Err(_) => sleep(LOCK_POLL),
        }
    }
}

/// All of the text, on the disk: a crash right after the swap must not leave an
/// empty config behind the name of the real one.
fn write_temp(tmp: &Path, text: &str) -> std::io::Result<()> {
    let mut f = fs::File::create(tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()
}

fn swap_in(file: &Path, text: &str) -> Result<(), CommandError> {
    swap_with(file, text, write_temp)
}

fn swap_with(file: &Path, text: &str, write: impl Fn(&Path, &str) -> std::io::Result<()>) -> Result<(), CommandError> {
    let tmp = file.with_extension(format!("json.bytehive.{}", std::process::id()));
    if let Err(e) = write(&tmp, text) {
        let _ = fs::remove_file(&tmp);
        return Err(failed("Claude's config cannot be written", e));
    }
    for tries in 1.. {
        match fs::rename(&tmp, file) {
            Ok(()) => return Ok(()),
            Err(_) if tries < SWAP_TRIES => sleep(LOCK_POLL),
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(failed("Claude's config cannot be written", e));
            }
        }
    }
    unreachable!()
}

/// Trust `key` in the config at `file`. A config that isn't there is left alone:
/// a Claude that has never run starts with its own setup anyway.
fn trust_in(file: &Path, key: &str, wait: Duration, limit: Duration) -> Result<(), CommandError> {
    if !file.exists() {
        return Ok(());
    }
    // Most starts find the folder trusted already and never touch a file Claudes are
    // busy with. A read that fails or looks torn is settled under the lock.
    if fs::read_to_string(file).is_ok_and(|text| matches!(with_trust(&text, key), Ok(None))) {
        return Ok(());
    }
    let mut lock = file.as_os_str().to_owned();
    lock.push(".lock");
    let _held = take_lock(PathBuf::from(lock), wait, limit)?;
    let text = fs::read_to_string(file).map_err(|e| failed("Claude's config cannot be read", e))?;
    match with_trust(&text, key)? {
        Some(next) => swap_in(file, &next),
        None => Ok(()),
    }
}

fn claude_trust_folder_impl(dir: &str) -> Result<(), CommandError> {
    let Some(file) = config_file() else { return Ok(()) };
    let common = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"], dir).ok();
    trust_in(&file, &project_key(dir, common.as_deref()), LOCK_WAIT, LOCK_STALE)
}

/// Tell Claude that `dir` is trusted, before an agent starts there.
#[tauri::command]
pub async fn claude_trust_folder(dir: String) -> Result<(), CommandError> {
    run_blocking(move || claude_trust_folder_impl(&dir)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Duration = Duration::ZERO;
    const LONG: Duration = Duration::from_secs(600);

    #[test]
    fn a_worktree_counts_as_its_main_checkout() {
        let key = project_key("D:\\.bytehive-worktrees\\shop-x\\cid", Some("D:/work/shop/.git"));
        assert_eq!(key, "D:/work/shop");
    }

    #[test]
    fn a_folder_outside_git_is_its_own_key() {
        assert_eq!(project_key("C:\\Users\\me\\.bytehive\\company\\office\\", None), "C:/Users/me/.bytehive/company/office");
    }

    #[test]
    fn a_submodule_keeps_its_own_folder() {
        assert_eq!(project_key("D:\\work\\shop\\sub", Some("D:/work/shop/.git/modules/sub")), "D:/work/shop/sub");
    }

    #[test]
    fn trust_keeps_everything_else_in_the_config() {
        let text = r#"{"oauthAccount":{"id":1},"projects":{"D:/a":{"allowedTools":["x"]},"D:/b":{"hasTrustDialogAccepted":false}}}"#;
        let out: Value = serde_json::from_str(&with_trust(text, "D:/b").unwrap().unwrap()).unwrap();
        assert_eq!(out["oauthAccount"]["id"], 1);
        assert_eq!(out["projects"]["D:/a"]["allowedTools"][0], "x");
        assert_eq!(out["projects"]["D:/b"]["hasTrustDialogAccepted"], true);
    }

    #[test]
    fn a_new_folder_gets_an_entry() {
        let out: Value = serde_json::from_str(&with_trust("{}", "D:/a").unwrap().unwrap()).unwrap();
        assert_eq!(out["projects"]["D:/a"]["hasTrustDialogAccepted"], true);
    }

    #[test]
    fn a_trusted_folder_writes_nothing() {
        assert_eq!(with_trust(r#"{"projects":{"D:/a":{"hasTrustDialogAccepted":true}}}"#, "D:/a").unwrap(), None);
    }

    #[test]
    fn a_config_it_cannot_read_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, "{ half written").unwrap();
        assert!(trust_in(&file, "D:/a", NOW, LONG).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{ half written");
    }

    #[test]
    fn no_config_stays_no_config() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        trust_in(&file, "D:/a", NOW, LONG).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn writes_the_file_and_gives_the_lock_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, "{}").unwrap();
        trust_in(&file, "D:/a", NOW, LONG).unwrap();
        let out: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(out["projects"]["D:/a"]["hasTrustDialogAccepted"], true);
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [".claude.json"]);
    }

    #[test]
    fn a_held_lock_is_waited_for_and_never_broken() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, "{}").unwrap();
        fs::create_dir(dir.path().join(".claude.json.lock")).unwrap();
        assert!(trust_in(&file, "D:/a", NOW, LONG).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
        assert!(dir.path().join(".claude.json.lock").exists());
    }

    #[test]
    fn a_write_that_fails_leaves_the_config_and_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, "{}").unwrap();
        let half_written = |tmp: &Path, _: &str| -> std::io::Result<()> {
            fs::write(tmp, "{ half")?;
            Err(std::io::Error::other("disk full"))
        };
        assert!(swap_with(&file, "{\"a\":1}", half_written).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [".claude.json"]);
    }

    #[test]
    fn the_temp_file_holds_all_of_the_text() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("t");
        write_temp(&tmp, "{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&tmp).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn a_trusted_folder_needs_no_lock() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, r#"{"projects":{"D:/a":{"hasTrustDialogAccepted":true}}}"#).unwrap();
        fs::create_dir(dir.path().join(".claude.json.lock")).unwrap();
        trust_in(&file, "D:/a", NOW, LONG).unwrap();
        assert!(dir.path().join(".claude.json.lock").exists());
    }

    #[test]
    fn windows_refusing_the_lock_folder_counts_as_held() {
        assert!(lock_contended(ErrorKind::AlreadyExists));
        assert_eq!(lock_contended(ErrorKind::PermissionDenied), cfg!(windows));
        assert!(!lock_contended(ErrorKind::NotFound));
    }

    #[test]
    fn a_stale_lock_that_will_not_go_is_given_up_on() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, "{}").unwrap();
        let lock = dir.path().join(".claude.json.lock");
        fs::create_dir(&lock).unwrap();
        fs::write(lock.join("in-use"), "").unwrap(); // remove_dir refuses a folder with something in it
        sleep(Duration::from_millis(30));
        assert!(trust_in(&file, "D:/a", NOW, Duration::from_millis(1)).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
    }

    #[test]
    fn a_lock_left_by_a_dead_claude_is_taken() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude.json");
        fs::write(&file, "{}").unwrap();
        fs::create_dir(dir.path().join(".claude.json.lock")).unwrap();
        sleep(Duration::from_millis(30));
        trust_in(&file, "D:/a", NOW, Duration::from_millis(1)).unwrap();
        assert!(fs::read_to_string(&file).unwrap().contains("hasTrustDialogAccepted"));
    }
}
