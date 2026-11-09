//! git for Review (the helpers of land.rs and review.rs in today's app), and
//! the two read-only looks the Changes drawer takes at a plain folder: its
//! repos, and a repo's working-tree diff.

use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Env for git calls that only look at a work tree. `status` and `diff`
/// otherwise take index.lock to refresh the stat cache, and an agent running
/// its own `git commit` there then dies with "index.lock exists".
pub(super) const READ_ONLY: (&str, &str) = ("GIT_OPTIONAL_LOCKS", "0");

pub(super) struct Out {
    pub ok: bool,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// git with extra env; stdout trimmed only at the end (a porcelain line can
/// start with a space that matters).
pub(super) fn run(args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Result<Out, String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd).creation_flags(CREATE_NO_WINDOW);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().map_err(|e| format!("git not available: {e}"))?;
    Ok(Out {
        ok: out.status.success(),
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

/// git, failing on a non-zero exit with git's own words.
pub(super) fn git(args: &[&str], cwd: &str) -> Result<String, String> {
    git_env(args, cwd, &[])
}

pub(super) fn git_env(args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Result<String, String> {
    let o = run(args, cwd, env)?;
    if o.ok {
        Ok(o.stdout.trim().to_string())
    } else if o.stderr.is_empty() {
        Err(format!("git {} failed", args.first().unwrap_or(&"")))
    } else {
        Err(o.stderr)
    }
}

pub(super) fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm(a) == norm(b)
}

/// Paths with uncommitted changes (untracked included), from `status -z`.
pub(super) fn dirty_paths(dir: &str) -> Result<Vec<String>, String> {
    // Not `git()`: its trim would eat the leading space of " M path".
    let o = run(&["-c", "core.quotepath=false", "status", "--porcelain", "-z", "--untracked-files=all"], dir, &[])?;
    if !o.ok {
        return Err(o.stderr);
    }
    let mut paths = Vec::new();
    let mut parts = o.stdout.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 4 || !entry.is_char_boundary(3) {
            continue;
        }
        let code = &entry[..2];
        paths.push(entry[3..].to_string());
        // A rename or copy is followed by its original path; the merge can touch that one too.
        if code.starts_with('R') || code.starts_with('C') {
            if let Some(orig) = parts.next() {
                paths.push(orig.to_string());
            }
        }
    }
    Ok(paths)
}

/// A temporary copy of a work tree's index, deleted on drop. Named per
/// process and per call, so two checks at once never share one (or its .lock).
pub(super) struct ScratchIndex {
    pub path: PathBuf,
}

impl ScratchIndex {
    pub fn copy_of(repo_root: &str) -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("bytehive-index-{}-{n}", std::process::id()));
        if let Ok(rel) = git(&["rev-parse", "--git-path", "index"], repo_root) {
            // No index yet (nothing ever staged): git starts a fresh one.
            let _ = std::fs::copy(Path::new(repo_root).join(rel), &path);
        }
        ScratchIndex { path }
    }
}

impl Drop for ScratchIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let mut lock = self.path.clone().into_os_string();
        lock.push(".lock");
        let _ = std::fs::remove_file(lock);
    }
}

/// The repos to review under `dir`: `dir`'s own repo when it is in one, else
/// each folder right under it that is a repo (a parent of repos), else none.
/// Paths use backslashes.
pub fn repos_under(dir: &str) -> Vec<String> {
    let mk = |p: &Path| p.to_string_lossy().replace('/', "\\");
    let inside = git(&["rev-parse", "--is-inside-work-tree"], dir).map(|s| s == "true").unwrap_or(false);
    if inside {
        if let Ok(top) = git(&["rev-parse", "--show-toplevel"], dir) {
            return vec![mk(Path::new(&top))];
        }
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() && p.join(".git").exists() {
                out.push(mk(&p));
            }
        }
    }
    out
}

/// A repo's working tree against HEAD as one unified diff: tracked changes
/// and new (untracked) files in full, .gitignore honoured. Empty when there
/// is nothing to show.
pub fn repo_diff(dir: &str) -> Result<String, String> {
    // `git diff` shows a new file in full only when the index knows its path.
    // Marking it intent-to-add in the real index took index.lock under the
    // agent's feet and left the files marked; a throwaway copy does it instead.
    let scratch = ScratchIndex::copy_of(dir);
    let index = scratch.path.to_string_lossy().into_owned();
    let with_index = [("GIT_INDEX_FILE", index.as_str()), READ_ONLY];
    let _ = git_env(&["add", "-N", "--", "."], dir, &with_index[..1]);
    let diff = run(&["-c", "core.quotepath=false", "diff", "--no-color", "HEAD"], dir, &with_index)
        .ok()
        .filter(|o| o.ok)
        .map(|o| o.stdout)
        .unwrap_or_default();
    Ok(diff.trim().to_string())
}

/// A folder under the system temp folder, removed when dropped.
#[cfg(test)]
pub(super) struct TempDir(pub PathBuf);

#[cfg(test)]
impl TempDir {
    pub fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = std::env::temp_dir().join(format!("bytehive-review-test-{}-{n}-{t}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{git, repo_diff, repos_under, TempDir};
    use std::path::Path;

    fn init_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        let d = dir.to_str().unwrap();
        for args in [&["init", "-q"][..], &["config", "user.email", "t@t.dev"], &["config", "user.name", "t"], &["config", "core.autocrlf", "false"]] {
            git(args, d).unwrap();
        }
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        git(&["add", "-A"], d).unwrap();
        git(&["commit", "-qm", "init"], d).unwrap();
    }

    fn name(p: &str) -> String {
        Path::new(p).file_name().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn finds_the_one_repo_when_the_folder_is_a_repo() {
        let tmp = TempDir::new();
        let repo = tmp.path().join("solo");
        init_repo(&repo);
        std::fs::create_dir(repo.join("sub")).unwrap();
        let got = repos_under(repo.join("sub").to_str().unwrap());
        assert_eq!(got.len(), 1);
        assert_eq!(name(&got[0]), "solo");
        assert!(!got[0].contains('/'), "{}", got[0]);
    }

    #[test]
    fn finds_the_repos_under_a_parent() {
        let tmp = TempDir::new();
        let parent = tmp.path().join("parent");
        for n in ["repoA", "repoB"] {
            init_repo(&parent.join(n));
        }
        std::fs::create_dir(parent.join("plain")).unwrap(); // not a repo: left out
        let got: Vec<String> = repos_under(parent.to_str().unwrap()).iter().map(|p| name(p)).collect();
        assert_eq!(got, vec!["repoA".to_string(), "repoB".to_string()]);
    }

    #[test]
    fn repo_diff_shows_changes_and_new_files_and_leaves_the_index_alone() {
        let tmp = TempDir::new();
        let repo = tmp.path().join("r");
        init_repo(&repo);
        let root = repo.to_str().unwrap();
        assert_eq!(repo_diff(root).unwrap(), "", "a clean tree has no diff");
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap();
        let index = repo.join(".git").join("index");
        let before = std::fs::read(&index).unwrap();
        let d = repo_diff(root).unwrap();
        assert!(d.contains("a.txt") && d.contains("+two"), "{d}");
        assert!(d.contains("new.txt") && d.contains("+fresh"), "new files must be reviewable: {d}");
        // Nothing marked intent-to-add, index bytes untouched.
        assert_eq!(git(&["ls-files"], root).unwrap(), "a.txt");
        assert_eq!(std::fs::read(&index).unwrap(), before, "index must not be rewritten");
    }

    #[test]
    fn repo_diff_works_while_an_agent_holds_index_lock() {
        let tmp = TempDir::new();
        let repo = tmp.path().join("locked");
        init_repo(&repo);
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap();
        let lock = repo.join(".git").join("index.lock");
        std::fs::write(&lock, "").unwrap(); // an agent's `git commit` in flight
        let d = repo_diff(repo.to_str().unwrap()).unwrap();
        assert!(d.contains("+two") && d.contains("+fresh"), "diff despite the lock: {d}");
        assert!(lock.exists(), "the agent's lock is not ours to touch");
    }
}
