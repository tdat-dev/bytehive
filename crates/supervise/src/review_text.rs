//! What Review types into an agent when it sends the work back, the comments
//! left on its diff until then, the line numbers the diff shows, and which
//! files deserve a first look. No IO, so it stays unit-testable.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::diff_file::{DiffFile, HunkKind, HunkLine};

/// A merge that would conflict: which branch, into what, in which files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub base: String,
    pub branch: String,
    pub files: Vec<String>,
}

/// The prepared message for a conflict: the files, and what to do about them.
pub fn conflict_text(c: &Conflict) -> String {
    let files = if c.files.is_empty() {
        "- (git did not name the files)".to_string()
    } else {
        c.files.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
    };
    format!(
        "Merging your branch {} into {} conflicts in:\n{files}\n\nNothing was merged; {} is unchanged. Merge {} into your branch (git merge {}), resolve the conflicts in those files, check that it still builds and the tests pass, and commit. Then tell me it is ready to merge again.",
        c.branch, c.base, c.base, c.base, c.base
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentSide {
    New,
    Old,
}

/// A comment on one line, or a run of lines, of the diff. `side` says which
/// numbers `start`..`end` count: the new file's, or the old one's for lines
/// the agent removed. `code` is the quoted line(s) as they were when written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineComment {
    pub id: String,
    pub path: String,
    pub side: CommentSide,
    pub start: u32,
    pub end: u32,
    pub code: Vec<String>,
    pub text: String,
}

/// "src/a.ts:12", "src/a.ts:12-14", "src/a.ts:7 (removed)".
pub fn comment_where(path: &str, side: CommentSide, start: u32, end: u32) -> String {
    let at = if start == end { format!("{start}") } else { format!("{start}-{end}") };
    format!("{path}:{at}{}", if side == CommentSide::Old { " (removed)" } else { "" })
}

/// Each comment as the agent reads it: where, the quoted code, what to do.
pub fn comments_text(list: &[LineComment]) -> String {
    let mut sorted: Vec<&LineComment> = list.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path).then(a.start.cmp(&b.start)));
    let body: Vec<String> = sorted
        .iter()
        .map(|c| {
            let quoted = c.code.iter().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n");
            format!("{}\n{quoted}\n{}", comment_where(&c.path, c.side, c.start, c.end), c.text.trim())
        })
        .collect();
    format!("Review comments on your changes:\n\n{}", body.join("\n\n"))
}

/// The one message Send back types: the conflict to resolve, your comments on
/// the diff, then your note. Empty when there is nothing to send.
pub fn send_back_text(note: Option<&str>, conflict: Option<&Conflict>, comments: &[LineComment]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = conflict {
        parts.push(conflict_text(c));
    }
    let kept: Vec<&LineComment> = comments.iter().filter(|c| !c.text.trim().is_empty()).collect();
    if !kept.is_empty() {
        parts.push(comments_text(&kept.into_iter().cloned().collect::<Vec<_>>()));
    }
    if let Some(note) = note.map(str::trim).filter(|n| !n.is_empty()) {
        parts.push(note.to_string());
    }
    let n = comments.iter().filter(|c| !c.text.trim().is_empty()).count();
    if n > 0 {
        parts.push(format!("Address {}, then tell me when it is done.", if n == 1 { "this comment" } else { "each comment" }));
    }
    parts.join("\n\n")
}

/// Drafts kept per agent while the app runs, until Send back delivers them.
#[derive(Debug, Default)]
pub struct CommentDrafts {
    drafts: HashMap<String, Vec<LineComment>>,
    next_id: u64,
}

#[derive(Debug, Clone)]
pub struct NewComment {
    pub id: Option<String>,
    pub path: String,
    pub side: CommentSide,
    pub start: u32,
    pub end: u32,
    pub code: Vec<String>,
    pub text: String,
}

impl CommentDrafts {
    pub fn new() -> CommentDrafts {
        CommentDrafts { drafts: HashMap::new(), next_id: 1 }
    }

    /// The comments waiting to go to this agent, in the order written.
    pub fn comments_of(&self, agent_id: &str) -> Vec<LineComment> {
        self.drafts.get(agent_id).cloned().unwrap_or_default()
    }

    /// Add a comment, or replace the one with the same id. Returns it.
    pub fn put_comment(&mut self, agent_id: &str, c: NewComment) -> LineComment {
        let mut list = self.comments_of(agent_id);
        let id = c.id.unwrap_or_else(|| {
            let id = format!("c{}", self.next_id);
            self.next_id += 1;
            id
        });
        let saved = LineComment { id, path: c.path, side: c.side, start: c.start, end: c.end, code: c.code, text: c.text };
        match list.iter().position(|x| x.id == saved.id) {
            Some(i) => list[i] = saved.clone(),
            None => list.push(saved.clone()),
        }
        self.drafts.insert(agent_id.to_string(), list);
        saved
    }

    pub fn drop_comment(&mut self, agent_id: &str, id: &str) {
        let list: Vec<LineComment> = self.comments_of(agent_id).into_iter().filter(|c| c.id != id).collect();
        if list.is_empty() {
            self.drafts.remove(agent_id);
        } else {
            self.drafts.insert(agent_id.to_string(), list);
        }
    }

    pub fn clear_comments(&mut self, agent_id: &str) {
        self.drafts.remove(agent_id);
    }
}

/// A diff line with the numbers it has on each side: `old` for a removed or
/// unchanged line, `new` for an added or unchanged one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberedLine {
    pub kind: HunkKind,
    pub text: String,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

fn hunk_head_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").unwrap())
}

/// Number each hunk's lines from its `@@ -a,b +c,d @@` header.
pub fn number_hunk(header: &str, lines: &[HunkLine]) -> Vec<NumberedLine> {
    let (mut o, mut n) = match hunk_head_re().captures(header) {
        Some(m) => (m[1].parse().unwrap_or(1), m[2].parse().unwrap_or(1)),
        None => (1, 1),
    };
    lines
        .iter()
        .map(|l| match l.kind {
            HunkKind::Add => {
                let out = NumberedLine { kind: l.kind, text: l.text.clone(), old: None, new: Some(n) };
                n += 1;
                out
            }
            HunkKind::Del => {
                let out = NumberedLine { kind: l.kind, text: l.text.clone(), old: Some(o), new: None };
                o += 1;
                out
            }
            HunkKind::Ctx => {
                let out = NumberedLine { kind: l.kind, text: l.text.clone(), old: Some(o), new: Some(n) };
                o += 1;
                n += 1;
                out
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffTotals {
    pub add: u32,
    pub del: u32,
}

/// Lines added and removed across a diff.
pub fn diff_totals(files: &[DiffFile]) -> DiffTotals {
    files.iter().fold(DiffTotals { add: 0, del: 0 }, |mut t, f| {
        t.add += f.additions;
        t.del += f.deletions;
        t
    })
}

fn risk_secret_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(^|/)\.env(\.|$)|\.pem$|\.key$|id_rsa|credentials|secrets?\.").unwrap())
}

fn risk_auth_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(auth|login|session|password|passwd|permission|acl)\b").unwrap())
}

fn risk_config_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"package\.json$|-lock\.(json|yaml)$|\.lock$|requirements[^/]*\.txt$|pyproject\.toml$|cargo\.toml$|cargo\.lock$|go\.(mod|sum)$|dockerfile$|migrations?[/_]|schema|\.sql$|\.ya?ml$|\.toml$|\.ini$|\.cfg$").unwrap()
    })
}

fn risk_test_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(^|/)(test|tests|__tests__|spec|e2e)/|\.(test|spec)\.|_test\.|test_").unwrap())
}

fn risk_docs_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.(md|mdx|rst|txt)$|(^|/)docs?/").unwrap())
}

/// Review risk of a path, higher first: secrets and auth before tests and docs.
pub fn risk_of(path: &str) -> u32 {
    let p = path.to_lowercase();
    if risk_secret_re().is_match(&p) {
        return 100;
    }
    if risk_auth_re().is_match(&p) {
        return 80;
    }
    if risk_config_re().is_match(&p) {
        return 60;
    }
    if risk_test_re().is_match(&p) {
        return 20;
    }
    if risk_docs_re().is_match(&p) {
        return 10;
    }
    40
}

/// Files riskiest first; a stable sort keeps the diff order inside a tier.
pub fn rank_files<T: Clone>(files: &[T], path_of: impl Fn(&T) -> &str) -> Vec<T> {
    let mut out = files.to_vec();
    out.sort_by(|a, b| risk_of(path_of(b)).cmp(&risk_of(path_of(a))));
    out
}
