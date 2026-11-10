//! Landing an agent's work (land.rs in today's app): one click takes the
//! branch of an agent's own worktree into the branch the project folder has
//! checked out, then cleans the worktree and the branch away. Or throws the
//! branch away (Discard).
//!
//! `land` does the whole job: commit what is uncommitted in the worktree,
//! refuse when the project folder has uncommitted edits the merge would
//! touch, merge (`--no-ff`), and on success remove the worktree and delete its
//! branch. A conflict is aborted, so the project folder is left exactly as it
//! was, and the conflicting files come back so the agent can be sent back to
//! resolve them. A dry run answers "would this merge cleanly?" without
//! touching anything: the worktree's state is snapshotted into a throwaway
//! commit (through a private index file) and put through `git merge-tree`,
//! which merges in memory.

use std::path::{Path, PathBuf};

use super::git::{dirty_paths, git, git_env, run, same_path, ScratchIndex};

pub use super::git::{repo_diff, repos_under};

/// The outcome of landing a branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LandResult {
    /// Merged (or, on a dry run, would merge cleanly). `removed` says the
    /// worktree and branch are gone; `note` says why they were kept when not.
    Ok { base: String, branch: String, removed: bool, note: Option<String> },
    /// The merge would conflict in these files. Nothing was merged: the merge
    /// was aborted and the project folder is as it was.
    Conflict { base: String, branch: String, files: Vec<String> },
    /// Refused or failed, with a message fit to show as it is.
    Error { message: String },
}

fn err(message: impl Into<String>) -> LandResult {
    LandResult::Error { message: message.into() }
}

/// The agent's side and the project folder's side of a landing.
struct Sides {
    /// The agent's worktree, as given.
    worktree: String,
    /// Its branch.
    branch: String,
    /// The main checkout, where the merge runs.
    main: String,
    /// The branch checked out there, which the merge goes into.
    base: String,
}

/// Work out both sides, or say why this worktree can't be landed.
fn sides(worktree: &str) -> Result<Sides, String> {
    if !Path::new(worktree).is_dir() {
        return Err(format!("The agent's folder is gone: {worktree}"));
    }
    let inside = git(&["rev-parse", "--is-inside-work-tree"], worktree).map(|s| s == "true").unwrap_or(false);
    if !inside {
        return Err(format!("Not a git work tree: {worktree}"));
    }
    let git_dir = git(&["rev-parse", "--path-format=absolute", "--git-dir"], worktree)?;
    let common = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"], worktree)?;
    if same_path(&git_dir, &common) {
        return Err("This agent works in the project folder itself, not on a branch of its own, so there is nothing to merge.".into());
    }
    let main = Path::new(&common)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| "Can't find the project folder of this worktree.".to_string())?;
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"], worktree)?;
    if branch == "HEAD" {
        return Err("The agent's worktree is not on a branch (detached HEAD).".into());
    }
    let base = git(&["rev-parse", "--abbrev-ref", "HEAD"], &main)?;
    if base == "HEAD" {
        return Err("The project folder is not on a branch (detached HEAD), so there is nothing to merge into.".into());
    }
    if base == branch {
        return Err(format!("The project folder has {branch} checked out itself."));
    }
    Ok(Sides { worktree: worktree.to_string(), branch, main, base })
}

/// The commit that stands for everything in the worktree right now. HEAD when
/// it is clean; otherwise a commit written through a private copy of the
/// index, so neither the branch, the index nor the files change.
fn snapshot(s: &Sides) -> Result<String, String> {
    let head = git(&["rev-parse", "HEAD"], &s.worktree)?;
    if dirty_paths(&s.worktree)?.is_empty() {
        return Ok(head);
    }
    // A copy of the real index keeps `add` to hashing what changed. Its own
    // name per call: the drawer checks while a merge runs, and a shared
    // scratch index made one of them fail on its .lock.
    let scratch = ScratchIndex::copy_of(&s.worktree);
    let index = scratch.path.to_string_lossy().into_owned();
    let env = [("GIT_INDEX_FILE", index.as_str())];
    // Checks at once write the same objects, and on Windows the second writer
    // of one is refused ("unable to write file ... Permission denied"): one
    // snapshot at a time here, and one more try for a writer outside this app.
    static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let write = || -> Result<String, String> {
        git_env(&["add", "-A", "--", "."], &s.worktree, &env)?;
        let tree = git_env(&["write-tree"], &s.worktree, &env)?;
        git(&["commit-tree", &tree, "-p", &head, "-m", "bytehive: merge check"], &s.worktree)
    };
    match write() {
        Err(e) if e.contains("unable to write") || e.contains("Permission denied") => {
            std::thread::sleep(std::time::Duration::from_millis(150));
            write()
        }
        got => got,
    }
}

/// Files the branch (as `tip`) changed since it left the base.
fn touched(s: &Sides, tip: &str) -> Result<Vec<String>, String> {
    let range = format!("{}...{}", s.base, tip);
    let out = git(&["-c", "core.quotepath=false", "diff", "--name-only", "-z", &range], &s.main)?;
    Ok(out.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect())
}

/// Refuse when the project folder has uncommitted edits in files the merge
/// would touch: git would refuse too, or mix them into the merge.
fn check_base_clean(s: &Sides, tip: &str) -> Result<(), String> {
    let dirty = dirty_paths(&s.main)?;
    if dirty.is_empty() {
        return Ok(());
    }
    let touched = touched(s, tip)?;
    let mut both: Vec<&String> = dirty.iter().filter(|p| touched.contains(p)).collect();
    both.dedup();
    if both.is_empty() {
        return Ok(());
    }
    let list = both.iter().take(6).map(|p| p.as_str()).collect::<Vec<_>>().join(", ");
    let more = if both.len() > 6 { format!(" and {} more", both.len() - 6) } else { String::new() };
    Err(format!(
        "{} has uncommitted changes to {list}{more}, which this merge also changes. Commit or stash them in the project folder, then merge again.",
        s.base
    ))
}

/// Would `tip` merge into the base cleanly? `Some(files)` when it conflicts;
/// None when it is clean, or when this git can't tell (no merge-tree).
fn conflicts_in_memory(s: &Sides, tip: &str) -> Option<Vec<String>> {
    let args = ["-c", "core.quotepath=false", "merge-tree", "--write-tree", "--name-only", "--no-messages", &s.base, tip];
    let o = run(&args, &s.main, &[]).ok()?;
    if o.code != 1 {
        return None; // 0: clean; anything else: this git can't say
    }
    // The first line is the merged tree; the conflicted files follow.
    let mut files: Vec<String> = o.stdout.lines().skip(1).take_while(|l| !l.is_empty()).map(str::to_string).collect();
    files.dedup();
    Some(files)
}

/// Whether a folder can be removed now. On Windows a folder that is some
/// process's current directory can't be deleted (nor renamed): wait a moment
/// for an agent that was just stopped to let go of it.
fn wait_released(dir: &str) -> bool {
    let p = Path::new(dir);
    if !p.exists() {
        return true;
    }
    let probe = PathBuf::from(format!("{}.bytehive-probe", dir.trim_end_matches(['\\', '/'])));
    for _ in 0..12 {
        if std::fs::rename(p, &probe).is_ok() {
            // Moved and couldn't move back: leave it be.
            return std::fs::rename(&probe, p).is_ok();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

/// Take the worktree away and delete its branch. `force` throws away
/// uncommitted work in it (Discard); a landing never needs that.
fn remove(s: &Sides, force: bool) -> Result<(), String> {
    if !wait_released(&s.worktree) {
        return Err("its folder is still in use (is the agent still running there?)".into());
    }
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&s.worktree);
    git(&args, &s.main)?;
    // Merged, `-d` is enough; discarded, the branch has work nobody kept.
    git(&["branch", if force { "-D" } else { "-d" }, &s.branch], &s.main)?;
    Ok(())
}

/// A commit message from what the agent was asked to do.
fn commit_message(message: Option<&str>, title: Option<&str>, branch: &str) -> String {
    if let Some(m) = message.map(str::trim).filter(|m| !m.is_empty()) {
        return m.to_string();
    }
    let t = title.map(|t| t.lines().next().unwrap_or("").trim()).unwrap_or("");
    if t.is_empty() {
        return format!("Work from {branch}");
    }
    let mut s: String = t.chars().take(72).collect();
    if t.chars().count() > 72 {
        s = s.trim_end().to_string();
        s.push('\u{2026}');
    }
    s
}

/// Merge an agent's worktree branch into the project folder's branch, then
/// remove the worktree and the branch. See the module docs. `expect_base` is
/// the branch a dry run showed you: someone may have switched branches there
/// in between, and the merge must not quietly go somewhere else.
pub fn land(worktree: &str, title: Option<&str>, dry_run: bool, expect_base: Option<&str>) -> LandResult {
    land_expecting(worktree, None, title, dry_run, expect_base)
}

fn land_expecting(worktree: &str, message: Option<&str>, title: Option<&str>, dry_run: bool, expect_base: Option<&str>) -> LandResult {
    if let Some(want) = expect_base.filter(|b| !b.is_empty()) {
        match sides(worktree) {
            Ok(s) if s.base != want => {
                return err(format!("The project folder switched from {want} to {} since the check. Look again, then merge.", s.base));
            }
            Err(e) => return err(e),
            Ok(_) => {}
        }
    }
    land_impl(worktree, message, title, dry_run)
}

fn land_impl(worktree: &str, message: Option<&str>, title: Option<&str>, dry_run: bool) -> LandResult {
    let s = match sides(worktree) {
        Ok(s) => s,
        Err(e) => return err(e),
    };
    if Path::new(&s.main).join(".git").join("MERGE_HEAD").exists() {
        return err(format!("The project folder is in the middle of another merge. Finish or abort it there, then merge {} again.", s.branch));
    }
    let conflict = |files: Vec<String>| LandResult::Conflict { base: s.base.clone(), branch: s.branch.clone(), files };
    let nothing = || err(format!("Nothing to merge: {} has nothing that {} doesn't already have.", s.branch, s.base));

    if dry_run {
        let tip = match snapshot(&s) {
            Ok(t) => t,
            Err(e) => return err(e),
        };
        if tip == git(&["rev-parse", &s.base], &s.main).unwrap_or_default() || ahead(&s, &tip) == 0 {
            return nothing();
        }
        if let Err(e) = check_base_clean(&s, &tip) {
            return err(e);
        }
        if let Some(files) = conflicts_in_memory(&s, &tip) {
            return conflict(files);
        }
        return LandResult::Ok { base: s.base.clone(), branch: s.branch.clone(), removed: false, note: None };
    }

    // 1. What the agent left uncommitted goes onto its branch.
    match dirty_paths(&s.worktree) {
        Ok(d) if !d.is_empty() => {
            let msg = commit_message(message, title, &s.branch);
            if let Err(e) = git(&["add", "-A", "--", "."], &s.worktree).and_then(|_| git(&["commit", "-q", "-m", &msg], &s.worktree)) {
                return err(format!("Couldn't commit the agent's changes: {e}"));
            }
        }
        Ok(_) => {}
        Err(e) => return err(e),
    }
    if ahead(&s, &s.branch) == 0 {
        return nothing();
    }
    // 2. Never mix the merge into your own uncommitted edits.
    if let Err(e) = check_base_clean(&s, &s.branch) {
        return err(e);
    }
    // 3. Merge. A conflict is aborted, leaving the project folder as it was.
    let msg = match title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => format!("Merge {}: {}", s.branch, commit_message(None, Some(t), &s.branch)),
        None => format!("Merge {} (bytehive)", s.branch),
    };
    let merged = match run(&["merge", "--no-ff", "-m", &msg, &s.branch], &s.main, &[]) {
        Ok(o) => o,
        Err(e) => return err(e),
    };
    if !merged.ok {
        let files = git(&["-c", "core.quotepath=false", "diff", "--name-only", "--diff-filter=U"], &s.main).unwrap_or_default();
        let _ = git(&["merge", "--abort"], &s.main);
        let files: Vec<String> = files.lines().map(str::to_string).filter(|l| !l.is_empty()).collect();
        if !files.is_empty() {
            return conflict(files);
        }
        let why = if merged.stderr.is_empty() { merged.stdout } else { merged.stderr };
        return err(if why.is_empty() { "The merge failed.".to_string() } else { why });
    }
    // 4. The work is in; its worktree and branch have done their job.
    let (removed, note) = match remove(&s, false) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(format!("Merged, but the worktree was kept: {e}"))),
    };
    LandResult::Ok { base: s.base.clone(), branch: s.branch.clone(), removed, note }
}

/// Commits `tip` has that the base doesn't.
fn ahead(s: &Sides, tip: &str) -> u64 {
    let range = format!("{}..{}", s.base, tip);
    git(&["rev-list", "--count", &range], &s.main).ok().and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// Everything on an agent's branch since it left the base: its commits and
/// what it has not committed yet (new files too), as one unified diff. This
/// is what a merge would bring in, which a plain working-tree diff misses
/// once the agent commits its own work. None when the worktree has no branch
/// of its own (the project folder itself, a detached HEAD, a folder gone).
pub fn branch_diff(worktree: &str) -> Option<String> {
    branch_diff_impl(worktree).ok()
}

fn branch_diff_impl(worktree: &str) -> Result<String, String> {
    let s = sides(worktree)?;
    let tip = snapshot(&s)?;
    let range = format!("{}...{}", s.base, tip);
    git(&["-c", "core.quotepath=false", "diff", "--no-color", &range], &s.main)
}

/// Throw an agent's branch away: remove its worktree, uncommitted work and
/// all, and delete the branch. The UI asks first.
pub fn discard_branch(worktree: &str) -> Result<(), String> {
    let s = sides(worktree)?;
    remove(&s, true).map_err(|e| format!("Couldn't discard {}: {e}", s.branch))
}

#[cfg(test)]
#[path = "land_tests.rs"]
mod tests;
