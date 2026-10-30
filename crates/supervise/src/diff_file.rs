//! Pure unified-diff parser: raw `git diff` text to structured
//! files/hunks/lines for read-only rendering. No git, no IO.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HunkKind {
    Ctx,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HunkLine {
    pub kind: HunkKind,
    /// Line content without the leading +/-/space.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    /// The @@ ... @@ line.
    pub header: String,
    pub lines: Vec<HunkLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffFile {
    /// New path (b/...), or old path for deletions.
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub hunks: Vec<DiffHunk>,
    /// Git reported "Binary files differ" (no text hunks).
    pub binary: bool,
}

pub fn parse_diff(raw: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut file: Option<usize> = None;
    let mut hunk: Option<(usize, usize)> = None;
    for line in raw.split('\n') {
        if line.starts_with("diff --git") {
            // Path from the header itself, so binary/mode-only files still get a name
            // even when there's no `+++ b/` line. The `+++`/`---` branches refine it.
            let mut path = String::new();
            if let Some(rest) = line.strip_prefix("diff --git a/") {
                if let Some((_, b)) = rest.rsplit_once(" b/") {
                    path = b.to_string();
                }
            }
            files.push(DiffFile { path, additions: 0, deletions: 0, hunks: Vec::new(), binary: false });
            file = Some(files.len() - 1);
            hunk = None;
        } else if let Some(fi) = file {
            if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
                files[fi].binary = true;
            } else if let Some(p) = line.strip_prefix("+++ ") {
                let p = p.strip_prefix("b/").unwrap_or(p);
                if p != "/dev/null" {
                    files[fi].path = p.to_string();
                }
            } else if let Some(p) = line.strip_prefix("--- ") {
                let p = p.strip_prefix("a/").unwrap_or(p);
                if files[fi].path.is_empty() && p != "/dev/null" {
                    files[fi].path = p.to_string();
                }
            } else if line.starts_with("@@") {
                let header = match line[2..].find("@@") {
                    Some(end) => line[..end + 4].to_string(),
                    None => line.to_string(),
                };
                files[fi].hunks.push(DiffHunk { header, lines: Vec::new() });
                hunk = Some((fi, files[fi].hunks.len() - 1));
            } else if let Some((hfi, hi)) = hunk {
                if hfi == fi {
                    let kind = match line.chars().next() {
                        Some('+') => Some(HunkKind::Add),
                        Some('-') => Some(HunkKind::Del),
                        Some(' ') => Some(HunkKind::Ctx),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        match kind {
                            HunkKind::Add => files[fi].additions += 1,
                            HunkKind::Del => files[fi].deletions += 1,
                            HunkKind::Ctx => {}
                        }
                        files[fi].hunks[hi].lines.push(HunkLine { kind, text: line[1..].to_string() });
                    }
                }
            }
        }
    }
    files
}
