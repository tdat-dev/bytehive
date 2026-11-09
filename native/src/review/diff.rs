//! Raw `git diff` text into files and hunks for the Changes drawer (diff.ts in
//! today's app). No git here, just the text.

/// One file of a diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffFile {
    /// The new path (b/...), or the old one for a deleted file.
    pub path: String,
    /// Where it came from when it was renamed; None otherwise.
    pub old_path: Option<String>,
    pub additions: usize,
    pub deletions: usize,
    /// git said "Binary files differ": no text hunks.
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

/// One `@@ ... @@` block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hunk {
    /// The `@@ -a,b +c,d @@` part of the line, without the function name after it.
    pub header: String,
    /// Its lines as git wrote them, each with its leading '+', '-' or ' '
    /// (that first character is the kind; text::number_hunk reads it).
    pub lines: Vec<String>,
}

/// Every file in a unified diff, in order. Empty text gives none.
pub fn parse_diff(raw: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    // Whether the last file has a hunk open that lines go into.
    let mut in_hunk = false;
    for line in raw.split('\n') {
        if line.starts_with("diff --git") {
            // The path from the header itself, so a binary or mode-only file
            // still has a name without a `+++ b/` line. The lines below refine it.
            let path = header_path(line).unwrap_or_default();
            files.push(DiffFile { path, ..Default::default() });
            in_hunk = false;
            continue;
        }
        let Some(file) = files.last_mut() else { continue };
        if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
            file.binary = true;
        } else if let Some(p) = line.strip_prefix("+++ ") {
            let p = p.strip_prefix("b/").unwrap_or(p);
            if p != "/dev/null" {
                file.path = p.to_string();
            }
        } else if let Some(p) = line.strip_prefix("--- ") {
            let p = p.strip_prefix("a/").unwrap_or(p);
            if p != "/dev/null" {
                if file.path.is_empty() {
                    file.path = p.to_string(); // a deletion keeps its old path
                }
                file.old_path = Some(p.to_string());
            }
        } else if let Some(p) = line.strip_prefix("rename from ") {
            file.old_path = Some(p.to_string());
        } else if line.starts_with("@@") {
            let header = match line[2..].find("@@") {
                Some(i) => &line[..i + 4],
                None => line,
            };
            file.hunks.push(Hunk { header: header.to_string(), lines: Vec::new() });
            in_hunk = true;
        } else if in_hunk && (line.starts_with('+') || line.starts_with('-') || line.starts_with(' ')) {
            if line.starts_with('+') {
                file.additions += 1;
            } else if line.starts_with('-') {
                file.deletions += 1;
            }
            if let Some(h) = file.hunks.last_mut() {
                h.lines.push(line.to_string());
            }
        }
    }
    // Only a path that really changed counts as a rename.
    for f in &mut files {
        if f.old_path.as_deref() == Some(f.path.as_str()) {
            f.old_path = None;
        }
    }
    files
}

/// The b/ path of `diff --git a/<x> b/<y>`, taking the last " b/" that
/// leaves both sides non-empty (diff.ts's greedy regex does the same).
fn header_path(line: &str) -> Option<String> {
    let rest = line.strip_prefix("diff --git a/")?;
    rest.rmatch_indices(" b/")
        .find(|(i, _)| *i > 0 && rest.len() > i + 3)
        .map(|(i, _)| rest[i + 3..].to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_diff;

    const SAMPLE: &str = "diff --git a/src/app.ts b/src/app.ts
index 111..222 100644
--- a/src/app.ts
+++ b/src/app.ts
@@ -1,3 +1,4 @@
 const a = 1;
-const b = 2;
+const b = 3;
+const c = 4;
 const d = 5;
";

    #[test]
    fn returns_one_file_with_its_path_and_counts() {
        let files = parse_diff(SAMPLE);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/app.ts");
        assert_eq!(files[0].old_path, None);
        assert_eq!(files[0].additions, 2);
        assert_eq!(files[0].deletions, 1);
    }

    #[test]
    fn parses_hunk_lines_with_kinds() {
        let h = &parse_diff(SAMPLE)[0].hunks[0];
        assert_eq!(h.header, "@@ -1,3 +1,4 @@");
        let kinds: Vec<char> = h.lines.iter().map(|l| l.chars().next().unwrap()).collect();
        assert_eq!(kinds, vec![' ', '-', '+', '+', ' ']);
        assert_eq!(&h.lines[2][1..], "const b = 3;");
    }

    #[test]
    fn returns_nothing_for_an_empty_diff() {
        assert!(parse_diff("").is_empty());
    }

    #[test]
    fn names_binary_files_from_the_header_and_flags_them() {
        let raw = [
            "diff --git a/logo.png b/logo.png",
            "new file mode 100644",
            "index 0000000..abc1234",
            "Binary files /dev/null and b/logo.png differ",
            "",
        ]
        .join("\n");
        let files = parse_diff(&raw);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "logo.png");
        assert!(files[0].binary);
        assert!(files[0].hunks.is_empty());
        assert_eq!((files[0].additions, files[0].deletions), (0, 0));
    }

    #[test]
    fn keeps_the_old_path_of_a_rename_and_the_name_of_a_deletion() {
        let raw = "diff --git a/old.rs b/new.rs
similarity index 100%
rename from old.rs
rename to new.rs
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
\\ No newline at end of file
";
        let files = parse_diff(raw);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "new.rs");
        assert_eq!(files[0].old_path.as_deref(), Some("old.rs"));
        assert_eq!(files[1].path, "gone.txt");
        assert_eq!(files[1].old_path, None);
        assert_eq!(files[1].deletions, 1);
        assert_eq!(files[1].hunks[0].lines, vec!["-bye".to_string()]);
    }
}
