// opencode conversations, through the public Chat API: feed store rows,
// read turns, in-place step updates, files, and errors.

use serde_json::{json, Value};

use supervise::opencode_chat::OpencodeChat;
use supervise::types::{Chat, ChatItem, StepItem, TodoState};

// Rows as the transcript hands them over: one per message or part.
fn msg(id: &str, created: u64, data: Value) -> String {
    json!({ "kind": "message", "id": id, "messageId": id, "created": created, "updated": created, "data": data }).to_string()
}

fn part(id: &str, message_id: &str, created: u64, data: Value) -> String {
    json!({ "kind": "part", "id": id, "messageId": message_id, "created": created, "updated": created, "data": data }).to_string()
}

fn tool(id: &str, message_id: &str, created: u64, name: &str, state: Value) -> String {
    part(id, message_id, created, json!({ "type": "tool", "tool": name, "callID": format!("c_{id}"), "state": state }))
}

fn step_of(item: &ChatItem) -> &StepItem {
    match item {
        ChatItem::Step(s) => s,
        _ => panic!("expected step"),
    }
}

#[test]
fn reads_a_turn_with_steps_and_text() {
    let mut chat = OpencodeChat::new();
    chat.feed(&[
        msg("msg_001", 1000, json!({ "role": "user", "agent": "build", "model": { "providerID": "p", "modelID": "m", "variant": "high" }, "time": { "created": 1000 } })),
        part("prt_001", "msg_001", 1001, json!({ "type": "text", "text": "fix the redirect" })),
        part("prt_002", "msg_001", 1001, json!({ "type": "text", "text": "<system context>", "synthetic": true })),
        msg("msg_002", 1002, json!({ "role": "assistant", "modelID": "deepseek-v4", "agent": "build", "tokens": { "input": 10, "output": 5, "reasoning": 2, "cache": { "read": 100, "write": 0 } }, "time": { "created": 1002 } })),
        part("prt_003", "msg_002", 1003, json!({ "type": "step-start" })),
        part("prt_004", "msg_002", 1003, json!({ "type": "reasoning", "text": "thinking" })),
        tool("prt_005", "msg_002", 1004, "bash", json!({ "status": "completed", "input": { "command": "npm test", "description": "Run the tests" }, "output": "12 passed", "metadata": { "exit": 0 } })),
        part("prt_006", "msg_002", 1005, json!({ "type": "text", "text": "Fixed." })),
    ].join("\n"));
    let kinds: Vec<&str> = chat.items().iter().map(|i| i.kind()).collect();
    assert_eq!(kinds, vec!["user", "step", "text"]);
    match &chat.items()[0] {
        ChatItem::User(u) => assert_eq!(u.text, "fix the redirect"),
        _ => panic!("expected user"),
    }
    let s = step_of(&chat.items()[1]);
    assert_eq!(s.verb, "Ran");
    assert_eq!(s.target, "Run the tests");
    assert_eq!(s.full.as_deref(), Some("npm test"));
    assert_eq!(s.output.as_deref(), Some("12 passed"));
    assert_eq!(s.done, true);
    assert_eq!(chat.meta().model.as_deref(), Some("deepseek-v4"));
    assert_eq!(chat.meta().effort.as_deref(), Some("high"));
    assert_eq!(chat.meta().permission.as_deref(), Some("build"));
    assert_eq!(chat.meta().context, 110);
    assert_eq!(chat.meta().output, 7);
    assert_eq!(chat.meta().turn, 1);
    assert_eq!(chat.meta().started, Some(1000));
}

#[test]
fn updates_a_tool_step_in_place_and_waits_for_its_message() {
    let mut chat = OpencodeChat::new();
    chat.feed(&tool("prt_010", "msg_009", 2001, "bash", json!({ "status": "running", "input": { "command": "npm run build" } })));
    assert!(chat.items().is_empty());
    chat.feed(&msg("msg_009", 2000, json!({ "role": "assistant", "modelID": "m", "time": { "created": 2000 } })));
    assert_eq!(chat.items().len(), 1);
    assert_eq!(step_of(&chat.items()[0]).done, false);
    chat.feed(&tool("prt_010", "msg_009", 2001, "bash", json!({ "status": "error", "input": { "command": "npm run build" }, "error": "exit 1" })));
    assert_eq!(chat.items().len(), 1);
    let s = step_of(&chat.items()[0]);
    assert_eq!(s.done, true);
    assert_eq!(s.error, true);
    assert_eq!(s.output.as_deref(), Some("exit 1"));
}

#[test]
fn keeps_time_order_whatever_order_rows_come_in() {
    let mut chat = OpencodeChat::new();
    chat.feed(&[
        msg("msg_020", 3000, json!({ "role": "assistant", "time": { "created": 3000 } })),
        part("prt_022", "msg_020", 3002, json!({ "type": "text", "text": "second" })),
        part("prt_021", "msg_020", 3001, json!({ "type": "text", "text": "first" })),
    ].join("\n"));
    let texts: Vec<&str> = chat.items().iter().map(|i| match i {
        ChatItem::Text(t) => t.text.as_str(),
        _ => "",
    }).collect();
    assert_eq!(texts, vec!["first", "second"]);
}

#[test]
fn tells_new_files_from_edited_ones_without_double_counting() {
    let patch = "Index: D:\\a\\x.ts\n===\n--- D:\\a\\x.ts\n+++ D:\\a\\x.ts\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n";
    let mut chat = OpencodeChat::new();
    chat.feed(&[
        msg("msg_030", 4000, json!({ "role": "user", "time": { "created": 4000 } })),
        part("prt_030", "msg_030", 4000, json!({ "type": "text", "text": "go" })),
        msg("msg_031", 4001, json!({ "role": "assistant", "time": { "created": 4001 } })),
        tool("prt_031", "msg_031", 4002, "write", json!({ "status": "completed", "input": { "filePath": "D:\\a\\new.ts", "content": "a\nb" }, "metadata": { "exists": false, "filepath": "D:\\a\\new.ts" } })),
        tool("prt_032", "msg_031", 4003, "edit", json!({ "status": "completed", "input": { "filePath": "D:\\a\\x.ts", "oldString": "old", "newString": "new" }, "metadata": { "filediff": { "file": "D:\\a\\x.ts", "patch": patch, "additions": 1, "deletions": 1 } } })),
        tool("prt_033", "msg_031", 4004, "write", json!({ "status": "completed", "input": { "filePath": "D:\\a\\x.ts", "content": "z" }, "metadata": { "exists": true } })),
    ].join("\n"));
    // fed again (a later read hands the same rows back): not counted twice
    chat.feed(&tool("prt_032", "msg_031", 4003, "edit", json!({ "status": "completed", "input": { "filePath": "D:\\a\\x.ts", "oldString": "old", "newString": "new" }, "metadata": { "filediff": { "patch": patch, "additions": 1, "deletions": 1 } } })));
    let files: Vec<(&str, bool, u32, u32, usize)> = chat.meta().files.iter().map(|f| (f.name.as_str(), f.is_new, f.added, f.removed, f.steps.len())).collect();
    assert_eq!(files, vec![("x.ts", false, 2, 1, 2), ("new.ts", true, 2, 0, 1)]);
    let edit = chat.items().iter().find(|i| i.id() == "prt_032").map(step_of).expect("edit step");
    let lines = edit.diff.clone().unwrap_or_default();
    let diff: Vec<(String, String)> = lines.iter().map(|l| {
        let sign = match l.sign {
            supervise::types::DiffSign::Same => " ",
            supervise::types::DiffSign::Added => "+",
            supervise::types::DiffSign::Removed => "-",
        };
        (sign.to_string(), l.text.clone())
    }).collect();
    assert_eq!(diff, vec![(" ".to_string(), "keep".to_string()), ("-".to_string(), "old".to_string()), ("+".to_string(), "new".to_string())]);
}

#[test]
fn shows_pictures_plan_errors_and_compaction() {
    let mut chat = OpencodeChat::new();
    chat.feed(&[
        msg("msg_040", 5000, json!({ "role": "user", "time": { "created": 5000 } })),
        part("prt_040", "msg_040", 5000, json!({ "type": "text", "text": "look" })),
        part("prt_041", "msg_040", 5000, json!({ "type": "file", "mime": "image/png", "filename": "clipboard", "url": "data:image/png;base64,iVBORw0KGgo=" })),
        msg("msg_041", 5001, json!({ "role": "assistant", "time": { "created": 5001 }, "error": { "name": "APIError", "data": { "message": "Insufficient balance" } } })),
        tool("prt_042", "msg_041", 5002, "todowrite", json!({ "status": "completed", "input": { "todos": [{ "content": "a", "status": "completed" }, { "content": "b", "status": "in_progress" }] } })),
        part("prt_043", "msg_041", 5003, json!({ "type": "compaction", "auto": true })),
    ].join("\n"));
    match &chat.items()[0] {
        ChatItem::User(u) => {
            assert_eq!(u.text, "look");
            assert_eq!(u.images, 1);
            let pics = u.pics.clone().unwrap_or_default();
            assert_eq!(pics.len(), 1);
            assert_eq!(pics[0].media, "image/png");
            assert_eq!(pics[0].data, "iVBORw0KGgo=");
        }
        _ => panic!("expected user"),
    }
    let todos = chat.meta().todos.clone().unwrap_or_default();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].state, TodoState::Completed);
    assert_eq!(todos[1].state, TodoState::InProgress);
    let drawn: Vec<String> = chat.items().iter().map(|i| match i {
        ChatItem::Note(n) => n.text.clone(),
        other => other.kind().to_string(),
    }).collect();
    assert_eq!(drawn, vec!["user", "step", "Conversation compacted", "Error: Insufficient balance"]);
}
