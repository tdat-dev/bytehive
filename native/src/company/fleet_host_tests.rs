use super::*;
use std::time::UNIX_EPOCH;

const RULE: &str = "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}";

#[test]
fn sees_claudes_input_box_between_its_two_rules() {
    let screen = format!("Welcome\n{RULE}\n> \n{RULE}\n  ? for shortcuts");
    assert!(composer_drawn(&screen));
    assert!(composer_drawn(&format!("{RULE}\n\u{203A} typed\n{RULE}")));
}

#[test]
fn sees_no_box_while_claude_is_still_starting() {
    assert!(!composer_drawn("Loading MCP servers..."));
    assert!(!composer_drawn(&format!("{RULE}\nno prompt here\n{RULE}")));
    assert!(!composer_drawn(&format!("{RULE}\n> only one rule")));
}

#[test]
fn takes_the_box_from_the_last_two_rules() {
    let screen = format!("{RULE}\n> old\n{RULE}\ntranscript\n{RULE}\nnothing\n{RULE}");
    assert!(!composer_drawn(&screen));
}

#[test]
fn reads_only_claude_for_a_box_any_other_cli_is_ready() {
    assert!(at_prompt("codex", ""));
    assert!(!at_prompt("claude", "booting"));
    assert!(at_prompt("Claude", &format!("{RULE}\n> \n{RULE}")));
}

#[test]
fn writes_a_board_and_its_mirror() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_str().unwrap();
    let b = board::default_board();
    write_board_file(d, &b, None).unwrap();
    assert_eq!(board::read(d).unwrap().unwrap().0.lists.len(), 3);
    assert!(dir.path().join(".bytehive").join("board.md").exists());
}

#[test]
fn calls_a_board_written_since_it_was_read_a_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_str().unwrap();
    let b = board::default_board();
    write_board_file(d, &b, None).unwrap();
    let (_, seen) = board::read(d).unwrap().unwrap();
    write_board_file(d, &b, seen).unwrap();
    assert_eq!(write_board_file(d, &b, Some(UNIX_EPOCH)), Err(BoardWrite::Conflict));
}

#[test]
fn has_no_fresh_tokens_for_a_cli_it_cannot_read() {
    assert_eq!(fresh_tokens("powershell", "D:\\a", None, 0), None);
    assert_eq!(fresh_tokens("claude", "D:\\a", None, 0), None);
}

#[test]
fn knows_a_running_process_and_an_invalid_pid() {
    assert!(pid_alive(std::process::id()));
    assert!(!pid_alive(0xFFFF_FFF0));
}

#[test]
fn reads_a_screen_again_only_after_new_output() {
    let mut reads = 0;
    let mut look = |printed: u64, screen: &str| {
        screen_facts("seat-screen-1", printed, || {
            reads += 1;
            (screen.to_string(), "claude".to_string())
        })
    };
    let boxed = format!("{RULE}\n> \n{RULE}");
    assert_eq!(look(5, &boxed), (false, true));
    assert_eq!(look(5, "anything"), (false, true), "no new output: what it said before");
    assert_eq!(look(6, "booting"), (false, false));
    drop(look);
    assert_eq!(reads, 2);
}
