//! The Changes panel's model, pure: diff lines numbered from their hunk
//! header (old and new line numbers, like GitHub), and which files you have
//! marked viewed. A viewed mark remembers what the file's diff looked like;
//! when the agent changes that file again the mark drops, so "viewed" never
//! hides new work.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use supervise::diff_file::{DiffFile, DiffHunk, HunkKind};

/// One painted row of a file's diff.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffRow {
    Hunk(String),
    Line { old: Option<u32>, new: Option<u32>, kind: HunkKind, text: String },
}

/// Old and new start lines from `@@ -a,b +c,d @@`; (1, 1) when unreadable.
pub fn hunk_starts(header: &str) -> (u32, u32) {
    let num = |tag: char| -> Option<u32> {
        let at = header.find(tag)?;
        let digits: String = header[at + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    };
    (num('-').unwrap_or(1), num('+').unwrap_or(1))
}

/// Every row of a file: each hunk's header, then its lines numbered.
pub fn rows(file: &DiffFile) -> Vec<DiffRow> {
    let mut out = Vec::new();
    for h in &file.hunks {
        out.push(DiffRow::Hunk(h.header.clone()));
        number_hunk(h, &mut out);
    }
    out
}

fn number_hunk(h: &DiffHunk, out: &mut Vec<DiffRow>) {
    let (mut old, mut new) = hunk_starts(&h.header);
    for l in &h.lines {
        let (o, n) = match l.kind {
            HunkKind::Ctx => (Some(old), Some(new)),
            HunkKind::Del => (Some(old), None),
            HunkKind::Add => (None, Some(new)),
        };
        if o.is_some() {
            old += 1;
        }
        if n.is_some() {
            new += 1;
        }
        out.push(DiffRow::Line { old: o, new: n, kind: l.kind, text: l.text.clone() });
    }
}

/// A new file git does not track yet, as an all-added diff, so review shows
/// it like any other change. None for binary text (a NUL byte).
pub fn untracked_file(path: &str, text: &str) -> Option<DiffFile> {
    if text.contains('\0') {
        return None;
    }
    let lines: Vec<supervise::diff_file::HunkLine> = text.lines().map(|l| supervise::diff_file::HunkLine { kind: HunkKind::Add, text: l.to_string() }).collect();
    let n = lines.len() as u32;
    let hunks = if n == 0 { Vec::new() } else { vec![DiffHunk { header: format!("@@ -0,0 +1,{n} @@"), lines }] };
    Some(DiffFile { path: path.to_string(), additions: n, deletions: 0, hunks, binary: false })
}

/// What a file's diff looks like, to tell when it changed since viewed.
pub fn fingerprint(file: &DiffFile) -> u64 {
    let mut h = DefaultHasher::new();
    file.path.hash(&mut h);
    for hunk in &file.hunks {
        hunk.header.hash(&mut h);
        for l in &hunk.lines {
            (l.kind as u8).hash(&mut h);
            l.text.hash(&mut h);
        }
    }
    h.finish()
}

/// Viewed marks per agent pane, per file path.
#[derive(Default)]
pub struct Viewed {
    marks: HashMap<String, HashMap<String, u64>>,
}

impl Viewed {
    pub fn is_viewed(&self, pane: &str, file: &DiffFile) -> bool {
        self.marks.get(pane).and_then(|m| m.get(&file.path)) == Some(&fingerprint(file))
    }

    pub fn set(&mut self, pane: &str, file: &DiffFile, on: bool) {
        let m = self.marks.entry(pane.to_string()).or_default();
        if on {
            m.insert(file.path.clone(), fingerprint(file));
        } else {
            m.remove(&file.path);
        }
    }

    pub fn count(&self, pane: &str, files: &[DiffFile]) -> usize {
        files.iter().filter(|f| self.is_viewed(pane, f)).count()
    }

    /// The first file not yet viewed after `from`, wrapping; none when all are.
    pub fn next_unviewed(&self, pane: &str, files: &[DiffFile], from: usize) -> Option<usize> {
        let n = files.len();
        (1..=n).map(|k| (from + k) % n).find(|&i| !self.is_viewed(pane, &files[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supervise::diff_file::parse_diff;

    const DIFF: &str = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -10,3 +10,3 @@ fn f()\n keep\n-old\n+new\n tail\ndiff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1,2 @@\n x\n+y\n";

    #[test]
    fn reads_hunk_starts() {
        assert_eq!(hunk_starts("@@ -10,3 +12,4 @@ fn f()"), (10, 12));
        assert_eq!(hunk_starts("@@ -1 +1,2 @@"), (1, 1));
        assert_eq!(hunk_starts("garbage"), (1, 1));
    }

    #[test]
    fn numbers_old_and_new_lines() {
        let files = parse_diff(DIFF);
        let r = rows(&files[0]);
        assert_eq!(r[0], DiffRow::Hunk("@@ -10,3 +10,3 @@".into()));
        let nums: Vec<(Option<u32>, Option<u32>)> = r[1..]
            .iter()
            .map(|x| match x {
                DiffRow::Line { old, new, .. } => (*old, *new),
                DiffRow::Hunk(_) => (None, None),
            })
            .collect();
        assert_eq!(nums, [(Some(10), Some(10)), (Some(11), None), (None, Some(11)), (Some(12), Some(12))]);
    }

    #[test]
    fn a_new_file_reads_as_all_added() {
        let f = untracked_file("src/new.rs", "fn a() {}\nfn b() {}\n").unwrap();
        assert_eq!((f.additions, f.deletions), (2, 0));
        let r = rows(&f);
        assert_eq!(r[0], DiffRow::Hunk("@@ -0,0 +1,2 @@".into()));
        assert!(matches!(&r[2], DiffRow::Line { old: None, new: Some(2), .. }));
        assert!(untracked_file("a.png", "PNG\0\0").is_none());
    }

    #[test]
    fn viewed_drops_when_the_file_changes_again() {
        let files = parse_diff(DIFF);
        let mut v = Viewed::default();
        v.set("p", &files[0], true);
        assert!(v.is_viewed("p", &files[0]));
        assert!(!v.is_viewed("other", &files[0]), "per pane");
        assert_eq!(v.count("p", &files), 1);
        assert_eq!(v.next_unviewed("p", &files, 0), Some(1));
        let changed = parse_diff(&DIFF.replace("+new", "+newer"));
        assert!(!v.is_viewed("p", &changed[0]), "new work clears the mark");
        v.set("p", &files[1], true);
        assert_eq!(v.next_unviewed("p", &files, 0), None);
    }
}
