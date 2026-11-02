//! Which of a folder's changed files are an agent's own work. Git only says
//! what is dirty in the folder; a file that was already dirty when the agent
//! started is not something it did. A file counts once it changed after the
//! agent started, or once it shows up dirty when it was clean at the agent's
//! first look.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// What one git poll learned about a folder: its changed paths (repo
/// relative, as git prints them) and each one's modified time in ms, 0 when
/// the file is gone.
#[derive(Clone, Default)]
pub struct FolderRead {
    pub files: Vec<String>,
    pub mtimes: Vec<u64>,
}

/// Read a folder's changed files and their times. Runs off the UI thread.
pub fn read(cwd: &Path) -> FolderRead {
    let files = agent_core::git::changed_files(cwd);
    let root = agent_core::git::repo_root(cwd).map(std::path::PathBuf::from).unwrap_or_else(|| cwd.to_path_buf());
    let mtimes = files
        .iter()
        .map(|f| {
            std::fs::metadata(root.join(f.trim_end_matches('/')))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64)
        })
        .collect();
    FolderRead { files, mtimes }
}

/// The files already dirty before an agent that started at `started` (ms):
/// changed earlier than that, or gone without a time to tell.
pub fn baseline(read: &FolderRead, started: u64) -> HashSet<String> {
    read.files.iter().zip(&read.mtimes).filter(|(_, t)| **t < started).map(|(f, _)| f.clone()).collect()
}

/// The agent's own files: changed since it started, or dirty now while
/// clean at its first look. With no baseline yet only the time can tell.
pub fn touched(read: &FolderRead, started: u64, base: Option<&HashSet<String>>) -> Vec<String> {
    read.files
        .iter()
        .zip(&read.mtimes)
        .filter(|(f, t)| **t >= started || base.is_some_and(|b| !b.contains(*f)))
        .map(|(f, _)| f.clone())
        .collect()
}

/// Baselines per pane id, taken the first time a folder's read arrives
/// after the pane started.
pub fn fill_baselines<'a>(bases: &mut HashMap<String, HashSet<String>>, read: &FolderRead, panes: impl Iterator<Item = (&'a str, u64)>) {
    for (id, started) in panes {
        bases.entry(id.to_string()).or_insert_with(|| baseline(read, started));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read() -> FolderRead {
        FolderRead { files: vec!["old.rs".into(), "gone.json".into(), "new.rs".into()], mtimes: vec![100, 0, 900] }
    }

    #[test]
    fn work_from_before_the_start_is_not_the_agents() {
        let base = baseline(&read(), 500);
        assert_eq!(base, HashSet::from(["old.rs".to_string(), "gone.json".to_string()]));
        assert_eq!(touched(&read(), 500, Some(&base)), vec!["new.rs".to_string()]);
    }

    #[test]
    fn an_old_file_edited_again_counts() {
        let base = baseline(&read(), 500);
        let mut now = read();
        now.mtimes[0] = 700;
        assert_eq!(touched(&now, 500, Some(&base)), vec!["old.rs".to_string(), "new.rs".to_string()]);
    }

    #[test]
    fn a_file_deleted_after_the_first_look_counts() {
        let first = FolderRead { files: vec!["old.rs".into()], mtimes: vec![100] };
        let base = baseline(&first, 500);
        let now = FolderRead { files: vec!["old.rs".into(), "a.ts".into()], mtimes: vec![100, 0] };
        assert_eq!(touched(&now, 500, Some(&base)), vec!["a.ts".to_string()]);
    }

    #[test]
    fn without_a_baseline_only_the_time_counts() {
        assert_eq!(touched(&read(), 500, None), vec!["new.rs".to_string()]);
    }
}
