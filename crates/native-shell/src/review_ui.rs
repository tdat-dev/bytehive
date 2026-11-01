//! Review panel: what changed in an agent's folder, what the diff itself
//! admits is unfinished, and the secrets or auth code to look at. Git reads
//! run on background threads; the UI only ever polls finished snapshots.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use agent_core::git::{branch_diff_text, branch_files, changed_files, checkpoint, current_branch, diff_text, repo_root};

use crate::land_ui::{LandBox, LandOut};
use crate::diffview::untracked_file;
use eframe::egui;
use supervise::diff_file::{parse_diff, DiffFile};
use supervise::done_check::{describe_done, describe_secrets, scan_done_diff, scan_secrets, DoneFinding, SecretFinding};

use crate::term_pane::{Pane, WorktreeInfo};
use crate::theme::{ADD, DEL, DIM, FAINT};

/// Everything Review shows for one folder, read in one go off the UI thread.
pub struct ReviewSnapshot {
    pub branch: Option<String>,
    pub changed: Vec<String>,
    pub files: Vec<DiffFile>,
    pub done: Vec<DoneFinding>,
    pub secrets: Vec<SecretFinding>,
}

impl ReviewSnapshot {
    /// The pure shaping: findings out of a diff. IO stays in `load_snapshot`.
    #[cfg(test)]
    pub fn from_parts(branch: Option<String>, changed: Vec<String>, diff: &str) -> ReviewSnapshot {
        Self::from_files(branch, changed, parse_diff(diff))
    }

    pub fn from_files(branch: Option<String>, changed: Vec<String>, files: Vec<DiffFile>) -> ReviewSnapshot {
        let done = scan_done_diff(&files);
        let secrets = scan_secrets(&files);
        ReviewSnapshot { branch, changed, files, done, secrets }
    }
}

/// The repos right under a folder that is not one itself (a parent of
/// repos), sorted by name.
pub fn repos_under(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.join(".git").exists()).collect();
    v.sort();
    v
}

pub fn load_snapshot(cwd: &PathBuf) -> ReviewSnapshot {
    if repo_root(cwd).is_none() {
        // A folder of repos: each one's changes, under its own folder name.
        let (mut changed, mut files) = (Vec::new(), Vec::new());
        for r in repos_under(cwd) {
            let name = r.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let snap = load_snapshot(&r);
            changed.extend(snap.changed.into_iter().map(|c| format!("{name}/{c}")));
            files.extend(snap.files.into_iter().map(|mut f| {
                f.path = format!("{name}/{}", f.path);
                f
            }));
        }
        return ReviewSnapshot::from_files(None, changed, files);
    }
    let branch = current_branch(cwd);
    let changed = changed_files(cwd);
    let mut files = parse_diff(&diff_text(cwd));
    let root = repo_root(cwd).map(PathBuf::from).unwrap_or_else(|| cwd.clone());
    add_untracked(&root, &changed, &mut files);
    ReviewSnapshot::from_files(branch, changed, files)
}

/// New files git does not track yet join the diff as all-added, so review
/// reads them like any change. Big (over 512 KB) or binary files stay out.
fn add_untracked(root: &Path, changed: &[String], files: &mut Vec<DiffFile>) {
    for c in changed {
        if files.iter().any(|f| &f.path == c) {
            continue;
        }
        let path = root.join(c);
        let small = std::fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() <= 512 * 1024);
        if let Some(f) = small.then(|| std::fs::read_to_string(&path).ok()).flatten().and_then(|t| untracked_file(c, &t)) {
            files.push(f);
        }
    }
}

/// What review reads for a worktree pane: the branch against its base, plus
/// whatever is still uncommitted in the worktree.
pub fn load_worktree_snapshot(w: &WorktreeInfo) -> ReviewSnapshot {
    let repo = PathBuf::from(&w.repo);
    let wt = PathBuf::from(&w.path);
    let mut changed = branch_files(&repo, &w.base, &w.branch);
    for f in changed_files(&wt) {
        if !changed.contains(&f) {
            changed.push(f);
        }
    }
    let mut diff = branch_diff_text(&repo, &w.base, &w.branch);
    let uncommitted = diff_text(&wt);
    if !uncommitted.is_empty() {
        if !diff.is_empty() && !diff.ends_with('\n') {
            diff.push('\n');
        }
        diff.push_str(&uncommitted);
    }
    let mut files = parse_diff(&diff);
    add_untracked(&wt, &changed, &mut files);
    ReviewSnapshot::from_files(Some(w.branch.clone()), changed, files)
}

enum ReviewMsg {
    Snapshot(String, ReviewSnapshot),
    Checkpoint(Option<String>),
}

pub struct ReviewPanel {
    selected: Option<String>,
    snapshot: Option<ReviewSnapshot>,
    loading: bool,
    checkpoint_msg: Option<String>,
    /// Merge, Send back, Discard for an agent in its own worktree.
    land: LandBox,
    /// What a branch action asks the app to do; the app takes it.
    pub land_out: Option<LandOut>,
    tx: Sender<ReviewMsg>,
    rx: Receiver<ReviewMsg>,
    /// A file picked to read in the Changes panel; the app takes it.
    pub open_changes: Option<usize>,
}

impl ReviewPanel {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        ReviewPanel { selected: None, snapshot: None, loading: false, checkpoint_msg: None, land: LandBox::default(), land_out: None, tx, rx, open_changes: None }
    }

    /// The pane under review and its last read diff, for the Changes panel.
    pub fn current(&self) -> (Option<&str>, Option<&ReviewSnapshot>) {
        (self.selected.as_deref(), self.snapshot.as_ref())
    }

    /// Review this pane (the Changes panel follows).
    pub fn select(&mut self, pane_id: &str) {
        if self.selected.as_deref() != Some(pane_id) {
            self.selected = Some(pane_id.to_string());
            self.snapshot = None;
            self.checkpoint_msg = None;
        }
    }

    fn cwd_of(panes: &[Pane], pane_id: &str) -> Option<PathBuf> {
        panes.iter().find(|p| p.id == pane_id).map(|p| match p.spec.cwd.clone() {
            Some(cwd) => PathBuf::from(cwd),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        })
    }

    fn request_load(&mut self, pane_id: &str, panes: &[Pane]) {
        let Some(cwd) = Self::cwd_of(panes, pane_id) else { return };
        let wt = panes.iter().find(|p| p.id == pane_id).and_then(|p| p.worktree.clone());
        self.loading = true;
        let tx = self.tx.clone();
        let id = pane_id.to_string();
        std::thread::spawn(move || {
            let snap = match wt {
                Some(w) => load_worktree_snapshot(&w),
                None => load_snapshot(&cwd),
            };
            let _ = tx.send(ReviewMsg::Snapshot(id, snap));
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, panes: &mut [Pane]) {
        ui.label(egui::RichText::new("Review").strong().size(15.0));
        if panes.is_empty() {
            ui.label(egui::RichText::new("No agents yet.").color(FAINT));
            return;
        }
        if self.selected.as_ref().is_none_or(|id| panes.iter().all(|p| &p.id != id)) {
            self.selected = Some(panes[0].id.clone());
            self.snapshot = None;
        }
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            for p in panes.iter() {
                let sel = self.selected.as_deref() == Some(p.id.as_str());
                if ui.selectable_label(sel, &p.name).clicked() {
                    self.selected = Some(p.id.clone());
                    self.snapshot = None;
                    self.checkpoint_msg = None;
                            changed = true;
                }
            }
        });
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ReviewMsg::Snapshot(id, snap) => {
                    if self.selected.as_deref() == Some(id.as_str()) {
                        self.snapshot = Some(snap);
                        self.loading = false;
                    }
                }
                ReviewMsg::Checkpoint(sha) => {
                    self.checkpoint_msg = Some(match sha {
                        Some(s) => format!("Checkpoint kept: {}", s.chars().take(8).collect::<String>()),
                        None => "Nothing to checkpoint.".to_string(),
                    });
                }
            }
        }
        let pane_id = self.selected.clone().unwrap_or_default();
        if changed || (self.snapshot.is_none() && !self.loading) {
            self.request_load(&pane_id, panes);
        }
        ui.horizontal(|ui| {
            if ui.small_button("Refresh").clicked() {
                self.request_load(&pane_id, panes);
            }
            if ui.small_button("Checkpoint").clicked() {
                if let Some(cwd) = Self::cwd_of(panes, &pane_id) {
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let sha = checkpoint(&cwd, "maestro review checkpoint");
                        let _ = tx.send(ReviewMsg::Checkpoint(sha));
                    });
                }
            }
            if let Some(msg) = self.checkpoint_msg.as_deref() {
                ui.label(egui::RichText::new(msg).small().color(DIM));
            }
        });
        let out = panes.iter().find(|p| Some(p.id.as_str()) == self.selected.as_deref()).and_then(|p| self.land.show(ui, p));
        match out {
            Some(LandOut::Merged(id)) => {
                if let Some(p) = panes.iter_mut().find(|p| p.id == id) {
                    p.merged = true;
                }
                self.snapshot = None;
                self.request_load(&id, panes);
            }
            Some(o) => self.land_out = Some(o),
            None => {}
        }
        ui.separator();
        if self.loading && self.snapshot.is_none() {
            ui.label(egui::RichText::new("Reading the diff…").color(FAINT));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
            return;
        }
        let Some(snap) = self.snapshot.as_ref() else { return };
        if self.loading {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        let branch = snap.branch.as_deref().unwrap_or("(no branch)");
        ui.label(egui::RichText::new(format!("{} · {} file{} changed", branch, snap.changed.len(), if snap.changed.len() == 1 { "" } else { "s" })).strong());
        let done_line = describe_done(&snap.done);
        ui.label(egui::RichText::new(if done_line.is_empty() { "Done gate: clean".to_string() } else { format!("Done gate: {done_line}") }).small());
        let secrets_line = describe_secrets(&snap.secrets);
        ui.label(
            egui::RichText::new(if secrets_line.is_empty() { "Secrets: clean".to_string() } else { format!("Secrets: {secrets_line}") })
                .small()
                .color(if secrets_line.is_empty() { DIM } else { DEL }),
        );
        ui.separator();
        if !snap.files.is_empty() && ui.button("Open changes").on_hover_text("The full diff, file by file (V marks viewed)").clicked() {
            self.open_changes = Some(0);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, f) in snap.files.iter().enumerate() {
                ui.horizontal(|ui| {
                    let name = egui::Label::new(egui::RichText::new(&f.path).monospace().small()).truncate().sense(egui::Sense::click());
                    if ui.add(name).on_hover_text(format!("{}\nClick to read its diff", f.path)).clicked() {
                        self.open_changes = Some(i);
                    }
                    if f.additions > 0 {
                        ui.label(egui::RichText::new(format!("+{}", f.additions)).monospace().small().color(ADD));
                    }
                    if f.deletions > 0 {
                        ui.label(egui::RichText::new(format!("-{}", f.deletions)).monospace().small().color(DEL));
                    }
                });
            }
            let untracked: Vec<&String> = snap.changed.iter().filter(|c| !snap.files.iter().any(|f| &f.path == *c)).collect();
            if !untracked.is_empty() {
                ui.label(egui::RichText::new("Untracked (not in the diff):").small().strong());
                for u in untracked {
                    ui.label(egui::RichText::new(format!("? {u}")).monospace().small().color(DIM));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1,2 @@\n const x = 1;\n+// TODO: handle empty\n";

    #[test]
    fn snapshot_shapes_findings_from_a_diff() {
        let snap = ReviewSnapshot::from_parts(Some("feat/x".to_string()), vec!["src/a.ts".to_string()], DIFF);
        assert_eq!(snap.branch.as_deref(), Some("feat/x"));
        assert_eq!(snap.files.len(), 1);
        assert_eq!(snap.done.len(), 1);
        assert_eq!(snap.done[0].text, "// TODO: handle empty");
        assert!(snap.secrets.is_empty());
        assert_eq!(describe_done(&snap.done), "1 TODO in src/a.ts");
    }

    #[test]
    fn headers_read_adds_and_dels_off_the_parse() {
        let snap = ReviewSnapshot::from_parts(Some("feat/x".to_string()), vec!["src/a.ts".to_string()], DIFF);
        assert_eq!(snap.files[0].additions, 1);
        assert_eq!(snap.files[0].deletions, 0);
    }

    #[test]
    fn empty_diff_is_a_clean_snapshot() {
        let snap = ReviewSnapshot::from_parts(None, vec![], "");
        assert!(snap.files.is_empty());
        assert!(snap.done.is_empty());
        assert!(snap.secrets.is_empty());
    }

    #[test]
    fn a_folder_of_repos_reviews_each_under_its_name() {
        let parent = std::env::temp_dir().join(format!("maestro-parent-review-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        let repo = scratch_repo("inner");
        std::fs::create_dir_all(&parent).unwrap();
        let inner = parent.join("api");
        std::fs::rename(&repo, &inner).unwrap();
        std::fs::write(inner.join("note.txt"), "hello\nworld\n").unwrap();
        let snap = load_snapshot(&parent);
        assert_eq!(snap.changed, vec!["api/note.txt".to_string()]);
        assert_eq!(snap.files.first().map(|f| f.path.as_str()), Some("api/note.txt"));
        let _ = std::fs::remove_dir_all(&parent);
    }

    fn scratch_repo(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("maestro-wt-review-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let run = |args: &[&str], cwd: &Path| {
            let out = std::process::Command::new("git").arg("-C").arg(cwd).args(args).output().unwrap();
            assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        run(&["init", "--quiet"], &root);
        run(&["config", "user.email", "t@t"], &root);
        run(&["config", "user.name", "t"], &root);
        run(&["config", "commit.gpgsign", "false"], &root);
        std::fs::write(root.join("note.txt"), "hello\n").unwrap();
        run(&["add", "."], &root);
        run(&["commit", "--quiet", "-m", "init"], &root);
        root
    }

    #[test]
    fn worktree_snapshot_reads_the_branch_and_merges_it_back() {
        use agent_core::git::{branch_files, create_worktree, current_branch};
        let repo = scratch_repo("snap");
        let base = current_branch(&repo).unwrap();
        let wt_path = repo.parent().unwrap().join(format!("maestro-wt-pane-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&wt_path);
        assert!(create_worktree(&repo, &wt_path, "feat/wt", "HEAD").is_some());
        std::fs::write(wt_path.join("note.txt"), "hello\n// TODO: polish\n").unwrap();
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git").arg("-C").arg(&wt_path).args(args).output().unwrap();
            assert!(out.status.success(), "{args:?}");
        };
        run(&["add", "."]);
        run(&["commit", "--quiet", "-m", "work"]);
        let info = WorktreeInfo { repo: repo.to_string_lossy().into_owned(), branch: "feat/wt".to_string(), base: base.clone(), path: wt_path.to_string_lossy().into_owned() };
        let snap = load_worktree_snapshot(&info);
        assert_eq!(snap.branch.as_deref(), Some("feat/wt"));
        assert!(snap.changed.iter().any(|c| c == "note.txt"), "changed: {:?}", snap.changed);
        assert!(!snap.files.is_empty());
        assert_eq!(snap.done.len(), 1);
        assert!(!branch_files(&repo, &base, "feat/wt").is_empty());
        use agent_core::git::{merge_branch, remove_worktree, MergeOutcome};
        assert_eq!(merge_branch(&repo, "feat/wt", &base), MergeOutcome::Merged);
        assert!(remove_worktree(&repo, &wt_path));
        let _ = std::fs::remove_dir_all(&repo);
    }
}
