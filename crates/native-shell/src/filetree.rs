//! The Files drawer's tree, pure: from a branch's flat file list to the
//! rows on screen. Folders come before files at each level, a folder's
//! children show only while it is open, and a search drops the tree for a
//! flat list of matching paths. Changed files carry their git mark; a folder
//! holding changes is marked too, so a closed folder still says "look here".

use std::collections::{HashMap, HashSet};

/// Most rows a search lists; past that, narrow the search.
pub const SEARCH_MAX: usize = 400;

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Repo-relative path with `/`; a folder's has no trailing slash.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub dir: bool,
    pub open: bool,
    /// 'A' / 'M' / 'D' for a changed file, '*' for a folder holding changes.
    pub mark: Option<char>,
}

/// Sort key: folders before files at every level, then by name.
fn tree_key(path: &str) -> Vec<(u8, String)> {
    let parts: Vec<&str> = path.split('/').collect();
    parts.iter().enumerate().map(|(i, p)| (if i + 1 < parts.len() { 0 } else { 1 }, p.to_lowercase())).collect()
}

/// The rows to paint. `files` may hold deleted paths too (from `marks`).
pub fn visible_rows(files: &[String], marks: &HashMap<String, char>, open: &HashSet<String>, query: &str) -> Vec<Row> {
    let mut all: Vec<&str> = files.iter().map(String::as_str).collect();
    for p in marks.keys() {
        if !files.contains(p) {
            all.push(p);
        }
    }
    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        let mut hits: Vec<&str> = all.into_iter().filter(|p| p.to_lowercase().contains(&q)).collect();
        hits.sort();
        hits.truncate(SEARCH_MAX);
        return hits
            .into_iter()
            .map(|p| Row { path: p.to_string(), name: p.to_string(), depth: 0, dir: false, open: false, mark: marks.get(p).copied() })
            .collect();
    }
    all.sort_by_key(|p| tree_key(p));
    all.dedup();
    let changed_dirs: HashSet<String> = marks
        .keys()
        .flat_map(|p| {
            let parts: Vec<&str> = p.split('/').collect();
            (1..parts.len()).map(move |i| parts[..i].join("/"))
        })
        .collect();
    let mut rows = Vec::new();
    let mut emitted: HashSet<String> = HashSet::new();
    for p in all {
        let parts: Vec<&str> = p.split('/').collect();
        let mut visible = true;
        for i in 1..parts.len() {
            let dir = parts[..i].join("/");
            if !emitted.contains(&dir) {
                emitted.insert(dir.clone());
                let is_open = open.contains(&dir);
                let mark = changed_dirs.contains(&dir).then_some('*');
                rows.push(Row { path: dir.clone(), name: parts[i - 1].to_string(), depth: i - 1, dir: true, open: is_open, mark });
            }
            if !open.contains(&dir) {
                visible = false;
                break;
            }
        }
        if visible {
            let name = parts.last().copied().unwrap_or(p).to_string();
            rows.push(Row { path: p.to_string(), name, depth: parts.len() - 1, dir: false, open: false, mark: marks.get(p).copied() });
        }
    }
    // A closed folder's sub-folders were emitted while walking; keep only
    // rows whose ancestors are all open.
    rows.retain(|r| {
        let parts: Vec<&str> = r.path.split('/').collect();
        (1..parts.len()).all(|i| open.contains(&parts[..i].join("/")))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<String> {
        ["README.md", "src/main.rs", "src/ui/app.rs", "src-tauri/lib.rs", "Cargo.toml"].iter().map(|s| s.to_string()).collect()
    }

    fn names(rows: &[Row]) -> Vec<String> {
        rows.iter().map(|r| format!("{}{}{}", "  ".repeat(r.depth), r.name, if r.dir { "/" } else { "" })).collect()
    }

    #[test]
    fn folders_first_and_closed_by_default() {
        let rows = visible_rows(&files(), &HashMap::new(), &HashSet::new(), "");
        assert_eq!(names(&rows), ["src/", "src-tauri/", "Cargo.toml", "README.md"]);
    }

    #[test]
    fn opening_a_folder_shows_its_children() {
        let open: HashSet<String> = ["src".to_string()].into();
        let rows = visible_rows(&files(), &HashMap::new(), &open, "");
        assert_eq!(names(&rows), ["src/", "  ui/", "  main.rs", "src-tauri/", "Cargo.toml", "README.md"]);
        let open: HashSet<String> = ["src".to_string(), "src/ui".to_string()].into();
        let rows = visible_rows(&files(), &HashMap::new(), &open, "");
        assert!(names(&rows).contains(&"    app.rs".to_string()));
    }

    #[test]
    fn changes_mark_files_and_their_folders() {
        let marks: HashMap<String, char> = [("src/ui/app.rs".to_string(), 'M'), ("src/gone.rs".to_string(), 'D')].into();
        let rows = visible_rows(&files(), &marks, &HashSet::new(), "");
        assert_eq!(rows[0].mark, Some('*'), "closed src/ still says it holds changes");
        let open: HashSet<String> = ["src".to_string()].into();
        let rows = visible_rows(&files(), &marks, &open, "");
        let gone = rows.iter().find(|r| r.path == "src/gone.rs").expect("a deleted file still shows");
        assert_eq!(gone.mark, Some('D'));
    }

    #[test]
    fn a_search_lists_matching_paths_flat() {
        let rows = visible_rows(&files(), &HashMap::new(), &HashSet::new(), "APP");
        assert_eq!(names(&rows), ["src/ui/app.rs"]);
        assert!(visible_rows(&files(), &HashMap::new(), &HashSet::new(), "zzz").is_empty());
    }
}
