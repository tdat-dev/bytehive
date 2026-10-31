//! Open projects: the folders agents work in, several at once, like the
//! approved Agent Inbox ("You can open more than one; the queue shows every
//! project together"). A chip row filters the Queue and the grid to one
//! project or shows them all. The list and the filter survive restarts in
//! `~/.maestro/native-projects.json`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::term_pane::Pane;

#[derive(Debug, Default, PartialEq)]
pub struct Projects {
    /// Folders in the order they were opened.
    pub dirs: Vec<String>,
    /// The project shown, or none for all of them.
    pub filter: Option<String>,
}

/// One spelling per folder: no trailing separator, Windows separators on
/// Windows, so `D:/x/` and `D:\x` are the same project.
pub fn normalize(dir: &str) -> String {
    let mut d = dir.trim().to_string();
    if cfg!(windows) {
        d = d.replace('/', "\\");
    }
    while d.len() > 3 && (d.ends_with('\\') || d.ends_with('/')) {
        d.pop();
    }
    d
}

fn same(a: &str, b: &str) -> bool {
    if cfg!(windows) { a.eq_ignore_ascii_case(b) } else { a == b }
}

/// True when `dir` is `root` or sits inside it.
pub fn is_within(dir: &str, root: &str) -> bool {
    let (d, r) = (normalize(dir), normalize(root));
    if same(&d, &r) {
        return true;
    }
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let prefix = if r.ends_with(sep) { r.clone() } else { format!("{r}{sep}") };
    d.len() > prefix.len() && same(&d[..prefix.len()], &prefix)
}

/// A project's short name: the folder's last part.
pub fn project_name(dir: &str) -> String {
    let d = normalize(dir);
    d.rsplit(['\\', '/']).find(|s| !s.is_empty()).unwrap_or(&d).to_string()
}

/// The folder that decides a pane's project: the repo it branched from when
/// it works in a worktree, else where it runs.
pub fn pane_home(pane: &Pane) -> String {
    match &pane.worktree {
        Some(w) => w.repo.clone(),
        None => crate::states::pane_cwd(pane),
    }
}

impl Projects {
    /// Open `dir` (once); returns its normalized spelling.
    pub fn open(&mut self, dir: &str) -> String {
        let d = normalize(dir);
        if !d.is_empty() && !self.dirs.iter().any(|x| same(x, &d)) {
            self.dirs.push(d.clone());
        }
        d
    }

    /// Close a project. Its agents keep running and show under all projects.
    pub fn close(&mut self, dir: &str) {
        self.dirs.retain(|x| !same(x, dir));
        if self.filter.as_deref().is_some_and(|f| same(f, dir)) {
            self.filter = None;
        }
    }

    /// The open project a folder belongs to: the deepest one holding it.
    pub fn owner(&self, dir: &str) -> Option<&str> {
        self.dirs.iter().filter(|p| is_within(dir, p)).max_by_key(|p| p.len()).map(String::as_str)
    }

    /// Whether the current filter shows an agent working from `home`.
    pub fn shows(&self, home: &str) -> bool {
        match &self.filter {
            None => true,
            Some(f) => self.owner(home).is_some_and(|o| same(o, f)),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({ "projects": self.dirs, "filter": self.filter })
    }

    pub fn from_json(v: &Value) -> Projects {
        let mut p = Projects::default();
        for d in v["projects"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            p.open(d);
        }
        p.filter = v["filter"].as_str().map(normalize).filter(|f| p.dirs.iter().any(|d| same(d, f)));
        p
    }

    /// Read the saved list; a missing or broken file is an empty list.
    pub fn load(path: &Path) -> Projects {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).map(|v| Projects::from_json(&v)).unwrap_or_default()
    }

    /// Best effort: a failed write only loses the list for next time.
    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, serde_json::to_string_pretty(&self.to_json()).unwrap_or_default());
    }
}

/// Where the list lives: `~/.maestro/native-projects.json`.
pub fn store_path() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".maestro").join("native-projects.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_spelling_per_folder() {
        let mut p = Projects::default();
        p.open("D:\\maestro");
        if cfg!(windows) {
            p.open("d:/maestro/");
        }
        assert_eq!(p.dirs.len(), 1);
        assert_eq!(project_name("D:\\work\\ByteWaker\\"), "ByteWaker");
    }

    #[test]
    fn a_folder_belongs_to_the_deepest_open_project() {
        let mut p = Projects::default();
        p.open("D:\\work");
        p.open("D:\\work\\app");
        assert_eq!(p.owner("D:\\work\\app\\src"), Some("D:\\work\\app"));
        assert_eq!(p.owner("D:\\work\\other"), Some("D:\\work"));
        assert_eq!(p.owner("D:\\workshop"), None, "a shared prefix is not a parent");
        assert_eq!(p.owner("E:\\x"), None);
    }

    #[test]
    fn the_filter_shows_one_project_or_all() {
        let mut p = Projects::default();
        p.open("D:\\a");
        p.open("D:\\b");
        assert!(p.shows("D:\\a\\src") && p.shows("E:\\elsewhere"));
        p.filter = Some("D:\\a".into());
        assert!(p.shows("D:\\a\\src"));
        assert!(!p.shows("D:\\b"));
        assert!(!p.shows("E:\\elsewhere"), "unowned agents only show under all");
        p.close("D:\\a");
        assert_eq!(p.filter, None, "closing the shown project shows all");
    }

    #[test]
    fn survives_a_round_trip_and_drops_a_stale_filter() {
        let mut p = Projects::default();
        p.open("D:\\a");
        p.open("D:\\b");
        p.filter = Some("D:\\b".into());
        let path = std::env::temp_dir().join(format!("maestro-projects-{}.json", std::process::id()));
        p.save(&path);
        assert_eq!(Projects::load(&path), p);
        std::fs::remove_file(&path).ok();
        let stale = Projects::from_json(&json!({ "projects": ["D:\\a"], "filter": "D:\\gone" }));
        assert_eq!(stale.filter, None);
        assert_eq!(Projects::load(Path::new("Z:\\no\\such.json")), Projects::default());
    }
}
