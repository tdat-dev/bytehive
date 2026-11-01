//! Git facts for review, through the git CLI: which branch a folder is on,
//! what changed in it, and the diff itself. Best-effort everywhere: a folder
//! that is not a repo, or a git that fails, reads as empty, never as an error
//! the UI must explain. (Checkpoints share the rule: a failure never blocks.)

use std::path::Path;
use std::process::Command;

pub(crate) fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(cwd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// The branch a folder is on, or none when it is not on one.
pub fn current_branch(cwd: &Path) -> Option<String> {
    let branch = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}

/// The top of the repo a folder sits in, with Windows separators (the form
/// `worktree_path_for` hashes, so D:/x and D:\x land in one place), or none
/// when the folder is not in a repo.
pub fn repo_root(cwd: &Path) -> Option<String> {
    let top = git(cwd, &["rev-parse", "--show-toplevel"])?.trim().to_string();
    if top.is_empty() {
        None
    } else if cfg!(windows) {
        Some(top.replace('/', "\\"))
    } else {
        Some(top)
    }
}

/// Paths changed in the working tree (staged, unstaged, untracked), relative
/// to the repo root, or empty when nothing is known.
pub fn changed_files(cwd: &Path) -> Vec<String> {
    let Some(out) = git(cwd, &["status", "--porcelain=v1", "--untracked-files=normal", "-z"]) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut chunks = out.split('\0').peekable();
    while let Some(entry) = chunks.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = entry.split_at(3);
        let path = path.trim_start();
        if path.is_empty() {
            continue;
        }
        // Renames print "old -> new" plus a second NUL part; keep the new side.
        if xy.as_bytes().first() == Some(&b'R') || xy.as_bytes().get(1) == Some(&b'R') {
            let _ = chunks.next();
        }
        let path = match path.split_once(" -> ") {
            Some((_, new)) => new,
            None => path,
        };
        // Quoted when it holds odd bytes; unquote the plain case.
        let path = path.strip_prefix('"').and_then(|p| p.strip_suffix('"')).unwrap_or(path);
        files.push(path.to_string());
    }
    files.sort();
    files.dedup();
    files
}

/// The working tree's diff against HEAD, for parsing and scanning.
pub fn diff_text(cwd: &Path) -> String {
    git(cwd, &["diff", "HEAD", "--"]).unwrap_or_default()
}

/// Diff fingerprint (files, added, removed) for stall detection. Untracked
/// files count as changed too, so an agent that only creates new files does
/// not read as stalled. None when nothing is known.
pub fn diff_stat(cwd: &Path) -> Option<(u64, u64, u64)> {
    let numstat = git(cwd, &["diff", "--numstat", "HEAD", "--"])?;
    let mut files = 0;
    let mut add = 0;
    let mut del = 0;
    for line in numstat.lines() {
        let mut parts = line.split('\t');
        let (a, d) = (parts.next()?, parts.next()?);
        if a == "-" || d == "-" {
            files += 1; // binary: changed, size unknown
            continue;
        }
        files += 1;
        add += a.parse::<u64>().unwrap_or(0);
        del += d.parse::<u64>().unwrap_or(0);
    }
    let untracked = git(cwd, &["ls-files", "--others", "--exclude-standard", "-z"]).map(|o| o.split('\0').filter(|p| !p.is_empty()).count() as u64).unwrap_or(0);
    files += untracked;
    Some((files, add, del))
}

/// A best-effort checkpoint: stash what is there without touching the tree,
/// and remember it. Returns the stash id, or none when there is nothing to
/// keep or git said no.
pub fn checkpoint(cwd: &Path, message: &str) -> Option<String> {
    let sha = git(cwd, &["stash", "create"])?.trim().to_string();
    if sha.is_empty() {
        return None;
    }
    let status = Command::new("git").arg("-C").arg(cwd).args(["stash", "store", "-m", message, &sha]).status().ok()?;
    if status.success() {
        Some(sha)
    } else {
        None
    }
}

/// Create a worktree for an agent at `path`, on a new `branch` from `base`.
/// Returns the branch when git accepted it.
pub fn create_worktree(repo: &Path, path: &Path, branch: &str, base: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "-b", branch])
        .arg(path)
        .arg(base)
        .output()
        .ok()?;
    if out.status.success() {
        Some(branch.to_string())
    } else {
        None
    }
}

/// Remove an agent's worktree after its branch landed. Forced: the branch is
/// merged, the folder is scratch.
pub fn remove_worktree(repo: &Path, path: &Path) -> bool {
    Command::new("git").arg("-C").arg(repo).args(["worktree", "remove", "--force"]).arg(path).status().map(|s| s.success()).unwrap_or(false)
}

/// What merging an agent branch into its base came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Landed on base.
    Merged,
    /// Would conflict: nothing was merged, the tree is as it was.
    Conflicts(Vec<String>),
    /// The main checkout is dirty: merge nothing rather than touch it.
    DirtyMain,
    /// Git refused for another reason.
    Failed(String),
}

/// Merge `branch` into `base` in the main checkout. The main worktree must be
/// clean, and it is left on `base`: conflicts are aborted, never left hanging.
pub fn merge_branch(repo: &Path, branch: &str, base: &str) -> MergeOutcome {
    if !changed_files(repo).is_empty() {
        return MergeOutcome::DirtyMain;
    }
    let run = |args: &[&str]| Command::new("git").arg("-C").arg(repo).args(args).output().ok();
    let prev = current_branch(repo).unwrap_or_else(|| base.to_string());
    let back = || {
        let _ = run(&["checkout", "--quiet", &prev]);
    };
    match run(&["checkout", "--quiet", base]) {
        Some(o) if o.status.success() => {}
        Some(o) => return MergeOutcome::Failed(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        None => return MergeOutcome::Failed("git not found".to_string()),
    }
    match run(&["merge", "--no-ff", "--no-commit", branch]) {
        Some(o) if o.status.success() => {}
        Some(_) => {
            let files = git(repo, &["diff", "--name-only", "--diff-filter=U"]).map(|o| o.lines().map(|l| l.to_string()).collect()).unwrap_or_default();
            let _ = run(&["merge", "--abort"]);
            back();
            return MergeOutcome::Conflicts(files);
        }
        None => return MergeOutcome::Failed("git not found".to_string()),
    }
    match run(&["commit", "--no-edit", "--quiet"]) {
        Some(o) if o.status.success() => {
            back();
            MergeOutcome::Merged
        }
        Some(o) => {
            let _ = run(&["merge", "--abort"]);
            back();
            MergeOutcome::Failed(String::from_utf8_lossy(&o.stderr).trim().to_string())
        }
        None => MergeOutcome::Failed("git not found".to_string()),
    }
}

/// The diff of a branch against its base (what review reads for worktree panes).
pub fn branch_diff_text(repo: &Path, base: &str, branch: &str) -> String {
    let range = format!("{base}...{branch}");
    git(repo, &["diff", &range, "--"]).unwrap_or_default()
}

/// Files a branch changes against its base.
pub fn branch_files(repo: &Path, base: &str, branch: &str) -> Vec<String> {
    let range = format!("{base}...{branch}");
    git(repo, &["diff", "--name-only", &range, "--"]).map(|o| o.lines().map(|l| l.to_string()).collect()).unwrap_or_default()
}

/// A short, stable, filesystem-safe slug for an arbitrary string.
fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// djb2 hash → 6-char base36, so two repos with the same folder name don't collide.
fn short_hash(s: &str) -> String {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    let mut n = h;
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = [b'0'; 6];
    for i in (0..6).rev() {
        buf[i] = digits[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8(buf.to_vec()).unwrap()
}

/// Compute the worktree directory for a repo + branch:
/// `<drive>:\.maestro-worktrees\<repo-slug>-<hash>\<branch-slug>-<branch-hash>`.
/// The root sits on the repo's own drive (never C:/profile) and out of the repo tree.
/// NOTE: UNC paths (\\server\share) are unsupported — Windows-local repos only.
pub fn worktree_path_for(repo_root: &str, branch: &str) -> std::path::PathBuf {
    let root = Path::new(repo_root);
    let repo_name = root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "repo".into());
    // Drive prefix on Windows (e.g. "D:"); fall back to the path root otherwise.
    let drive = repo_root.get(0..2).filter(|d| d.ends_with(':')).unwrap_or("");
    let folder = format!("{}-{}", slug(&repo_name), short_hash(repo_root));
    let leaf = format!("{}-{}", slug(branch), short_hash(branch));
    let mut p = std::path::PathBuf::new();
    p.push(format!("{drive}\\"));
    p.push(".maestro-worktrees");
    p.push(folder);
    p.push(leaf);
    p
}

/// The folder that holds every worktree of a repo (the parent of
/// `worktree_path_for`'s), where its agents' sessions ran.
pub fn worktrees_of(repo_root: &str) -> std::path::PathBuf {
    let mut p = worktree_path_for(repo_root, "x");
    p.pop();
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn git_ok() -> bool {
        Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    fn fresh_repo() -> Option<std::path::PathBuf> {
        if !git_ok() {
            return None;
        }
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("maestro-git-test-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&dir).ok()?;
        let run = |args: &[&str]| Command::new("git").arg("-C").arg(&dir).args(args).output().ok().filter(|o| o.status.success());
        run(&["init"])?;
        run(&["config", "user.email", "test@maestro.local"])?;
        run(&["config", "user.name", "maestro-test"])?;
        run(&["config", "commit.gpgsign", "false"])?;
        run(&["config", "core.autocrlf", "false"])?;
        std::fs::write(dir.join("a.txt"), "one\n").ok()?;
        run(&["add", "."])?;
        run(&["commit", "-m", "first"])?;
        Some(dir)
    }

    #[test]
    fn reads_branch_changes_and_diff() {
        let Some(dir) = fresh_repo() else { return };
        assert!(current_branch(&dir).is_some());
        assert!(changed_files(&dir).is_empty());
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.join("new.txt"), "hello\n").unwrap();
        let mut changed = changed_files(&dir);
        changed.sort();
        assert_eq!(changed, vec!["a.txt".to_string(), "new.txt".to_string()]);
        let diff = diff_text(&dir);
        assert!(diff.contains("a.txt"));
        assert!(diff.contains("+two"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn checkpoint_keeps_work_without_touching_the_tree() {
        let Some(dir) = fresh_repo() else { return };
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        let sha = checkpoint(&dir, "maestro test checkpoint");
        assert!(sha.is_some());
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn diff_stat_counts_tracked_and_untracked() {
        let Some(dir) = fresh_repo() else { return };
        assert_eq!(diff_stat(&dir), Some((0, 0, 0)));
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.join("new.txt"), "hello\n").unwrap();
        assert_eq!(diff_stat(&dir), Some((2, 1, 0)));
        std::fs::remove_dir_all(&dir).ok();
    }

    fn commit_on(dir: &std::path::Path, branch: Option<&str>, file: &str, content: &str) -> bool {
        let run = |args: &[&str]| Command::new("git").arg("-C").arg(dir).args(args).output().ok().filter(|o| o.status.success()).is_some();
        if let Some(b) = branch {
            if !run(&["checkout", "-b", b]) {
                return false;
            }
        }
        if std::fs::write(dir.join(file), content).is_err() {
            return false;
        }
        run(&["add", "."]) && run(&["commit", "-m", "work"])
    }

    #[test]
    fn worktree_round_trip() {
        let Some(dir) = fresh_repo() else { return };
        let base = current_branch(&dir).unwrap();
        let wt = dir.join("wt-agent");
        assert_eq!(create_worktree(&dir, &wt, "agent/one", &base), Some("agent/one".to_string()));
        assert!(wt.join("a.txt").is_file());
        assert_eq!(current_branch(&wt).as_deref(), Some("agent/one"));
        assert!(remove_worktree(&dir, &wt));
        assert!(!wt.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repo_root_finds_the_top_from_a_subfolder() {
        let Some(dir) = fresh_repo() else { return };
        let sub = dir.join("deep").join("er");
        std::fs::create_dir_all(&sub).unwrap();
        let top = repo_root(&sub).expect("inside a repo");
        let want = std::fs::canonicalize(&dir).unwrap();
        assert_eq!(std::fs::canonicalize(&top).unwrap(), want);
        if cfg!(windows) {
            assert!(!top.contains('/'), "Windows separators: {top}");
        }
        assert_eq!(repo_root(&std::env::temp_dir().join("maestro-surely-no-repo-here")), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn merge_clean_branch_and_read_its_diff_first() {
        let Some(dir) = fresh_repo() else { return };
        let base = current_branch(&dir).unwrap();
        assert!(commit_on(&dir, Some("agent/two"), "b.txt", "work\n"));
        assert_eq!(branch_files(&dir, &base, "agent/two"), vec!["b.txt".to_string()]);
        assert!(branch_diff_text(&dir, &base, "agent/two").contains("+work"));
        assert_eq!(merge_branch(&dir, "agent/two", &base), MergeOutcome::Merged);
        assert!(dir.join("b.txt").is_file());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn merge_conflict_reports_files_and_restores() {
        let Some(dir) = fresh_repo() else { return };
        let base = current_branch(&dir).unwrap();
        assert!(commit_on(&dir, Some("agent/three"), "a.txt", "theirs\n"));
        assert!(Command::new("git").arg("-C").arg(&dir).args(["checkout", "--quiet", &base]).status().map(|s| s.success()).unwrap_or(false));
        assert!(commit_on(&dir, None, "a.txt", "ours\n"));
        assert_eq!(merge_branch(&dir, "agent/three", &base), MergeOutcome::Conflicts(vec!["a.txt".to_string()]));
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "ours\n");
        assert!(changed_files(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn merge_refuses_a_dirty_main() {
        let Some(dir) = fresh_repo() else { return };
        let base = current_branch(&dir).unwrap();
        assert!(commit_on(&dir, Some("agent/four"), "b.txt", "work\n"));
        assert!(Command::new("git").arg("-C").arg(&dir).args(["checkout", "--quiet", &base]).status().map(|s| s.success()).unwrap_or(false));
        std::fs::write(dir.join("a.txt"), "dirty\n").unwrap();
        assert_eq!(merge_branch(&dir, "agent/four", &base), MergeOutcome::DirtyMain);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn path_lives_on_repo_drive_under_hidden_root() {
        let p = worktree_path_for("D:\\WhaleloSource\\payments-svc", "maestro/claude-a1b2").to_string_lossy().to_string();
        assert!(p.starts_with("D:\\.maestro-worktrees\\"), "got {p}");
        assert!(p.contains("payments-svc-"), "got {p}");
        assert!(p.contains("maestro-claude-a1b2-"), "got {p}");
    }

    #[test]
    fn same_name_different_path_differ() {
        let a = worktree_path_for("D:\\a\\app", "maestro/x");
        let b = worktree_path_for("D:\\b\\app", "maestro/x");
        assert_ne!(a, b);
    }

    #[test]
    fn non_repos_read_as_empty() {
        let dir = std::env::temp_dir().join(format!("maestro-not-a-repo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        assert_eq!(current_branch(&dir), None);
        assert!(changed_files(&dir).is_empty());
        assert!(diff_text(&dir).is_empty());
        assert_eq!(checkpoint(&dir, "x"), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
