//! The files on a branch, for the Files drawer: every file git knows (and
//! untracked ones it does not ignore), plus what each changed file did, as
//! one letter: A added, M modified, D deleted. Best effort like `git`: a
//! folder that is not a repo reads as empty.

use std::path::Path;

use crate::git::git;

/// One changed file and what happened to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatus {
    pub path: String,
    /// 'A' added (or untracked), 'M' modified (or renamed), 'D' deleted.
    pub mark: char,
}

/// Read `git status --porcelain=v1 -z` output into marks. Pure.
pub fn parse_status(out: &str) -> Vec<FileStatus> {
    let mut files = Vec::new();
    let mut chunks = out.split('\0');
    while let Some(entry) = chunks.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = entry.split_at(3);
        let (x, y) = (xy.as_bytes()[0], xy.as_bytes()[1]);
        // A rename's second NUL part is the old path; skip it.
        if x == b'R' || y == b'R' {
            let _ = chunks.next();
        }
        let mark = if x == b'D' || y == b'D' {
            'D'
        } else if x == b'A' || xy.starts_with("??") {
            'A'
        } else {
            'M'
        };
        let path = path.trim_start();
        let path = path.strip_prefix('"').and_then(|p| p.strip_suffix('"')).unwrap_or(path);
        if !path.is_empty() {
            files.push(FileStatus { path: path.to_string(), mark });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    files
}

/// What changed under `cwd`, one mark per file, relative to `cwd` like
/// [`listed_files`] (git status itself speaks from the repo root).
pub fn file_statuses(cwd: &Path) -> Vec<FileStatus> {
    let Some(out) = git(cwd, &["status", "--porcelain=v1", "--untracked-files=all", "-z", "--", "."]) else {
        return Vec::new();
    };
    let prefix = git(cwd, &["rev-parse", "--show-prefix"]).map(|p| p.trim().to_string()).unwrap_or_default();
    parse_status(&out)
        .into_iter()
        .filter_map(|f| f.path.strip_prefix(&prefix).map(|p| FileStatus { path: p.to_string(), mark: f.mark }))
        .collect()
}

/// Every file on the branch: tracked ones and untracked ones git does not
/// ignore, repo-relative with `/`, sorted.
pub fn listed_files(cwd: &Path) -> Vec<String> {
    let Some(out) = git(cwd, &["ls-files", "--cached", "--others", "--exclude-standard", "-z"]) else {
        return Vec::new();
    };
    let mut v: Vec<String> = out.split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_each_kind_of_change() {
        let out = " M src/a.rs\0A  src/new.rs\0?? notes.md\0 D gone.rs\0R  b.rs\0old_b.rs\0";
        let got: Vec<(String, char)> = parse_status(out).into_iter().map(|f| (f.path, f.mark)).collect();
        assert_eq!(
            got,
            [("b.rs".into(), 'M'), ("gone.rs".into(), 'D'), ("notes.md".into(), 'A'), ("src/a.rs".into(), 'M'), ("src/new.rs".into(), 'A')]
        );
    }

    #[test]
    fn a_folder_outside_a_repo_lists_nothing() {
        let dir = std::env::temp_dir().join("bytehive-surely-no-repo-files");
        assert!(listed_files(&dir).is_empty());
        assert!(file_statuses(&dir).is_empty());
    }
}
