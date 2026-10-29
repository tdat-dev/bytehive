// Unified-diff parsing: files with paths and counts, hunk line kinds.

use supervise::diff_file::{parse_diff, HunkKind};

const SAMPLE: &str = "diff --git a/src/app.ts b/src/app.ts\nindex 111..222 100644\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1,3 +1,4 @@\n const a = 1;\n-const b = 2;\n+const b = 3;\n+const c = 4;\n const d = 5;\n";

#[test]
fn returns_one_file_with_path_and_counts() {
    let files = parse_diff(SAMPLE);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "src/app.ts");
    assert_eq!(files[0].additions, 2);
    assert_eq!(files[0].deletions, 1);
}

#[test]
fn parses_hunk_lines_with_kinds() {
    let h = parse_diff(SAMPLE)[0].hunks[0].clone();
    assert_eq!(h.header, "@@ -1,3 +1,4 @@");
    let kinds: Vec<HunkKind> = h.lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, vec![HunkKind::Ctx, HunkKind::Del, HunkKind::Add, HunkKind::Add, HunkKind::Ctx]);
    assert_eq!(h.lines[2].text, "const b = 3;");
}

#[test]
fn returns_empty_for_an_empty_diff() {
    assert!(parse_diff("").is_empty());
}

#[test]
fn names_binary_files_and_flags_them() {
    let raw = "diff --git a/logo.png b/logo.png\nnew file mode 100644\nindex 0000000..abc1234\nBinary files /dev/null and b/logo.png differ\n";
    let files = parse_diff(raw);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "logo.png");
    assert_eq!(files[0].binary, true);
    assert!(files[0].hunks.is_empty());
    assert_eq!(files[0].additions, 0);
    assert_eq!(files[0].deletions, 0);
}
