// Review texts: send-back messages, diff line numbers, per-agent drafts,
// and riskiest-first file order.

use supervise::diff_file::{DiffFile, HunkKind, HunkLine};
use supervise::review_text::{
    comment_where, comments_text, conflict_text, diff_totals, number_hunk, rank_files, risk_of, send_back_text, CommentDrafts, CommentSide,
    Conflict, LineComment, NewComment,
};

fn comment(text: &str) -> LineComment {
    LineComment {
        id: "x".to_string(),
        path: "src/a.ts".to_string(),
        side: CommentSide::New,
        start: 12,
        end: 12,
        code: vec!["const x = 1;".to_string()],
        text: text.to_string(),
    }
}

#[test]
fn conflict_names_files_and_the_fix() {
    let t = conflict_text(&Conflict { base: "main".to_string(), branch: "bytehive/eli-1a2b3c".to_string(), files: vec!["src/a.ts".to_string(), "src/b.ts".to_string()] });
    assert!(t.contains("Merging your branch bytehive/eli-1a2b3c into main conflicts in:\n- src/a.ts\n- src/b.ts"));
    assert!(t.contains("git merge main"));
    assert!(t.contains("resolve the conflicts"));
    assert!(!t.contains('\u{2014}'));
}

#[test]
fn types_one_message_conflict_then_note() {
    assert_eq!(send_back_text(Some("  add a test  "), None, &[]), "add a test");
    let conflict = Conflict { base: "main".to_string(), branch: "b".to_string(), files: vec![] };
    let both = send_back_text(Some("and keep the old name"), Some(&conflict), &[]);
    assert!(both.starts_with("Merging your branch"));
    assert!(both.ends_with("\n\nand keep the old name"));
    assert_eq!(send_back_text(Some("   "), None, &[]), "");
    assert_eq!(send_back_text(None, None, &[]), "");
}

#[test]
fn numbers_each_side_from_the_hunk_header() {
    let lines = vec![
        HunkLine { kind: HunkKind::Ctx, text: "a".to_string() },
        HunkLine { kind: HunkKind::Del, text: "b".to_string() },
        HunkLine { kind: HunkKind::Add, text: "B".to_string() },
        HunkLine { kind: HunkKind::Add, text: "C".to_string() },
        HunkLine { kind: HunkKind::Ctx, text: "d".to_string() },
    ];
    let numbered = number_hunk("@@ -10,3 +10,4 @@ fn x()", &lines);
    let pairs: Vec<(Option<u32>, Option<u32>)> = numbered.iter().map(|l| (l.old, l.new)).collect();
    assert_eq!(pairs, vec![(Some(10), Some(10)), (Some(11), None), (None, Some(11)), (None, Some(12)), (Some(12), Some(13))]);
}

#[test]
fn counts_a_new_file_from_one_and_totals_a_diff() {
    let one = number_hunk("@@ -0,0 +1 @@", &[HunkLine { kind: HunkKind::Add, text: "x".to_string() }]);
    assert_eq!(one[0].new, Some(1));
    let totals = diff_totals(&[
        DiffFile { path: "a".to_string(), additions: 3, deletions: 1, hunks: vec![], binary: false },
        DiffFile { path: "b".to_string(), additions: 2, deletions: 0, hunks: vec![], binary: false },
    ]);
    assert_eq!((totals.add, totals.del), (5, 1));
}

#[test]
fn says_where_a_comment_is() {
    let c = comment("Name it");
    assert_eq!(comment_where(&c.path, c.side, c.start, c.end), "src/a.ts:12");
    assert_eq!(comment_where("src/a.ts", CommentSide::New, 12, 14), "src/a.ts:12-14");
    assert_eq!(comment_where("src/a.ts", CommentSide::Old, 7, 7), "src/a.ts:7 (removed)");
}

#[test]
fn types_every_comment_where_quote_comment_then_todo() {
    let mut second = comment("Guard b too");
    second.path = "src/b.ts".to_string();
    second.start = 3;
    second.end = 4;
    second.code = vec!["if (a)".to_string(), "  go();".to_string()];
    let mut first = comment("Rename x to count");
    first.id = "y".to_string();
    let empty = comment("   ");
    let t = send_back_text(Some("Also run the linter."), None, &[second, first, empty]);
    assert_eq!(
        t,
        [
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
        .join("\n")
    );
    assert!(!t.contains('\u{2014}'));
    assert!(comments_text(&[comment("x")]).starts_with("Review comments on your changes:"));
}

#[test]
fn keeps_drafts_per_agent_until_cleared() {
    let mut drafts = CommentDrafts::new();
    let new = |text: &str| NewComment {
        id: None,
        path: "a.ts".to_string(),
        side: CommentSide::New,
        start: 1,
        end: 1,
        code: vec!["x".to_string()],
        text: text.to_string(),
    };
    let a = drafts.put_comment("ana", new("one"));
    drafts.put_comment("ana", NewComment { start: 2, end: 2, code: vec!["y".to_string()], text: "two".to_string(), ..new("") });
    drafts.put_comment("bo", NewComment { path: "b.ts".to_string(), side: CommentSide::Old, start: 5, end: 5, code: vec!["z".to_string()], text: "why?".to_string(), id: None });
    drafts.put_comment("ana", NewComment { id: Some(a.id.clone()), path: "a.ts".to_string(), side: CommentSide::New, start: 1, end: 1, code: vec!["x".to_string()], text: "one, edited".to_string() });
    let texts: Vec<String> = drafts.comments_of("ana").iter().map(|x| x.text.clone()).collect();
    assert_eq!(texts, vec!["one, edited".to_string(), "two".to_string()]);
    drafts.drop_comment("ana", &a.id);
    let texts: Vec<String> = drafts.comments_of("ana").iter().map(|x| x.text.clone()).collect();
    assert_eq!(texts, vec!["two".to_string()]);
    drafts.clear_comments("ana");
    assert!(drafts.comments_of("ana").is_empty());
    assert_eq!(drafts.comments_of("bo").len(), 1);
    drafts.clear_comments("bo");
}

#[test]
fn ranks_secrets_above_auth_above_code_above_tests_above_docs() {
    let mut paths = vec!["docs/guide.md", "src/app.test.ts", "src/app.ts", "src/auth/login.ts", ".env"];
    paths.sort_by(|a, b| risk_of(b).cmp(&risk_of(a)));
    assert_eq!(paths, vec![".env", "src/auth/login.ts", "src/app.ts", "src/app.test.ts", "docs/guide.md"]);
}

#[test]
fn ranks_manifests_with_config_and_keeps_diff_order_inside_a_tier() {
    assert!(risk_of("package.json") > risk_of("src/app.ts"));
    assert!(risk_of("Dockerfile") > risk_of("src/app.ts"));
    let files = vec![("b.ts".to_string(),), ("a.ts".to_string(),)];
    let ranked = rank_files(&files, |f| f.0.as_str());
    let paths: Vec<&str> = ranked.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(paths, vec!["b.ts", "a.ts"]);
}
