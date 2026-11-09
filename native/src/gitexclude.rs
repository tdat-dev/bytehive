//! A project's `.bytehive/` folder is ByteHive's, not the project's, so it must
//! not show up in `git status`. The repo's own `info/exclude` keeps it out without
//! touching anything tracked: a `.gitignore` stays the owner's to edit
//! (src-tauri/src/gitexclude.rs). The bridge calls it once per project.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use crate::worktree::git;

const LINE: &str = ".bytehive/";

fn failed(e: impl std::fmt::Display) -> String {
    format!("git's exclude file: {e}")
}

/// What to add to an exclude file holding `text`, or None when it has the line.
fn addition(text: &str) -> Option<String> {
    if text.lines().any(|l| l.trim() == LINE) {
        return None;
    }
    let gap = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
    Some(format!("{gap}{LINE}\n"))
}

/// Keep `.bytehive/` out of `git status` in the repo at `dir`, if it is one.
/// Blocking (a few git calls): run it off the UI thread.
pub fn keep_out(dir: &str) -> Result<(), String> {
    // Not a repo, no git, or a folder that has gone: there is no status to keep clean.
    // Asked of git because a worktree's exclude file is its main checkout's.
    let Ok(rel) = git(&["rev-parse", "--git-path", "info/exclude"], dir) else { return Ok(()) };
    if git(&["check-ignore", "-q", ".bytehive/"], dir).is_ok() {
        return Ok(());
    }
    // A team that commits the folder wants new files in it to show up.
    if git(&["ls-files", "--", ".bytehive"], dir).is_ok_and(|tracked| !tracked.is_empty()) {
        return Ok(());
    }
    let file = Path::new(dir).join(rel);
    let text = match fs::read(&file) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(failed(e)),
    };
    let Some(add) = addition(&text) else { return Ok(()) };
    if let Some(info) = file.parent() {
        fs::create_dir_all(info).map_err(failed)?;
    }
    // Added to the end, so a write cut short costs none of what the owner has there.
    let mut out = fs::OpenOptions::new().create(true).append(true).open(&file).map_err(failed)?;
    out.write_all(add.as_bytes()).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn run(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// A repo with one commit that the machine's own ignore rules can't reach.
    fn repo(tmp: &Path) -> PathBuf {
        let dir = tmp.join("proj");
        fs::create_dir(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        run(&dir, &["config", "user.email", "a@b.c"]);
        run(&dir, &["config", "user.name", "a"]);
        run(&dir, &["config", "core.excludesFile", tmp.join("no-such-file").to_str().unwrap()]);
        fs::write(dir.join("a.txt"), "a").unwrap();
        run(&dir, &["add", "."]);
        run(&dir, &["commit", "-qm", "a"]);
        dir
    }

    fn exclude_of(dir: &Path) -> String {
        fs::read_to_string(dir.join(".git/info/exclude")).unwrap_or_default()
    }

    fn count(text: &str) -> usize {
        text.lines().filter(|l| *l == LINE).count()
    }

    fn make_bytehive(dir: &Path) {
        fs::create_dir_all(dir.join(".bytehive")).unwrap();
        fs::write(dir.join(".bytehive/board.json"), "{}").unwrap();
    }

    #[test]
    fn a_folder_it_makes_stays_out_of_git_status() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        make_bytehive(&dir);
        assert!(!run(&dir, &["status", "--short"]).is_empty());
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(run(&dir, &["status", "--short"]), "");
    }

    #[test]
    fn it_is_written_once() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        keep_out(dir.to_str().unwrap()).unwrap();
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(count(&exclude_of(&dir)), 1);
    }

    #[test]
    fn what_the_owner_has_there_stays() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        fs::write(dir.join(".git/info/exclude"), "secrets.txt").unwrap();
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(exclude_of(&dir), "secrets.txt\n.bytehive/\n");
    }

    #[test]
    fn a_missing_info_folder_is_made() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        fs::remove_dir_all(dir.join(".git/info")).unwrap();
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(count(&exclude_of(&dir)), 1);
    }

    #[test]
    fn a_gitignore_that_covers_it_is_enough_and_is_not_touched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        fs::write(dir.join(".gitignore"), ".bytehive/\n").unwrap();
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(count(&exclude_of(&dir)), 0);
        assert_eq!(fs::read_to_string(dir.join(".gitignore")).unwrap(), ".bytehive/\n");
    }

    #[test]
    fn a_folder_the_team_commits_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        make_bytehive(&dir);
        run(&dir, &["add", "."]);
        run(&dir, &["commit", "-qm", "board"]);
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(count(&exclude_of(&dir)), 0);
        fs::write(dir.join(".bytehive/new.json"), "{}").unwrap();
        assert!(run(&dir, &["status", "--short"]).contains("new.json"));
    }

    #[test]
    fn a_tracked_gitignore_is_never_edited() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        fs::write(dir.join(".gitignore"), "node_modules/\n").unwrap();
        keep_out(dir.to_str().unwrap()).unwrap();
        assert_eq!(fs::read_to_string(dir.join(".gitignore")).unwrap(), "node_modules/\n");
        assert_eq!(count(&exclude_of(&dir)), 1);
    }

    #[test]
    fn a_project_that_is_a_worktree_uses_its_main_checkouts_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        let wt = tmp.path().join("wt");
        run(&dir, &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()]);
        make_bytehive(&wt);
        keep_out(wt.to_str().unwrap()).unwrap();
        assert_eq!(count(&exclude_of(&dir)), 1);
        assert_eq!(run(&wt, &["status", "--short"]), "");
    }

    #[test]
    fn a_subfolder_of_a_repo_is_covered_by_the_same_line() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = repo(tmp.path());
        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        make_bytehive(&sub);
        keep_out(sub.to_str().unwrap()).unwrap();
        assert_eq!(count(&exclude_of(&dir)), 1);
        assert_eq!(run(&dir, &["status", "--short"]), "");
    }

    #[test]
    fn outside_a_repo_nothing_happens() {
        let tmp = tempfile::tempdir().unwrap();
        keep_out(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_folder_that_has_gone_is_no_failure() {
        let tmp = tempfile::tempdir().unwrap();
        keep_out(tmp.path().join("gone").to_str().unwrap()).unwrap();
    }

    #[test]
    fn a_crlf_exclude_file_counts_too() {
        assert_eq!(addition("x\r\n.bytehive/\r\n"), None);
    }
}
