//! Ported from src/board.test.ts, src/dispatch.test.ts, mcp/src/board.test.ts
//! and the byte format bytehive-mcp writes.

use chrono::NaiveDate;

use super::{
    ReadError, add_card, add_list, assign_card, board_path, board_to_markdown, change, default_board, delete_list, dispatch_prompt, done_card_ids, find_card, fmt_due, mk_card, move_card, move_list, parse,
    read, rename_list, serialize, uid, valid_due,
};

/// What `JSON.stringify({ version: 2, lists }, null, 2)` writes for this board.
const MCP_BYTES: &str = r#"{
  "version": 2,
  "lists": [
    {
      "id": "l1",
      "title": "To do",
      "cards": [
        {
          "id": "c1",
          "title": "Fix ü \"login\"",
          "desc": "line\nnext",
          "labels": [
            "red"
          ],
          "due": null,
          "checklist": [
            {
              "id": "i1",
              "text": "a",
              "done": true
            }
          ],
          "assignee": "Claude Code #1",
          "returns": 2,
          "done": {
            "repoRoot": "r",
            "files": [],
            "at": 1730000000000,
            "by": "Ana"
          }
        }
      ]
    },
    {
      "id": "l2",
      "title": "Done",
      "cards": []
    }
  ]
}"#;

#[test]
fn writes_the_same_bytes_as_the_mcp_server() {
    let b = parse(MCP_BYTES).unwrap();
    assert_eq!(serialize(&b), MCP_BYTES);
    assert_eq!(parse(&serialize(&b)).unwrap(), b);
}

#[test]
fn refuses_shapes_that_are_not_a_board() {
    assert!(parse("null").is_err());
    assert!(parse(r#"{"todo":[]}"#).is_err());
    assert!(parse("{not json").is_err());
}

#[test]
fn normalizes_sparse_cards() {
    let b = parse(r#"{"lists":[{"id":"l1","title":"T","cards":[{"id":"c1","title":"x"}]}]}"#).unwrap();
    let c = &b.lists[0].cards[0];
    assert_eq!((c.desc.as_str(), c.labels.len(), c.due.clone(), c.checklist.len()), ("", 0, None, 0));
    assert_eq!(c.assignee, None);
}

#[test]
fn keeps_assignee_and_done_by_drops_junk() {
    let b = parse(
        r#"{"lists":[{"id":"l1","title":"Done","cards":[
            {"id":"c1","title":"t","assignee":"Claude Code #1","done":{"repoRoot":"r","files":["a.ts",3],"at":5,"by":"Claude Code #1"}},
            {"id":"c2","title":"junk","assignee":9,"labels":["red",4],"checklist":[{"text":"ok"},{"done":true},"x"],"returns":0,"job":""},
            {"title":"no id","due":7}
        ]},{"title":5}]}"#,
    )
    .unwrap();
    let cards = &b.lists[0].cards;
    assert_eq!(cards[0].assignee.as_deref(), Some("Claude Code #1"));
    let done = cards[0].done.as_ref().unwrap();
    assert_eq!((done.by.as_deref(), done.files.clone()), (Some("Claude Code #1"), vec!["a.ts".to_string()]));
    assert_eq!(cards[1].assignee, None);
    assert_eq!(cards[1].labels, vec!["red".to_string()]);
    assert_eq!(cards[1].checklist.len(), 1);
    assert!(cards[1].checklist[0].id.starts_with('i') && !cards[1].checklist[0].done);
    assert_eq!((cards[1].returns, cards[1].job.clone()), (None, None));
    assert!(cards[2].id.starts_with('c') && cards[2].due.is_none());
    assert_eq!(b.lists[1].title, "List");
}

#[test]
fn ids_have_the_mcp_shape() {
    let id = uid("l");
    assert!(id.len() >= 9 && id.starts_with('l') && id[1..].chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()));
    assert_ne!(uid("c"), uid("c"));
}

#[test]
fn done_ids_come_from_any_list_called_done() {
    let mut b = parse(r#"{"lists":[{"id":"l1","title":"To do","cards":[{"id":"a"}]},{"id":"l2","title":"DONE","cards":[{"id":"b"}]}]}"#).unwrap();
    assert_eq!(done_card_ids(&b), vec!["b".to_string()]);
    b.lists[0].title = " done ".into();
    assert_eq!(done_card_ids(&b).len(), 2);
}

#[test]
fn finds_cards_across_lists() {
    let mut b = default_board();
    let c = mk_card("hello");
    let id = c.id.clone();
    b.lists[1].cards.push(c);
    assert_eq!(find_card(&b, &id), Some((1, 0)));
    assert_eq!(find_card(&b, "nope"), None);
}

#[test]
fn lists_add_rename_delete() {
    let mut b = default_board();
    add_list(&mut b, "  Review ");
    add_list(&mut b, "   ");
    assert_eq!(b.lists.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), ["To do", "Doing", "Done", "Review"]);
    let id = b.lists[3].id.clone();
    rename_list(&mut b, &id, " ");
    assert_eq!(b.lists[3].title, "Review");
    rename_list(&mut b, &id, "QA");
    assert_eq!(b.lists[3].title, "QA");
    delete_list(&mut b, &id);
    assert_eq!(b.lists.len(), 3);
}

#[test]
fn moves_cards_and_lists() {
    let mut b = default_board();
    let (todo, doing) = (b.lists[0].id.clone(), b.lists[1].id.clone());
    add_card(&mut b, &todo, "a");
    add_card(&mut b, &todo, " c ");
    add_card(&mut b, &todo, "");
    let (a, c) = (b.lists[0].cards[0].id.clone(), b.lists[0].cards[1].id.clone());
    assert_eq!(b.lists[0].cards[1].title, "c");
    move_card(&mut b, &c, &todo, Some(&a));
    assert_eq!(b.lists[0].cards.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["c", "a"]);
    move_card(&mut b, &a, &doing, None);
    assert_eq!((b.lists[0].cards.len(), b.lists[1].cards[0].title.as_str()), (1, "a"));
    // A list an agent removed meanwhile: nothing moves.
    move_card(&mut b, &a, "lgone", None);
    assert_eq!(find_card(&b, &a), Some((1, 0)));
    move_list(&mut b, &doing, Some(&todo));
    assert_eq!(b.lists.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), ["Doing", "To do", "Done"]);
    move_list(&mut b, &doing, None);
    assert_eq!(b.lists[2].title, "Doing");
}

#[test]
fn sending_a_card_claims_it_and_pulls_it_into_doing() {
    let mut b = parse(r#"{"lists":[{"id":"l1","title":"To do","cards":[{"id":"c1","title":"x"}]}]}"#).unwrap();
    assign_card(&mut b, "c1", Some("Codex #1"), true);
    assert_eq!(b.lists[0].title, "Doing");
    assert_eq!(b.lists[0].cards[0].assignee.as_deref(), Some("Codex #1"));
    assign_card(&mut b, "c1", None, false);
    assert_eq!(b.lists[0].cards[0].assignee, None);
}

#[test]
fn dispatch_prompt_has_the_task_and_the_board_tools() {
    let mut card = mk_card("Fix login bug");
    card.desc = "Session cookie expires too early".into();
    card.checklist = vec![super::ChecklistItem { id: "i1".into(), text: "add test".into(), done: false }, super::ChecklistItem { id: "i2".into(), text: "done thing".into(), done: true }];
    let p = dispatch_prompt(&card);
    assert!(p.starts_with("Task: Fix login bug\n\nSession cookie expires too early\n\n- [ ] add test\n\n"));
    assert!(p.contains("card_move (card: \"Fix login bug\")") && p.contains("\"Doing\"") && p.contains("card_done"));
    assert!(!p.contains("done thing"));
    assert!(dispatch_prompt(&mk_card("Tidy")).starts_with("Task: Tidy\n\nWhen you start"));
}

#[test]
fn mirrors_the_board_as_markdown() {
    let b = parse(MCP_BYTES).unwrap();
    assert_eq!(board_to_markdown(&b), "# Board\n\n## To do (1)\n\n- [x] Fix ü \"login\"\n  line\n  next\n  - [x] a\n\n## Done (0)\n\n_(empty)_\n");
}

#[test]
fn due_dates() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    assert_eq!(fmt_due("2026-11-09", today), ("Nov 9".to_string(), false, true));
    assert_eq!(fmt_due("2026-11-10", today), ("Nov 10".to_string(), false, true));
    assert_eq!(fmt_due("2026-11-11", today), ("Nov 11".to_string(), false, false));
    assert_eq!(fmt_due("2026-11-08", today), ("Nov 8".to_string(), true, false));
    assert!(valid_due("2026-02-28") && !valid_due("2026-02-30") && !valid_due("2026-2-3") && !valid_due(""));
}

/// A folder under the system temp folder, removed when dropped.
struct Temp(std::path::PathBuf);

impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("bytehive-board-test-{}", uid("t")));
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }
    fn dir(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_change_lands_on_the_file_as_an_agent_left_it() {
    let t = Temp::new();
    let dir = t.dir();
    assert_eq!(read(&dir), Ok(None));
    let shown = default_board();
    let todo = shown.lists[0].id.clone();
    // No file yet: the board on screen is written.
    let (b, at) = change(&dir, &shown, &|b| add_card(b, &todo, "mine")).unwrap();
    assert!(at.is_some() && b.lists[0].cards.len() == 1);
    // An agent adds a card the screen has not seen; ours keeps it.
    let mut theirs = b.clone();
    add_card(&mut theirs, &todo, "agent's");
    std::fs::write(board_path(&dir), serialize(&theirs)).unwrap();
    let (b, _) = change(&dir, &shown, &|b| add_card(b, &todo, "second")).unwrap();
    assert_eq!(b.lists[0].cards.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["mine", "agent's", "second"]);
    assert_eq!(std::fs::read_to_string(board_path(&dir)).unwrap(), serialize(&b));
    assert!(std::fs::read_to_string(t.0.join(".bytehive").join("board.md")).unwrap().contains("- [ ] second"));
    // A corrupt file reads as such, and your change replaces it.
    std::fs::write(board_path(&dir), "{oops").unwrap();
    assert!(matches!(read(&dir), Err(ReadError::Corrupt(_))));
    let (b, _) = change(&dir, &shown, &|b| add_list(b, "QA")).unwrap();
    assert_eq!(b.lists.len(), 4);
    assert_eq!(read(&dir).unwrap().unwrap().0, b);
}
