//! What Review types into an agent when it sends the work back, the comments
//! you leave on its diff until then, and the line numbers the diff shows
//! (reviewtext.ts in today's app). No git and no UI here.

use super::diff::DiffFile;

/// What a diff line does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Add,
    Del,
    Ctx,
}

/// A diff line with the numbers it has on each side: `old` for a removed or
/// unchanged line, `new` for an added or unchanged one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberedLine {
    pub kind: Kind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    /// The line without its leading '+', '-' or ' '.
    pub text: String,
}

/// The start lines of `@@ -a,b +c,d @@`: (a, c).
fn hunk_starts(header: &str) -> Option<(u32, u32)> {
    // One side: digits, then maybe a comma and more digits.
    fn start(s: &str) -> Option<u32> {
        let (a, b) = s.split_once(',').unwrap_or((s, "0"));
        let digits = |t: &str| !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit());
        if digits(a) && digits(b) { a.parse().ok() } else { None }
    }
    let rest = header.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('+')?;
    let (new, rest) = rest.split_once(' ')?;
    if !rest.starts_with("@@") {
        return None;
    }
    Some((start(old)?, start(new)?))
}

/// Number a hunk's lines (as diff::Hunk keeps them) from its header.
pub fn number_hunk(header: &str, lines: &[String]) -> Vec<NumberedLine> {
    let (mut o, mut n) = hunk_starts(header).unwrap_or((1, 1));
    lines
        .iter()
        .map(|l| {
            let text = l.get(1..).unwrap_or("").to_string();
            match l.chars().next() {
                Some('+') => {
                    n += 1;
                    NumberedLine { kind: Kind::Add, old: None, new: Some(n - 1), text }
                }
                Some('-') => {
                    o += 1;
                    NumberedLine { kind: Kind::Del, old: Some(o - 1), new: None, text }
                }
                _ => {
                    o += 1;
                    n += 1;
                    NumberedLine { kind: Kind::Ctx, old: Some(o - 1), new: Some(n - 1), text }
                }
            }
        })
        .collect()
}

/// Lines added and removed across a diff: (add, del).
pub fn diff_totals(files: &[DiffFile]) -> (usize, usize) {
    files.iter().fold((0, 0), |(a, d), f| (a + f.additions, d + f.deletions))
}

/// Which numbers a comment's lines count: the new file's, or the old one's
/// for lines the agent removed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    New,
    Old,
}

/// A comment on one line, or a run of lines, of the diff.
#[derive(Clone, Debug)]
pub struct LineComment {
    pub id: String,
    pub path: String,
    pub side: Side,
    pub start: u32,
    pub end: u32,
    /// The quoted line(s) as they were when you wrote it.
    pub code: Vec<String>,
    pub text: String,
}

/// "src/a.ts:12", "src/a.ts:12-14", "src/a.ts:7 (removed)".
pub fn comment_where(path: &str, side: Side, start: u32, end: u32) -> String {
    let at = if start == end { format!("{start}") } else { format!("{start}-{end}") };
    format!("{path}:{at}{}", if side == Side::Old { " (removed)" } else { "" })
}

/// A merge that would conflict: which branch, into what, in which files.
#[derive(Clone, Debug)]
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
        "Merging your branch {branch} into {base} conflicts in:\n{files}\n\n\
         Nothing was merged; {base} is unchanged. Merge {base} into your branch (git merge {base}), \
         resolve the conflicts in those files, check that it still builds and the tests pass, and commit. \
         Then tell me it is ready to merge again.",
        branch = c.branch,
        base = c.base,
    )
}

/// Each comment as the agent reads it: where, the quoted code, what to do.
pub fn comments_text(list: &[LineComment]) -> String {
    let mut sorted: Vec<&LineComment> = list.iter().collect();
    // localeCompare in the old app: case folded first, then as written.
    sorted.sort_by(|a, b| {
        a.path.to_lowercase().cmp(&b.path.to_lowercase()).then_with(|| a.path.cmp(&b.path)).then(a.start.cmp(&b.start))
    });
    let each: Vec<String> = sorted
        .iter()
        .map(|c| {
            let code = c.code.iter().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n");
            format!("{}\n{code}\n{}", comment_where(&c.path, c.side, c.start, c.end), c.text.trim())
        })
        .collect();
    format!("Review comments on your changes:\n\n{}", each.join("\n\n"))
}

/// The one message Send back types: the conflict to resolve, your comments on
/// the diff, then your note. Empty when there is nothing to send.
pub fn send_back_text(note: &str, comments: &[LineComment], conflict: Option<&Conflict>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = conflict {
        parts.push(conflict_text(c));
    }
    let comments: Vec<LineComment> = comments.iter().filter(|c| !c.text.trim().is_empty()).cloned().collect();
    if !comments.is_empty() {
        parts.push(comments_text(&comments));
    }
    let note = note.trim();
    if !note.is_empty() {
        parts.push(note.to_string());
    }
    if !comments.is_empty() {
        let which = if comments.len() == 1 { "this comment" } else { "each comment" };
        parts.push(format!("Address {which}, then tell me when it is done."));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::super::diff::DiffFile;
    use super::{comment_where, conflict_text, diff_totals, number_hunk, send_back_text, Conflict, LineComment, Side};

    fn conflict() -> Conflict {
        Conflict { base: "main".into(), branch: "bytehive/eli-1a2b3c".into(), files: vec!["src/a.ts".into(), "src/b.ts".into()] }
    }

    #[test]
    fn lists_the_conflicting_files_and_asks_the_agent_to_merge_the_base_in() {
        let t = conflict_text(&conflict());
        assert!(t.contains("Merging your branch bytehive/eli-1a2b3c into main conflicts in:\n- src/a.ts\n- src/b.ts"), "{t}");
        assert!(t.contains("git merge main"));
        assert!(t.contains("resolve the conflicts"));
        assert!(!t.contains('\u{2014}'));
    }

    #[test]
    fn types_one_message_conflict_first_then_the_note_nothing_when_empty() {
        assert_eq!(send_back_text("  add a test  ", &[], None), "add a test");
        let c = conflict();
        let both = send_back_text("and keep the old name", &[], Some(&c));
        assert!(both.starts_with("Merging your branch"));
        assert!(both.ends_with("\n\nand keep the old name"));
        assert_eq!(send_back_text("   ", &[], None), "");
        assert_eq!(send_back_text("", &[], None), "");
    }

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn numbers_each_side_from_the_hunk_header() {
        let got = number_hunk("@@ -10,3 +10,4 @@ fn x()", &lines(&[" a", "-b", "+B", "+C", " d"]));
        let nums: Vec<(Option<u32>, Option<u32>)> = got.iter().map(|l| (l.old, l.new)).collect();
        assert_eq!(nums, vec![(Some(10), Some(10)), (Some(11), None), (None, Some(11)), (None, Some(12)), (Some(12), Some(13))]);
        assert_eq!(got[2].text, "B");
    }

    #[test]
    fn counts_a_new_file_from_line_one_and_totals_a_diff() {
        assert_eq!(number_hunk("@@ -0,0 +1 @@", &lines(&["+x"]))[0].new, Some(1));
        let f = |path: &str, additions, deletions| DiffFile { path: path.into(), additions, deletions, ..Default::default() };
        assert_eq!(diff_totals(&[f("a", 3, 1), f("b", 2, 0)]), (5, 1));
    }

    fn c(path: &str, side: Side, start: u32, end: u32, code: &[&str], text: &str) -> LineComment {
        LineComment { id: "x".into(), path: path.into(), side, start, end, code: lines(code), text: text.into() }
    }

    #[test]
    fn says_where_a_comment_is() {
        assert_eq!(comment_where("src/a.ts", Side::New, 12, 12), "src/a.ts:12");
        assert_eq!(comment_where("src/a.ts", Side::New, 12, 14), "src/a.ts:12-14");
        assert_eq!(comment_where("src/a.ts", Side::Old, 7, 7), "src/a.ts:7 (removed)");
    }

    #[test]
    fn types_every_comment_in_one_message_then_what_to_do() {
        let t = send_back_text(
            "Also run the linter.",
            &[
                c("src/b.ts", Side::New, 3, 4, &["if (a)", "  go();"], "Guard b too"),
                c("src/a.ts", Side::New, 12, 12, &["const x = 1;"], "Rename x to count"),
                c("src/a.ts", Side::New, 12, 12, &["const x = 1;"], "   "), // an empty one is not sent
            ],
            None,
        );
        let want = [
            "Review comments on your changes:",
            "",
            "src/a.ts:12\n> const x = 1;\nRename x to count",
            "",
            "src/b.ts:3-4\n> if (a)\n>   go();\nGuard b too",
            "",
            "Also run the linter.",
            "",
            "Address each comment, then tell me when it is done.",
        ]
        .join("\n");
        assert_eq!(t, want);
        assert!(!t.contains('\u{2014}'));
        let one = send_back_text("", &[c("a.rs", Side::Old, 1, 1, &["x"], "why?")], None);
        assert!(one.ends_with("Address this comment, then tell me when it is done."), "{one}");
    }
}
