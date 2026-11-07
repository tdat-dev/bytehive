use crate::error::{run_blocking, CommandError};
use serde::Serialize;

// Every command here shells out to git (possibly several times, on big repos)
// and is therefore an async wrapper over a sync `*_impl` body: sync Tauri
// commands run on the main thread and freeze the UI for the duration. Tests
// exercise the `*_impl` functions directly.
use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// CREATE_NO_WINDOW — without it, git children spawned from the windowed (release)
/// build pop up a flashing console window that steals focus and looks like lag.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn git(args: &[&str], cwd: &str) -> Result<String, CommandError> {
    Ok(git_env(args, cwd, &[])?.trim().to_string())
}

/// Env for git calls that only LOOK at a work tree. `status`/`diff` otherwise
/// take `index.lock` to refresh the index's stat cache, and these run every few
/// seconds in trees where an agent is running its own git: its `git commit`
/// then dies with "index.lock exists".
const READ_ONLY: [(&str, &str); 1] = [("GIT_OPTIONAL_LOCKS", "0")];

/// Run git with extra env vars; stdout untrimmed (porcelain lines can start
/// with a space that matters).
fn git_env(args: &[&str], cwd: &str, envs: &[(&str, &str)]) -> Result<String, CommandError> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd
        .output()
        .map_err(|e| CommandError::Failed(format!("git not available: {e}")))?;
    if !out.status.success() {
        return Err(CommandError::Failed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Serialize)]
pub struct RepoRef {
    pub path: String,
    pub name: String,
}

/// Discover the git repos to review under `dir`:
/// - if `dir` itself is a git repo → return just it;
/// - else → each immediate sub-folder that is a git repo (parent-of-repos case);
/// - else → empty.
#[tauri::command]
pub async fn git_repos_under(dir: String) -> Vec<RepoRef> {
    run_blocking(move || Ok(git_repos_under_impl(dir)))
        .await
        .unwrap_or_default()
}

fn git_repos_under_impl(dir: String) -> Vec<RepoRef> {
    let mk = |p: &Path| RepoRef {
        path: p.to_string_lossy().replace('/', "\\"),
        name: p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    };
    // Case 1: dir itself is a repo.
    if git(&["rev-parse", "--is-inside-work-tree"], &dir)
        .map(|s| s == "true")
        .unwrap_or(false)
    {
        if let Ok(top) = git(&["rev-parse", "--show-toplevel"], &dir) {
            return vec![mk(Path::new(&top))];
        }
    }
    // Case 2: immediate sub-folders that are repos.
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() && p.join(".git").exists() {
                out.push(mk(&p));
            }
        }
    }
    out
}

/// Raw unified diff of a repo's working tree vs HEAD — tracked changes AND
/// newly created (untracked) files, which show as full additions. Honors
/// .gitignore. Empty string when there is nothing to show. For the diff viewer,
/// on demand: the task list's line counts come from `git_changed_files`.
#[tauri::command]
pub async fn repo_diff(repo_root: String) -> Result<String, CommandError> {
    run_blocking(move || repo_diff_impl(repo_root)).await
}

fn repo_diff_impl(repo_root: String) -> Result<String, CommandError> {
    // `git diff` only shows a new file in full when the index knows its path.
    // Marking untracked files intent-to-add (`add -N`) in the REAL index took
    // index.lock under the agent's feet and left the files marked; do it in a
    // throwaway copy of the index instead.
    let scratch = ScratchIndex::copy_of(&repo_root);
    let index = scratch.path.to_string_lossy().to_string();
    let with_index = [("GIT_INDEX_FILE", index.as_str()), READ_ONLY[0]];
    let _ = git_env(&["add", "-N", "--", "."], &repo_root, &with_index[..1]);
    let diff = git_env(
        &["-c", "core.quotepath=false", "diff", "--no-color", "HEAD"],
        &repo_root,
        &with_index,
    )
    .unwrap_or_default();
    Ok(diff.trim().to_string())
}

/// The only untracked paths that are not an agent's work: files a machine wrote
/// for itself, named one by one. Everything else counts, however it looks: a
/// pattern here once hid `.claude/commands/ship.md` from Review and then deleted
/// it with the worktree. Files git tracks count wherever they are, and ignored
/// ones never come up. `.claude/worktrees` is not here: it holds real checkouts.
pub(crate) const MACHINE_FILES: [&str; 5] = [
    ".claude/settings.local.json",
    ".bytehive/board.json",
    ".bytehive/fleet.json",
    ".bytehive/outbox.jsonl",
    ".bytehive/spawn-requests.jsonl",
];

fn is_machine_file(path: &str) -> bool {
    MACHINE_FILES.contains(&path)
}

/// Every untracked, unignored path in `dir`. git shows a checkout nested in it as
/// the one folder, with a trailing slash.
fn untracked(dir: &str) -> Result<Vec<String>, CommandError> {
    let out = git_env(&["-c", "core.quotepath=false", "ls-files", "--others", "--exclude-standard", "-z"], dir, &READ_ONLY)?;
    Ok(out.split('\0').filter(|p| !p.is_empty() && !is_machine_file(p)).map(str::to_string).collect())
}

/// Untracked files that are work: not ignored, not the machine's, and not a
/// nested checkout, which `add` would turn into a gitlink.
pub(crate) fn new_work(dir: &str) -> Result<Vec<String>, CommandError> {
    Ok(untracked(dir)?.into_iter().filter(|p| !p.ends_with('/')).collect())
}

/// Checkouts nested in `dir`, with whatever uncommitted work they hold.
pub(crate) fn nested_checkouts(dir: &str) -> Result<Vec<String>, CommandError> {
    Ok(untracked(dir)?.into_iter().filter(|p| p.ends_with('/')).collect())
}

/// Folders a build makes again, so losing them with a worktree costs nothing.
const REGENERABLE: [&str; 7] = ["node_modules", "target", "dist", ".next", "build", "__pycache__", ".venv"];

fn regenerable(path: &str) -> bool {
    path.split('/').any(|part| REGENERABLE.contains(&part))
}

/// Ignored files in `dir` that nothing can bring back: a `.env`, a local database.
/// `worktree remove` deletes ignored files without a word, forced or not.
pub(crate) fn ignored_keepers(dir: &str) -> Result<Vec<String>, CommandError> {
    let list = |extra: &[&str]| -> Result<Vec<String>, CommandError> {
        let mut args = vec!["--literal-pathspecs", "-c", "core.quotepath=false", "ls-files", "--others", "--ignored", "--exclude-standard", "-z"];
        args.extend(extra);
        Ok(git_env(&args, dir, &READ_ONLY)?.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect())
    };
    let mut keep = Vec::new();
    // Folded by folder, so node_modules is not walked file by file. A folded folder
    // that is not regenerable is opened: it may hold one, or only machine files.
    for entry in list(&["--directory"])? {
        if regenerable(&entry) {
            continue;
        }
        if entry.ends_with('/') {
            keep.extend(list(&["--", &entry])?);
        } else {
            keep.push(entry);
        }
    }
    keep.retain(|p| !is_machine_file(p) && !regenerable(p));
    keep.sort();
    keep.dedup();
    Ok(keep)
}

/// Names for a note: ten, then how many more.
pub(crate) fn names_capped(names: &[String]) -> String {
    const SHOWN: usize = 10;
    let list = names[..names.len().min(SHOWN)].join(", ");
    if names.len() > SHOWN { format!("{list} and {} more", names.len() - SHOWN) } else { list }
}

/// `git add <flag>` the new work in `dir`. By name from a file, not by a pathspec
/// that leaves the machine files out: git refuses such a pathspec when one of them
/// is ignored, and a list can be longer than a command line.
pub(crate) fn add_new_work(dir: &str, flag: &str, envs: &[(&str, &str)]) -> Result<(), CommandError> {
    twice(|| add_listed(dir, flag, envs, &new_work(dir)?))
}

/// One more try: a file can go between listing it and adding it, and an agent
/// deleting a temp file must not look like git failing.
fn twice(mut attempt: impl FnMut() -> Result<(), CommandError>) -> Result<(), CommandError> {
    attempt().or_else(|_| attempt())
}

fn add_listed(dir: &str, flag: &str, envs: &[(&str, &str)], files: &[String]) -> Result<(), CommandError> {
    if files.is_empty() {
        return Ok(());
    }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let list = std::env::temp_dir().join(format!("bytehive-paths-{}-{n}", std::process::id()));
    std::fs::write(&list, files.join("\0")).map_err(|e| CommandError::Failed(e.to_string()))?;
    let from = format!("--pathspec-from-file={}", list.display());
    let done = git_env(&["--literal-pathspecs", "add", flag, "--pathspec-file-nul", &from], dir, envs);
    let _ = std::fs::remove_file(&list);
    done.map(|_| ())
}

/// The machine files lying untracked in `dir`: what Review and a merge leave out.
pub(crate) fn left_out_of(dir: &str) -> Result<Vec<String>, CommandError> {
    let mut args = vec!["--literal-pathspecs", "-c", "core.quotepath=false", "ls-files", "--others", "--exclude-standard", "-z", "--"];
    args.extend(MACHINE_FILES);
    let out = git_env(&args, dir, &READ_ONLY)?;
    let mut names: Vec<String> = out.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect();
    names.sort();
    Ok(names)
}

/// "2 files left out: a, b", or None with nothing left out.
pub(crate) fn left_out_words(names: &[String]) -> Option<String> {
    match names.len() {
        0 => None,
        1 => Some(format!("1 file left out: {}", names[0])),
        n => Some(format!("{n} files left out: {}", names_capped(names))),
    }
}

/// What Review says was left out of an agent's own worktree.
#[tauri::command]
pub async fn agent_left_out(dir: String) -> Result<Vec<String>, CommandError> {
    run_blocking(move || left_out_of(&repo_of(&dir)?)).await
}

/// The repo of a folder an agent works in: the folder's own, or the first one
/// under it, the same one Review has always shown for a parent of repos.
fn repo_of(dir: &str) -> Result<String, CommandError> {
    git_repos_under_impl(dir.to_string())
        .into_iter()
        .next()
        .map(|r| r.path)
        .ok_or_else(|| CommandError::Failed(format!("Not a git repository: {dir}")))
}

/// The one answer to "what did this agent change?". `git diff` from `from` to
/// the files on disk: commits since then, uncommitted edits, and new files that
/// are not junk. `view` picks the shape (numbers or text), so the row, the
/// Ready to review state and Review cannot disagree. `from` None is where the
/// agent's own branch left its base, or HEAD for a folder with no such branch;
/// `paths` narrows it to the files a shared folder's agent changed.
pub(crate) fn diff_of(dir: &str, from: Option<&str>, paths: Option<&[String]>, view: &[&str]) -> Result<String, CommandError> {
    let repo = repo_of(dir)?;
    let fork = match from {
        Some(f) => f.to_string(),
        None => crate::land::fork_point(&repo).unwrap_or_else(|_| "HEAD".into()),
    };
    // The throwaway index again: see repo_diff_impl.
    let scratch = ScratchIndex::copy_of(&repo);
    let index = scratch.path.to_string_lossy().to_string();
    let with_index = [("GIT_INDEX_FILE", index.as_str()), READ_ONLY[0]];
    add_new_work(&repo, "-N", &with_index[..1])?;
    // File names, not patterns: `[id].tsx` is a real name.
    let mut args = vec!["--literal-pathspecs", "-c", "core.quotepath=false", "diff"];
    args.extend(view);
    args.push(&fork);
    if let Some(p) = paths {
        args.push("--");
        args.extend(p.iter().map(String::as_str));
    }
    git_env(&args, &repo, &with_index)
}

/// What an agent changed, file by file with its line counts. See `diff_of`.
#[tauri::command]
pub async fn agent_changes(dir: String, from: Option<String>, paths: Option<Vec<String>>) -> Result<Vec<ChangedFile>, CommandError> {
    run_blocking(move || agent_changes_impl(&dir, from.as_deref(), paths.as_deref())).await
}

fn agent_changes_impl(dir: &str, from: Option<&str>, paths: Option<&[String]>) -> Result<Vec<ChangedFile>, CommandError> {
    if paths.is_some_and(|p| p.is_empty()) {
        return Ok(Vec::new());
    }
    let out = diff_of(dir, from, paths, &["--numstat", "-z"])?;
    Ok(parse_numstat(&out)
        .into_iter()
        .map(|(path, (added, removed))| ChangedFile {
            path: path.replace('/', "\\"),
            status: "M".into(),
            added: Some(added),
            removed: Some(removed),
        })
        .collect())
}

/// The same changes as text, for Review.
#[tauri::command]
pub async fn agent_diff(dir: String, from: Option<String>, paths: Option<Vec<String>>) -> Result<String, CommandError> {
    run_blocking(move || agent_diff_impl(&dir, from.as_deref(), paths.as_deref())).await
}

pub(crate) fn agent_diff_impl(dir: &str, from: Option<&str>, paths: Option<&[String]>) -> Result<String, CommandError> {
    if paths.is_some_and(|p| p.is_empty()) {
        return Ok(String::new());
    }
    Ok(diff_of(dir, from, paths, &["--no-color"])?.trim().to_string())
}

#[derive(Serialize)]
pub struct FolderState {
    /// HEAD now; None in a repo with no commit yet.
    pub head: Option<String>,
    /// Every uncommitted path that is not junk, with a stamp that changes when
    /// the file does.
    pub files: std::collections::HashMap<String, String>,
    /// Paths commits changed between `since` and HEAD.
    pub committed: Vec<String>,
    /// HEAD left `since` by something other than a commit: a pull, a branch
    /// switch, a reset. What differs then came from elsewhere.
    pub jumped: bool,
}

/// How a folder several agents may share stands right now: tasks.ts compares two
/// of these to see which files moved, and who was working when they did.
#[tauri::command]
pub async fn folder_state(dir: String, since: Option<String>) -> Result<FolderState, CommandError> {
    run_blocking(move || folder_state_impl(&dir, since.as_deref())).await
}

fn folder_state_impl(dir: &str, since: Option<&str>) -> Result<FolderState, CommandError> {
    let repo = repo_of(dir)?;
    let names = |out: String| -> Vec<String> { out.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect() };
    let mut dirty = names(git_env(
        &["-c", "core.quotepath=false", "diff", "--name-only", "-z", "HEAD"],
        &repo,
        &READ_ONLY,
    )
    .unwrap_or_default());
    dirty.extend(new_work(&repo)?);
    let files = dirty
        .into_iter()
        .map(|p| {
            let stamp = stamp_of(&Path::new(&repo).join(&p));
            (p, stamp)
        })
        .collect();
    let head = git(&["rev-parse", "--verify", "-q", "HEAD"], &repo).ok().filter(|h| !h.is_empty());
    let (committed, jumped) = match (since, head.as_deref()) {
        (Some(from), Some(to)) if from != to => {
            let range = format!("{from}..{to}");
            let paths = names(git_env(&["-c", "core.quotepath=false", "diff", "--name-only", "-z", &range], &repo, &READ_ONLY).unwrap_or_default());
            (paths, head_jumped(&repo, from))
        }
        _ => (Vec::new(), false),
    };
    Ok(FolderState { head, files, committed, jumped })
}

/// Did HEAD get here from `since` by anything but commits? The reflog says how
/// each step was made. Past its end nobody can say, so it counts as a jump: that
/// moves a baseline, where a wrong "no" would hand an agent a whole pull.
fn head_jumped(repo: &str, since: &str) -> bool {
    let log = git_env(&["log", "-g", "-n", "200", "--format=%H %gs", "HEAD"], repo, &READ_ONLY).unwrap_or_default();
    for line in log.lines() {
        let (hash, how) = line.split_once(' ').unwrap_or((line, ""));
        if hash == since {
            return false;
        }
        if !how.starts_with("commit") {
            return true;
        }
    }
    true
}

/// Size and modified time, or "gone" for a deleted file.
fn stamp_of(path: &Path) -> String {
    let Ok(m) = std::fs::metadata(path) else { return "gone".into() };
    let at = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis());
    format!("{}:{at}", m.len())
}

/// A temporary copy of a work tree's index, deleted on drop. Named per process
/// and per call, so two checks at once never share one (or its .lock).
pub(crate) struct ScratchIndex {
    pub(crate) path: std::path::PathBuf,
}

impl ScratchIndex {
    pub(crate) fn copy_of(repo_root: &str) -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("bytehive-index-{}-{n}", std::process::id()));
        if let Ok(rel) = git(&["rev-parse", "--git-path", "index"], repo_root) {
            // Missing index (a repo with nothing staged yet) → git starts a fresh one.
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

#[derive(Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    /// Lines added / removed vs HEAD; None when unknown (e.g. no commit yet).
    /// Binary files count 0.
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

/// Files changed in the working tree vs HEAD (including untracked), as porcelain
/// status pairs with their line counts. Used to attach "what changed" evidence
/// to a finished task and polled for the task list's +/- counts. Read-only: it
/// never writes the index, so it can run beside an agent's own git.
#[tauri::command]
pub async fn git_changed_files(repo_root: String) -> Result<Vec<ChangedFile>, CommandError> {
    run_blocking(move || git_changed_files_impl(repo_root)).await
}

/// Untracked files counted line by line; past this many they count as unknown.
const MAX_UNTRACKED_COUNTED: usize = 500;
/// Only this much of an untracked file is read to count its lines.
const MAX_COUNT_BYTES: u64 = 1024 * 1024;

fn git_changed_files_impl(repo_root: String) -> Result<Vec<ChangedFile>, CommandError> {
    let out = git_env(
        &["-c", "core.quotepath=false", "status", "--porcelain", "-z", "-uall"],
        &repo_root,
        &READ_ONLY,
    )
    .unwrap_or_default();
    let counts = numstat(&repo_root);
    let mut files = Vec::new();
    let mut untracked = 0usize;
    let mut tokens = out.split('\0');
    while let Some(entry) = tokens.next() {
        if entry.len() < 4 || !entry.is_char_boundary(3) {
            continue;
        }
        let status = entry[..2].trim().to_string();
        let path = entry[3..].to_string();
        // Renames/copies: -z puts the original path in the next field.
        if status.contains('R') || status.contains('C') {
            tokens.next();
        }
        let (added, removed) = if status == "??" {
            untracked += 1;
            let n = if untracked <= MAX_UNTRACKED_COUNTED {
                count_lines(&Path::new(&repo_root).join(&path))
            } else {
                None
            };
            (n, n.map(|_| 0))
        } else if let Some(c) = &counts {
            let (a, d) = c.get(&path).copied().unwrap_or((0, 0));
            (Some(a), Some(d))
        } else {
            (None, None)
        };
        files.push(ChangedFile {
            path: path.replace('/', "\\"),
            status,
            added,
            removed,
        });
    }
    Ok(files)
}

/// Lines added/removed per tracked path vs HEAD (`git diff --numstat`), keyed
/// by git's forward-slash path. None when git can't say (e.g. no HEAD yet).
fn numstat(repo_root: &str) -> Option<std::collections::HashMap<String, (u64, u64)>> {
    let out = git_env(
        &["-c", "core.quotepath=false", "diff", "--numstat", "-z", "HEAD"],
        repo_root,
        &READ_ONLY,
    )
    .ok()?;
    Some(parse_numstat(&out))
}

/// `git diff --numstat -z` as path → (added, removed).
fn parse_numstat(out: &str) -> std::collections::HashMap<String, (u64, u64)> {
    let mut map = std::collections::HashMap::new();
    let mut tokens = out.split('\0');
    while let Some(entry) = tokens.next() {
        let mut parts = entry.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        // A rename leaves the path empty and puts "old\0new" in the next fields.
        let path = if path.is_empty() {
            tokens.next();
            match tokens.next() {
                Some(new) => new,
                None => continue,
            }
        } else {
            path
        };
        // Binary files show "-".
        let n = |s: &str| s.parse::<u64>().unwrap_or(0);
        map.insert(path.to_string(), (n(a), n(d)));
    }
    map
}

/// Lines in a new file, the way `git diff` counts them (a last line without a
/// newline counts too). Binary → 0. Reads at most MAX_COUNT_BYTES.
fn count_lines(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_COUNT_BYTES)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.iter().take(8000).any(|&b| b == 0) {
        return Some(0);
    }
    let newlines = buf.iter().filter(|&&b| b == b'\n').count() as u64;
    let unterminated = buf.last().is_some_and(|&b| b != b'\n');
    Some(newlines + unterminated as u64)
}

// ===================== Slice 2: write side (commit / merge / discard) =====================

/// Guard: the path must exist and be inside a git work tree. Returns a typed
/// error otherwise so the UI can surface it instead of silently no-op'ing.
fn ensure_work_tree(path: &str) -> Result<(), CommandError> {
    if !Path::new(path).is_dir() {
        return Err(CommandError::Failed(format!("path does not exist: {path}")));
    }
    let inside = git(&["rev-parse", "--is-inside-work-tree"], path)
        .map(|s| s == "true")
        .unwrap_or(false);
    if !inside {
        return Err(CommandError::Failed(format!("not a git work tree: {path}")));
    }
    Ok(())
}

/// What an agent's reviewed repo path actually is, so the UI can show the right
/// affordances: a per-agent **worktree** can be committed AND merged into its
/// parent repo; a plain repo can only be committed into itself.
#[derive(Serialize)]
pub struct RepoInfo {
    /// Current branch name (empty if detached HEAD).
    pub branch: String,
    /// True when this path is a linked worktree (not the main checkout).
    pub is_worktree: bool,
    /// The main repo's working dir when `is_worktree` — where a merge runs.
    pub main_root: Option<String>,
    /// True when there is anything to commit (uncommitted changes present).
    pub dirty: bool,
}

/// Describe a reviewed repo path: its branch, whether it is a linked worktree,
/// the main checkout it belongs to, and whether it has uncommitted changes.
#[tauri::command]
pub async fn review_repo_info(repo_path: String) -> Result<RepoInfo, CommandError> {
    run_blocking(move || review_repo_info_impl(repo_path)).await
}

fn review_repo_info_impl(repo_path: String) -> Result<RepoInfo, CommandError> {
    ensure_work_tree(&repo_path)?;
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"], &repo_path)
        .unwrap_or_default();
    let branch = if branch == "HEAD" { String::new() } else { branch };
    // A linked worktree's `.git` is a FILE (gitdir pointer); the main checkout's
    // is a directory. `--git-common-dir` differs from `--git-dir` for worktrees.
    let git_dir = git(&["rev-parse", "--absolute-git-dir"], &repo_path).unwrap_or_default();
    let common_dir = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"], &repo_path)
        .unwrap_or_default();
    let is_worktree = !git_dir.is_empty() && !common_dir.is_empty() && git_dir != common_dir;
    let main_root = if is_worktree {
        // common_dir is "<main>/.git"; its parent is the main work tree.
        Path::new(&common_dir)
            .parent()
            .map(|p| p.to_string_lossy().replace('/', "\\"))
    } else {
        None
    };
    // Anything to commit? porcelain is empty on a clean tree.
    let status = git(&["status", "--porcelain"], &repo_path).unwrap_or_default();
    Ok(RepoInfo {
        branch,
        is_worktree,
        main_root,
        dirty: !status.trim().is_empty(),
    })
}

/// Stage everything in `worktree_path` and commit it with `message`.
/// Returns the new commit's full SHA. Errors if there is nothing to commit.
#[tauri::command]
pub async fn review_commit(worktree_path: String, message: String) -> Result<String, CommandError> {
    run_blocking(move || review_commit_impl(worktree_path, message)).await
}

fn review_commit_impl(worktree_path: String, message: String) -> Result<String, CommandError> {
    ensure_work_tree(&worktree_path)?;
    let msg = message.trim();
    if msg.is_empty() {
        return Err(CommandError::Failed("commit message is empty".into()));
    }
    let status = git(&["status", "--porcelain"], &worktree_path)?;
    if status.trim().is_empty() {
        return Err(CommandError::Failed("nothing to commit (working tree clean)".into()));
    }
    git(&["add", "-A", "--", "."], &worktree_path)?;
    // `--` separates the flag list from the (here empty) pathspec; message passed
    // as a single arg via the vec — never interpolated into a shell string.
    git(&["commit", "-m", msg], &worktree_path)?;
    let sha = git(&["rev-parse", "HEAD"], &worktree_path)?;
    Ok(sha)
}

/// Merge `branch` into the current branch of `repo_root` with `--no-ff`.
/// On conflict: collect the conflicting files, **abort** the merge (leaving the
/// repo clean), and return a structured error. Never uses force flags.
#[tauri::command]
pub async fn review_merge(repo_root: String, branch: String) -> Result<String, CommandError> {
    run_blocking(move || review_merge_impl(repo_root, branch)).await
}

fn review_merge_impl(repo_root: String, branch: String) -> Result<String, CommandError> {
    ensure_work_tree(&repo_root)?;
    if branch.trim().is_empty() || branch.starts_with('-') {
        return Err(CommandError::Failed(format!("invalid branch: {branch}")));
    }
    let msg = format!("Merge {branch} (bytehive)");
    let mut cmd = Command::new("git");
    cmd.args(["merge", "--no-ff", &branch, "-m", &msg])
        .current_dir(&repo_root);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd
        .output()
        .map_err(|e| CommandError::Failed(format!("git not available: {e}")))?;
    if out.status.success() {
        return git(&["rev-parse", "HEAD"], &repo_root);
    }
    // Failed: detect a conflict, list files cheaply, then abort to restore a
    // clean state. (Any merge in progress is unwound; non-conflict failures —
    // e.g. unknown branch — also get aborted defensively, which is a safe no-op.)
    let conflicts = git(&["diff", "--name-only", "--diff-filter=U"], &repo_root)
        .unwrap_or_default();
    let _ = git(&["merge", "--abort"], &repo_root); // best-effort cleanup
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !conflicts.trim().is_empty() {
        let files: Vec<&str> = conflicts.lines().collect();
        return Err(CommandError::Failed(format!(
            "merge conflict with files: {}",
            files.join(", ")
        )));
    }
    Err(CommandError::Failed(if stderr.is_empty() {
        "merge failed".into()
    } else {
        stderr
    }))
}

/// Discard ALL uncommitted changes in `worktree_path`: revert tracked files
/// (`checkout -- .`) and delete untracked files/dirs (`clean -fd`). Scoped to
/// the given work tree only; committed history is untouched.
#[tauri::command]
pub async fn review_discard(worktree_path: String) -> Result<(), CommandError> {
    run_blocking(move || review_discard_impl(worktree_path)).await
}

fn review_discard_impl(worktree_path: String) -> Result<(), CommandError> {
    ensure_work_tree(&worktree_path)?;
    git(&["checkout", "--", "."], &worktree_path)?;
    git(&["clean", "-fd"], &worktree_path)?;
    Ok(())
}

/// Remove a linked worktree and (optionally) delete its branch. Used after a
/// successful merge or an explicit discard to clean up. NOT forced: a dirty
/// worktree makes `git worktree remove` fail, which surfaces as an error so the
/// user doesn't lose uncommitted work by accident.
#[tauri::command]
pub async fn review_remove_worktree(
    repo_root: String,
    worktree_path: String,
    branch: Option<String>,
) -> Result<(), CommandError> {
    run_blocking(move || review_remove_worktree_impl(repo_root, worktree_path, branch)).await
}

fn review_remove_worktree_impl(
    repo_root: String,
    worktree_path: String,
    branch: Option<String>,
) -> Result<(), CommandError> {
    ensure_work_tree(&repo_root)?;
    git(&["worktree", "remove", &worktree_path], &repo_root)?;
    if let Some(b) = branch {
        if !b.trim().is_empty() && !b.starts_with('-') {
            let _ = git(&["branch", "-D", &b], &repo_root); // best-effort
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo(dir: &Path) {
        let d = dir.to_str().unwrap();
        git(&["init", "-q"], d).unwrap();
        git(&["config", "user.email", "t@t.dev"], d).unwrap();
        git(&["config", "user.name", "t"], d).unwrap();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        git(&["add", "-A"], d).unwrap();
        git(&["commit", "-qm", "init"], d).unwrap();
    }

    #[test]
    fn finds_single_repo_when_dir_is_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("solo");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let got = git_repos_under_impl(repo.to_string_lossy().to_string());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "solo");
    }

    #[test]
    fn finds_sub_repos_under_a_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        for name in ["repoA", "repoB"] {
            let r = parent.join(name);
            std::fs::create_dir(&r).unwrap();
            init_repo(&r);
        }
        std::fs::create_dir(parent.join("plain")).unwrap(); // non-git, ignored
        let got = git_repos_under_impl(parent.to_string_lossy().to_string());
        let names: Vec<_> = got.iter().map(|r| r.name.clone()).collect();
        assert_eq!(names, vec!["repoA".to_string(), "repoB".to_string()]);
    }

    fn repo_at(tmp: &Path) -> String {
        init_repo(tmp);
        git(&["config", "core.autocrlf", "false"], tmp.to_str().unwrap()).unwrap();
        // whatever this machine's own global ignore list says
        git(&["config", "core.excludesFile", "no-such-file"], tmp.to_str().unwrap()).unwrap();
        tmp.to_string_lossy().to_string()
    }

    fn write(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn sorted(files: &[ChangedFile]) -> Vec<String> {
        let mut v: Vec<String> = files.iter().map(|f| f.path.replace('\\', "/")).collect();
        v.sort();
        v
    }

    #[test]
    fn an_agent_s_changes_count_commits_and_new_files_but_not_machine_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        let from = git(&["rev-parse", "HEAD"], &root).unwrap();
        write(tmp.path(), "a.txt", "one\ncommitted\n");
        git(&["commit", "-qam", "agent work"], &root).unwrap();
        write(tmp.path(), "new.txt", "fresh\n");
        write(tmp.path(), ".claude/settings.local.json", "{}\n");
        write(tmp.path(), ".bytehive/fleet.json", "{}\n");
        write(tmp.path(), ".gitignore", "built/\n");
        write(tmp.path(), "built/x.js", "noise\n");
        let files = agent_changes_impl(&root, Some(&from), None).unwrap();
        assert_eq!(sorted(&files), vec![".gitignore", "a.txt", "new.txt"]);
        assert_eq!(
            left_out_of(&root).unwrap(),
            vec![".bytehive/fleet.json", ".claude/settings.local.json"]
        );
        let a = files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!((a.added, a.removed), (Some(1), Some(0)));
    }

    #[test]
    fn work_in_a_folder_that_looks_like_junk_still_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        let work = [".claude/agents/x.md", ".claude/commands/ship.md", "logs/analysis.md", "notes.log"];
        for f in work {
            write(tmp.path(), f, "real work\n");
        }
        assert_eq!(sorted(&agent_changes_impl(&root, None, None).unwrap()), work);
        assert_eq!(agent_diff_impl(&root, None, None).unwrap().matches("+real work").count(), 4);
        assert_eq!(folder_state_impl(&root, None).unwrap().files.len(), 4);
        assert!(left_out_of(&root).unwrap().is_empty());
    }

    #[test]
    fn an_ignored_machine_file_breaks_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        write(tmp.path(), ".gitignore", ".claude/settings.local.json\n");
        write(tmp.path(), ".claude/settings.local.json", "{}\n");
        write(tmp.path(), "new.txt", "fresh\n");
        assert_eq!(sorted(&agent_changes_impl(&root, None, None).unwrap()), vec![".gitignore", "new.txt"]);
        assert!(left_out_of(&root).unwrap().is_empty());
    }

    #[test]
    fn a_name_is_only_the_machine_s_when_it_is_exactly_one_of_them() {
        assert!(is_machine_file(".claude/settings.local.json"));
        for f in [".claude/settings.local.json.bak", ".claude/worktrees/a/b.txt", "x/.claude/settings.local.json", ".bytehive/memory.json"] {
            assert!(!is_machine_file(f), "{f}");
        }
    }

    #[test]
    fn a_file_gone_between_the_list_and_the_add_is_listed_again() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        write(tmp.path(), "here.txt", "fresh\n");
        let gone = vec!["gone.txt".to_string()];
        assert!(add_listed(&root, "-N", &[], &gone).is_err(), "git refuses a name that has gone");
        let mut lists = vec![gone, vec!["here.txt".to_string()]].into_iter();
        twice(|| add_listed(&root, "-N", &[], &lists.next().unwrap())).unwrap();
        assert_eq!(lists.next(), None);
    }

    #[test]
    fn left_out_files_are_told_by_name() {
        assert_eq!(left_out_words(&[]), None);
        assert_eq!(left_out_words(&["a".into()]).unwrap(), "1 file left out: a");
        assert_eq!(left_out_words(&["a".into(), "b".into()]).unwrap(), "2 files left out: a, b");
        let many: Vec<String> = (0..12).map(|i| format!("f{i}")).collect();
        assert_eq!(left_out_words(&many).unwrap(), "12 files left out: f0, f1, f2, f3, f4, f5, f6, f7, f8, f9 and 2 more");
    }

    #[test]
    fn the_row_and_review_read_the_same_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        let from = git(&["rev-parse", "HEAD"], &root).unwrap();
        write(tmp.path(), "a.txt", "one\ntwo\n");
        git(&["commit", "-qam", "agent work"], &root).unwrap();
        write(tmp.path(), "new.txt", "fresh\n");
        write(tmp.path(), ".claude/settings.local.json", "noise\n");
        let files = agent_changes_impl(&root, Some(&from), None).unwrap();
        let text = agent_diff_impl(&root, Some(&from), None).unwrap();
        assert_eq!(text.matches("diff --git").count(), files.len());
        assert!(text.contains("+two") && text.contains("+fresh") && !text.contains("noise"), "{text}");
    }

    #[test]
    fn a_tracked_file_in_a_junk_folder_still_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        write(tmp.path(), ".claude/settings.local.json", "one\n");
        git(&["add", "-A"], &root).unwrap();
        git(&["commit", "-qm", "settings"], &root).unwrap();
        write(tmp.path(), ".claude/settings.local.json", "two\n");
        assert_eq!(sorted(&agent_changes_impl(&root, None, None).unwrap()), vec![".claude/settings.local.json"]);
    }

    #[test]
    fn paths_narrow_the_answer_to_one_agent_s_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        write(tmp.path(), "a.txt", "mine\n");
        write(tmp.path(), "[id].txt", "mine too\n");
        write(tmp.path(), "i.txt", "someone else's\n");
        let mine = vec!["a.txt".to_string(), "[id].txt".to_string()];
        assert_eq!(sorted(&agent_changes_impl(&root, None, Some(&mine)).unwrap()), vec!["[id].txt", "a.txt"]);
        assert!(agent_changes_impl(&root, None, Some(&[])).unwrap().is_empty());
        assert_eq!(agent_diff_impl(&root, None, Some(&[])).unwrap(), "");
    }

    #[test]
    fn looking_at_an_agent_s_changes_leaves_the_index_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        write(tmp.path(), "new.txt", "fresh\n");
        let index = tmp.path().join(".git").join("index");
        let before = std::fs::read(&index).unwrap();
        agent_changes_impl(&root, None, None).unwrap();
        agent_diff_impl(&root, None, None).unwrap();
        folder_state_impl(&root, None).unwrap();
        assert_eq!(std::fs::read(&index).unwrap(), before);
    }

    #[test]
    fn a_folder_s_state_shows_what_moved_and_leaves_junk_out() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        let clean = folder_state_impl(&root, None).unwrap();
        assert!(clean.files.is_empty());
        let head = clean.head.clone().unwrap();
        write(tmp.path(), "a.txt", "one\ntwo\n");
        write(tmp.path(), "new.txt", "fresh\n");
        write(tmp.path(), ".claude/settings.local.json", "noise\n");
        let dirty = folder_state_impl(&root, Some(&head)).unwrap();
        let mut names: Vec<&String> = dirty.files.keys().collect();
        names.sort();
        assert_eq!(names, vec!["a.txt", "new.txt"]);
        assert!(dirty.committed.is_empty());
        write(tmp.path(), "new.txt", "fresher and longer\n");
        let again = folder_state_impl(&root, Some(&head)).unwrap();
        assert_ne!(again.files["new.txt"], dirty.files["new.txt"]);
        assert_eq!(again.files["a.txt"], dirty.files["a.txt"]);
        git(&["commit", "-qam", "agent work"], &root).unwrap();
        let after = folder_state_impl(&root, Some(&head)).unwrap();
        assert_eq!(after.committed, vec!["a.txt"]);
        assert!(!after.files.contains_key("a.txt"));
        assert_ne!(after.head, clean.head);
    }

    #[test]
    fn a_commit_is_not_a_jump_but_a_branch_switch_or_a_reset_is() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo_at(tmp.path());
        let first = git(&["rev-parse", "HEAD"], &root).unwrap();
        write(tmp.path(), "a.txt", "two\n");
        git(&["commit", "-qam", "agent work"], &root).unwrap();
        let second = git(&["rev-parse", "HEAD"], &root).unwrap();
        assert!(!folder_state_impl(&root, Some(&first)).unwrap().jumped);
        git(&["checkout", "-q", "-b", "other", &first], &root).unwrap();
        assert!(folder_state_impl(&root, Some(&second)).unwrap().jumped, "a branch switch");
        git(&["reset", "-q", "--hard", &second], &root).unwrap();
        assert!(folder_state_impl(&root, Some(&first)).unwrap().jumped, "a reset");
    }

    #[test]
    fn a_folder_with_no_repo_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(folder_state_impl(&tmp.path().to_string_lossy(), None).is_err());
    }

    #[test]
    fn repo_diff_shows_working_tree_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("r");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap(); // modify tracked file
        let d = repo_diff_impl(repo.to_string_lossy().to_string()).unwrap();
        assert!(d.contains("a.txt"), "diff should name the file: {d}");
        assert!(d.contains("+two"), "diff should show the added line: {d}");
    }

    #[test]
    fn repo_diff_clean_tree_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("clean");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        assert_eq!(repo_diff_impl(repo.to_string_lossy().to_string()).unwrap(), "");
    }

    #[test]
    fn repo_diff_includes_untracked_files() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("u");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap(); // untracked
        let d = repo_diff_impl(repo.to_string_lossy().to_string()).unwrap();
        assert!(d.contains("new.txt"), "new files must be reviewable: {d}");
        assert!(d.contains("+fresh"), "added content must show: {d}");
    }

    #[test]
    fn looking_at_changes_leaves_the_index_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("ro");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap();
        let index = repo.join(".git").join("index");
        let before = std::fs::read(&index).unwrap();

        let files = git_changed_files_impl(root.clone()).unwrap();
        let d = repo_diff_impl(root.clone()).unwrap();
        assert!(files.iter().any(|f| f.path == "new.txt"), "untracked file listed");
        assert!(d.contains("+fresh"), "diff still shows new files: {d}");

        // Nothing marked intent-to-add, index bytes untouched.
        assert_eq!(git(&["ls-files"], &root).unwrap(), "a.txt");
        assert_eq!(std::fs::read(&index).unwrap(), before, "index must not be rewritten");
        assert!(git(&["status", "--porcelain"], &root).unwrap().contains("?? new.txt"));
    }

    #[test]
    fn looking_at_changes_works_while_the_agent_holds_index_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("locked");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(repo.join("new.txt"), "fresh\n").unwrap();
        let lock = repo.join(".git").join("index.lock");
        std::fs::write(&lock, "").unwrap(); // an agent's `git commit` in flight

        let files = git_changed_files_impl(root.clone()).unwrap();
        assert_eq!(files.len(), 2, "both changes seen: {:?}", files.iter().map(|f| &f.path).collect::<Vec<_>>());
        let d = repo_diff_impl(root).unwrap();
        assert!(d.contains("+two") && d.contains("+fresh"), "diff despite the lock: {d}");
        assert!(lock.exists(), "the agent's lock is not ours to touch");
    }

    #[test]
    fn changed_files_carry_line_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("counts");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("gone.txt"), "x\ny\n").unwrap();
        git(&["add", "-A"], &root).unwrap();
        git(&["commit", "-qm", "two"], &root).unwrap();

        std::fs::write(repo.join("a.txt"), "uno\ntwo\n").unwrap(); // 1 changed + 1 added
        std::fs::remove_file(repo.join("gone.txt")).unwrap(); // 2 removed
        std::fs::create_dir(repo.join("sub")).unwrap();
        std::fs::write(repo.join("sub").join("new.txt"), "1\n2\n3").unwrap(); // 3, last unterminated
        std::fs::write(repo.join("blob.bin"), [0u8, 1, 2, 0]).unwrap(); // binary

        let files = git_changed_files_impl(root).unwrap();
        let get = |p: &str| {
            files
                .iter()
                .find(|f| f.path == p)
                .map(|f| (f.status.as_str(), f.added, f.removed))
                .unwrap_or_else(|| panic!("{p} missing"))
        };
        assert_eq!(get("a.txt"), ("M", Some(2), Some(1)));
        assert_eq!(get("gone.txt"), ("D", Some(0), Some(2)));
        assert_eq!(get("sub\\new.txt"), ("??", Some(3), Some(0)));
        assert_eq!(get("blob.bin"), ("??", Some(0), Some(0)));
        assert_eq!(files.len(), 4);
    }

    // ---- Slice 2: write-side commands ----

    #[test]
    fn review_commit_stages_all_and_returns_sha() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("c");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap(); // modify tracked
        std::fs::write(repo.join("b.txt"), "brand new\n").unwrap(); // untracked

        let sha = review_commit_impl(root.clone(), "bytehive: agent changes".into()).expect("commit");
        assert_eq!(sha.len(), 40, "expected a 40-char sha, got {sha}");
        // Tree is clean after committing everything.
        assert_eq!(git(&["status", "--porcelain"], &root).unwrap(), "");
        // The untracked file made it into the commit.
        let show = git(&["show", "--stat", "HEAD"], &root).unwrap();
        assert!(show.contains("b.txt"), "new file should be in the commit: {show}");
    }

    #[test]
    fn review_commit_errors_on_clean_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("clean2");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let r = review_commit_impl(repo.to_string_lossy().to_string(), "noop".into());
        assert!(r.is_err(), "committing a clean tree must error");
    }

    #[test]
    fn review_merge_no_ff_brings_branch_in() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("m");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        // Create a feature branch with a new file, commit, return to default.
        git(&["checkout", "-q", "-b", "feature"], &root).unwrap();
        std::fs::write(repo.join("feat.txt"), "feature\n").unwrap();
        git(&["add", "-A"], &root).unwrap();
        git(&["commit", "-qm", "feat"], &root).unwrap();
        // Back to the original branch (whatever `init` produced).
        git(&["checkout", "-q", "-"], &root).unwrap();
        assert!(!repo.join("feat.txt").exists(), "feat file should be branch-only pre-merge");

        let sha = review_merge_impl(root.clone(), "feature".into()).expect("merge");
        assert_eq!(sha.len(), 40);
        assert!(repo.join("feat.txt").exists(), "merge should bring the feature file in");
        // --no-ff → the merge commit has two parents.
        let parents = git(&["rev-list", "--parents", "-n", "1", "HEAD"], &root).unwrap();
        assert_eq!(parents.split_whitespace().count(), 3, "merge commit must have 2 parents: {parents}");
    }

    #[test]
    fn review_merge_conflict_aborts_and_reports_files() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("conf");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        // Branch edits a.txt one way…
        git(&["checkout", "-q", "-b", "feat"], &root).unwrap();
        std::fs::write(repo.join("a.txt"), "branch change\n").unwrap();
        git(&["commit", "-qam", "branch edit"], &root).unwrap();
        // …default edits the same line differently → conflict on merge.
        git(&["checkout", "-q", "-"], &root).unwrap();
        std::fs::write(repo.join("a.txt"), "main change\n").unwrap();
        git(&["commit", "-qam", "main edit"], &root).unwrap();

        let err = review_merge_impl(root.clone(), "feat".into()).unwrap_err();
        let CommandError::Failed(msg) = err else {
            panic!("expected Failed, got {err:?}");
        };
        assert!(msg.contains("conflict"), "should report a conflict: {msg}");
        assert!(msg.contains("a.txt"), "should name the conflicting file: {msg}");
        // Merge was aborted → tree is clean, no merge in progress.
        assert_eq!(git(&["status", "--porcelain"], &root).unwrap(), "");
    }

    #[test]
    fn review_discard_reverts_and_cleans() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("d");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("a.txt"), "dirtied\n").unwrap(); // modify tracked
        std::fs::write(repo.join("junk.txt"), "junk\n").unwrap(); // untracked

        review_discard_impl(root.clone()).expect("discard");
        // Line endings may be normalised to CRLF by core.autocrlf on Windows;
        // compare on trimmed content so the test is platform-agnostic.
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap().replace("\r\n", "\n"),
            "one\n"
        );
        assert!(!repo.join("junk.txt").exists(), "untracked file should be removed");
        assert_eq!(git(&["status", "--porcelain"], &root).unwrap(), "");
    }

    #[test]
    fn review_repo_info_flags_clean_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("info");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let info = review_repo_info_impl(repo.to_string_lossy().to_string()).unwrap();
        assert!(!info.is_worktree, "a plain checkout is not a worktree");
        assert!(info.main_root.is_none());
        assert!(!info.dirty, "freshly committed tree is clean");
        assert!(!info.branch.is_empty(), "should report a branch name");
    }

    #[test]
    fn review_repo_info_detects_worktree_and_merge_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("wt-main");
        std::fs::create_dir(&repo).unwrap();
        init_repo(&repo);
        let root = repo.to_string_lossy().to_string();
        let wt = tmp.path().join("wt-linked");
        let wt_str = wt.to_string_lossy().to_string();
        git(&["worktree", "add", "-b", "agent-x", &wt_str, "HEAD"], &root).expect("add worktree");

        // The linked worktree is detected as such, pointing back at the main root.
        let info = review_repo_info_impl(wt_str.clone()).unwrap();
        assert!(info.is_worktree, "linked worktree should be flagged");
        assert_eq!(info.branch, "agent-x");
        let main = info.main_root.expect("worktree should know its main root");
        assert!(
            std::path::Path::new(&main).join(".git").exists(),
            "main_root should point at the main checkout: {main}"
        );

        // Make a change in the worktree, commit it, then merge into main and clean up.
        std::fs::write(wt.join("w.txt"), "from agent\n").unwrap();
        review_commit_impl(wt_str.clone(), "bytehive: agent-x changes".into()).expect("commit in wt");
        review_merge_impl(main.clone(), "agent-x".into()).expect("merge into main");
        assert!(
            std::path::Path::new(&main).join("w.txt").exists(),
            "merged file should appear in main"
        );
        review_remove_worktree_impl(main.clone(), wt_str.clone(), Some("agent-x".into()))
            .expect("remove worktree + branch");
        assert!(!wt.exists(), "worktree dir should be gone");
    }
}
