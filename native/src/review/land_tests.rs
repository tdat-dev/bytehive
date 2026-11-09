//! land.rs against temporary repos.

use super::super::git::{git, TempDir};
use super::{branch_diff, branch_diff_impl, commit_message, discard_branch, land_expecting, land_impl, LandResult};
use std::path::{Path, PathBuf};

fn g(args: &[&str], dir: &Path) -> String {
    git(args, dir.to_str().unwrap()).unwrap_or_else(|e| panic!("git {args:?}: {e}"))
}

/// A repo with one commit on its default branch, plus an agent worktree on
/// branch `agent`. Returns (main, worktree).
fn setup(tmp: &Path) -> (PathBuf, PathBuf) {
    let main = tmp.join("main");
    std::fs::create_dir(&main).unwrap();
    g(&["init", "-q"], &main);
    g(&["config", "user.email", "t@t.dev"], &main);
    g(&["config", "user.name", "t"], &main);
    g(&["config", "core.autocrlf", "false"], &main);
    std::fs::write(main.join("a.txt"), "one\n").unwrap();
    std::fs::write(main.join("b.txt"), "bee\n").unwrap();
    g(&["add", "-A"], &main);
    g(&["commit", "-qm", "init"], &main);
    let wt = tmp.join("wt");
    g(&["worktree", "add", "-q", "-b", "agent", wt.to_str().unwrap(), "HEAD"], &main);
    (main, wt)
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn checks_running_at_once_do_not_trip_over_each_other() {
    let tmp = TempDir::new();
    let (_main, wt) = setup(tmp.path());
    std::fs::write(wt.join("a.txt"), "one\ntwo\n").unwrap(); // dirty: each check writes a snapshot
    let w = s(&wt);
    let runs: Vec<_> = (0..6)
        .map(|_| {
            let w = w.clone();
            std::thread::spawn(move || land_impl(&w, None, None, true))
        })
        .collect();
    for r in runs {
        let r = r.join().unwrap();
        assert!(matches!(r, LandResult::Ok { .. }), "every check should pass, got {r:?}");
    }
}

#[test]
fn a_base_that_changed_since_the_check_is_refused() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("a.txt"), "one\ntwo\n").unwrap();
    let LandResult::Ok { base, .. } = land_impl(&s(&wt), None, None, true) else { panic!("dry run should be clean") };
    g(&["checkout", "-q", "-b", "other"], &main); // someone switched branches
    let r = land_expecting(&s(&wt), None, None, false, Some(&base));
    let LandResult::Error { message } = r else { panic!("expected a refusal, got {r:?}") };
    assert!(message.contains("switched"), "{message}");
    assert!(wt.exists(), "nothing landed, the worktree stays");
    assert_eq!(std::fs::read_to_string(main.join("a.txt")).unwrap(), "one\n");
}

#[test]
fn clean_merge_commits_merges_and_removes_the_worktree() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    // Uncommitted work, a new file among it: landing commits it first.
    std::fs::write(wt.join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(wt.join("new.txt"), "fresh\n").unwrap();

    let r = super::land(&s(&wt), Some("Add the second line"), false, None);
    let LandResult::Ok { base, branch, removed, note } = r else { panic!("expected ok, got {r:?}") };
    assert_eq!(branch, "agent");
    assert!(!base.is_empty());
    assert!(removed, "worktree should be removed: {note:?}");
    assert!(!wt.exists(), "worktree folder should be gone");
    assert_eq!(std::fs::read_to_string(main.join("a.txt")).unwrap(), "one\ntwo\n");
    assert!(main.join("new.txt").exists());
    // --no-ff merge commit, and the agent's commit carries the task title
    let parents = g(&["rev-list", "--parents", "-n", "1", "HEAD"], &main);
    assert_eq!(parents.split_whitespace().count(), 3);
    assert_eq!(g(&["log", "-1", "--format=%s", "HEAD^2"], &main), "Add the second line");
    assert_eq!(g(&["log", "-1", "--format=%s", "HEAD"], &main), "Merge agent: Add the second line");
    // the branch is gone, the project folder is clean
    assert!(g(&["branch", "--list", "agent"], &main).is_empty());
    assert_eq!(g(&["status", "--porcelain"], &main), "");
}

#[test]
fn a_given_message_wins_over_the_title() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("c.txt"), "see\n").unwrap();
    let r = land_impl(&s(&wt), Some("Add c"), Some("Task title"), false);
    assert!(matches!(r, LandResult::Ok { .. }), "{r:?}");
    assert_eq!(g(&["log", "-1", "--format=%s", "HEAD^2"], &main), "Add c");
    assert_eq!(g(&["log", "-1", "--format=%s", "HEAD"], &main), "Merge agent: Task title");
}

#[test]
fn without_a_title_the_merge_says_bytehive() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("c.txt"), "see\n").unwrap();
    assert!(matches!(super::land(&s(&wt), None, false, None), LandResult::Ok { removed: true, .. }));
    assert_eq!(g(&["log", "-1", "--format=%s", "HEAD^2"], &main), "Work from agent");
    assert_eq!(g(&["log", "-1", "--format=%s", "HEAD"], &main), "Merge agent (bytehive)");
}

/// The same line changed on both sides.
fn conflicting(tmp: &Path) -> (PathBuf, PathBuf) {
    let (main, wt) = setup(tmp);
    std::fs::write(wt.join("a.txt"), "agent's line\n").unwrap();
    g(&["commit", "-qam", "agent edit"], &wt);
    std::fs::write(main.join("a.txt"), "main's line\n").unwrap();
    g(&["commit", "-qam", "main edit"], &main);
    (main, wt)
}

#[test]
fn conflict_is_aborted_and_lists_the_files() {
    let tmp = TempDir::new();
    let (main, wt) = conflicting(tmp.path());
    let head = g(&["rev-parse", "HEAD"], &main);

    let r = land_impl(&s(&wt), None, None, false);
    let base = g(&["rev-parse", "--abbrev-ref", "HEAD"], &main);
    assert_eq!(r, LandResult::Conflict { base, branch: "agent".into(), files: vec!["a.txt".into()] });
    // nothing merged, nothing left half-done, the worktree stays
    assert_eq!(g(&["rev-parse", "HEAD"], &main), head);
    assert_eq!(g(&["status", "--porcelain"], &main), "");
    assert!(!main.join(".git").join("MERGE_HEAD").exists());
    assert!(wt.exists());
    assert_eq!(std::fs::read_to_string(main.join("a.txt")).unwrap(), "main's line\n");
}

#[test]
fn dry_run_sees_the_conflict_without_touching_anything() {
    let tmp = TempDir::new();
    let (main, wt) = conflicting(tmp.path());
    // uncommitted work in the worktree stays uncommitted
    std::fs::write(wt.join("b.txt"), "bee two\n").unwrap();
    let head = g(&["rev-parse", "HEAD"], &main);
    let agent = g(&["rev-parse", "agent"], &main);

    let r = land_impl(&s(&wt), None, None, true);
    assert!(matches!(&r, LandResult::Conflict { files, .. } if files == &vec!["a.txt".to_string()]), "{r:?}");
    assert_eq!(g(&["rev-parse", "HEAD"], &main), head);
    assert_eq!(g(&["rev-parse", "agent"], &main), agent);
    assert_eq!(g(&["status", "--porcelain"], &wt), "M b.txt");
}

#[test]
fn dry_run_says_a_clean_merge_is_clean_and_changes_nothing() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("new.txt"), "fresh\n").unwrap();
    let r = super::land(&s(&wt), None, true, None);
    assert!(matches!(&r, LandResult::Ok { removed: false, note: None, .. }), "{r:?}");
    assert_eq!(g(&["status", "--porcelain"], &wt), "?? new.txt");
    assert!(!main.join("new.txt").exists());
    assert!(wt.exists());
}

#[test]
fn refuses_when_the_project_folder_has_edits_the_merge_touches() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("a.txt"), "agent\n").unwrap();
    std::fs::write(main.join("a.txt"), "my own edit\n").unwrap();
    let head = g(&["rev-parse", "HEAD"], &main);

    for dry in [true, false] {
        let r = land_impl(&s(&wt), None, None, dry);
        let LandResult::Error { message } = r else { panic!("expected a refusal, got {r:?}") };
        assert!(message.contains("uncommitted changes to a.txt"), "{message}");
    }
    // your edit is untouched and nothing was merged
    assert_eq!(std::fs::read_to_string(main.join("a.txt")).unwrap(), "my own edit\n");
    assert_eq!(g(&["rev-parse", "HEAD"], &main), head);
}

#[test]
fn edits_the_merge_does_not_touch_are_left_alone() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("a.txt"), "agent\n").unwrap();
    std::fs::write(main.join("b.txt"), "my own edit\n").unwrap();
    let r = land_impl(&s(&wt), None, None, false);
    assert!(matches!(r, LandResult::Ok { removed: true, .. }), "{r:?}");
    assert_eq!(std::fs::read_to_string(main.join("a.txt")).unwrap(), "agent\n");
    assert_eq!(std::fs::read_to_string(main.join("b.txt")).unwrap(), "my own edit\n");
}

#[test]
fn nothing_to_merge_is_an_error_not_a_merge() {
    let tmp = TempDir::new();
    let (_main, wt) = setup(tmp.path());
    for dry in [true, false] {
        let r = land_impl(&s(&wt), None, None, dry);
        assert!(matches!(&r, LandResult::Error { message } if message.starts_with("Nothing to merge")), "{r:?}");
    }
    assert!(wt.exists());
}

#[test]
fn the_project_folder_itself_has_nothing_to_merge() {
    let tmp = TempDir::new();
    let (main, _wt) = setup(tmp.path());
    let r = land_impl(&s(&main), None, None, false);
    assert!(matches!(&r, LandResult::Error { message } if message.contains("project folder itself")), "{r:?}");
}

#[test]
fn discard_removes_the_worktree_and_its_branch_work_and_all() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("a.txt"), "committed\n").unwrap();
    g(&["commit", "-qam", "agent edit"], &wt);
    std::fs::write(wt.join("junk.txt"), "uncommitted\n").unwrap();
    discard_branch(&s(&wt)).expect("discard");
    assert!(!wt.exists());
    assert!(g(&["branch", "--list", "agent"], &main).is_empty());
    assert_eq!(std::fs::read_to_string(main.join("a.txt")).unwrap(), "one\n");
}

#[test]
fn branch_diff_has_commits_and_uncommitted_work_but_not_the_base_s_own() {
    let tmp = TempDir::new();
    let (main, wt) = setup(tmp.path());
    std::fs::write(wt.join("a.txt"), "one\ncommitted\n").unwrap();
    g(&["commit", "-qam", "agent edit"], &wt);
    std::fs::write(wt.join("b.txt"), "bee\nuncommitted\n").unwrap();
    std::fs::write(wt.join("new.txt"), "fresh\n").unwrap();
    std::fs::write(main.join("base.txt"), "the base moved on\n").unwrap();
    g(&["add", "-A"], &main);
    g(&["commit", "-qm", "base moves"], &main);
    let d = branch_diff(&s(&wt)).unwrap();
    assert!(d.contains("+committed") && d.contains("+uncommitted") && d.contains("+fresh"), "{d}");
    assert!(!d.contains("base.txt"), "{d}");
    // reading it changed nothing
    assert_eq!(g(&["status", "--porcelain"], &wt), "M b.txt\n?? new.txt");
    assert!(branch_diff_impl(&s(&main)).is_err());
    assert!(branch_diff(&s(&main)).is_none());
}

#[test]
fn commit_message_comes_from_the_title() {
    assert_eq!(commit_message(Some("  mine "), Some("t"), "b"), "mine");
    assert_eq!(commit_message(None, Some("Fix the flaky test\nmore"), "b"), "Fix the flaky test");
    assert_eq!(commit_message(Some(" "), None, "bytehive/ana-1"), "Work from bytehive/ana-1");
    let long = "x".repeat(100);
    assert_eq!(commit_message(None, Some(&long), "b").chars().count(), 73);
}
