//! The patch format Codex applies: one entry per file it adds, updates or deletes.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::MAX_DIFF_LINES;
use crate::types::{DiffLine, DiffSign};

/// One file in an apply_patch: what happened to it and its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchFile {
    pub path: String,
    pub kind: PatchKind,
    pub diff: Vec<DiffLine>,
    pub added: u32,
    pub removed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchKind {
    Add,
    Update,
    Delete,
}

fn patch_head_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\*\*\* (Add|Update|Delete) File: (.+)$").unwrap())
}

fn patch_move_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\*\*\* Move to: (.+)$").unwrap())
}

/// Codex's patch format ("*** Begin Patch", "*** Add File: …", "@@", ±lines).
pub fn parse_patch(input: &str) -> Vec<PatchFile> {
    let mut out: Vec<PatchFile> = Vec::new();
    let mut cur: Option<usize> = None;
    for line in input.split('\n') {
        let raw = line.strip_suffix('\r').unwrap_or(line);
        if raw.is_empty() {
            continue;
        }
        if let Some(m) = patch_head_re().captures(raw) {
            let kind = match &m[1] {
                "Add" => PatchKind::Add,
                "Delete" => PatchKind::Delete,
                _ => PatchKind::Update,
            };
            out.push(PatchFile { path: m[2].trim().to_string(), kind, diff: Vec::new(), added: 0, removed: 0 });
            cur = Some(out.len() - 1);
            continue;
        }
        if let Some(m) = patch_move_re().captures(raw) {
            if let Some(i) = cur {
                out[i].path = m[1].trim().to_string();
            }
            continue;
        }
        let Some(i) = cur else { continue };
        if raw.starts_with("***") {
            continue;
        }
        if raw.starts_with("@@") {
            if !out[i].diff.is_empty() && out[i].diff.len() < MAX_DIFF_LINES {
                out[i].diff.push(DiffLine { sign: DiffSign::Same, text: "\u{22ef}".to_string() });
            }
            continue;
        }
        let sign = raw.chars().next().unwrap_or(' ');
        let text = raw.get(sign.len_utf8()..).unwrap_or("").to_string();
        match sign {
            '+' => {
                out[i].added += 1;
                if out[i].diff.len() < MAX_DIFF_LINES {
                    out[i].diff.push(DiffLine { sign: DiffSign::Added, text });
                }
            }
            '-' => {
                out[i].removed += 1;
                if out[i].diff.len() < MAX_DIFF_LINES {
                    out[i].diff.push(DiffLine { sign: DiffSign::Removed, text });
                }
            }
            ' ' => {
                if out[i].diff.len() < MAX_DIFF_LINES {
                    out[i].diff.push(DiffLine { sign: DiffSign::Same, text });
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_patch_reads_add_update_delete() {
        let files = parse_patch("*** Begin Patch\n*** Add File: D:/app/new.ts\n+export const a = 1;\n+export const b = 2;\n*** Update File: D:/app/old.ts\n@@ function x\n keep\n-gone\n+here\n@@\n+more\n*** Delete File: D:/app/dead.ts\n*** End Patch");
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "D:/app/new.ts");
        assert_eq!(files[0].kind, PatchKind::Add);
        assert_eq!((files[0].added, files[0].removed), (2, 0));
        assert_eq!(files[1].kind, PatchKind::Update);
        assert_eq!((files[1].added, files[1].removed), (2, 1));
        let signs: Vec<String> = files[1]
            .diff
            .iter()
            .map(|d| {
                let sign = match d.sign {
                    DiffSign::Same => " ",
                    DiffSign::Added => "+",
                    DiffSign::Removed => "-",
                };
                format!("{sign}{}", d.text)
            })
            .collect();
        assert_eq!(signs, vec![" keep", "-gone", "+here", " \u{22ef}", "+more"]);
        assert_eq!(files[2].kind, PatchKind::Delete);
    }

}
