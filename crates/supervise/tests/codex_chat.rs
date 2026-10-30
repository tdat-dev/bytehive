// Codex conversations, through the public Chat API: feed transcript lines, read items and meta.

use serde_json::{json, Value};

use supervise::codex_chat::CodexChat;
use supervise::types::{BgState, Chat, ChatItem, StepItem, TodoState};

fn at(sec: u32) -> String {
    format!("2026-09-25T10:00:{sec:02}.000Z")
}

fn row(sec: u32, kind: &str, payload: Value) -> String {
    json!({ "timestamp": at(sec), "type": kind, "payload": payload }).to_string()
}

fn item(sec: u32, payload: Value) -> String {
    row(sec, "response_item", payload)
}

fn user(sec: u32, texts: &[&str]) -> String {
    let content: Vec<Value> = texts.iter().map(|t| json!({ "type": "input_text", "text": t })).collect();
    item(sec, json!({ "type": "message", "role": "user", "content": content }))
}

fn said(sec: u32, text: &str) -> String {
    item(sec, json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": text }] }))
}

fn call(sec: u32, call_id: &str, name: &str, args: Value) -> String {
    item(sec, json!({ "type": "function_call", "call_id": call_id, "name": name, "arguments": args.to_string() }))
}

fn out(sec: u32, call_id: &str, output: Value) -> String {
    item(sec, json!({ "type": "function_call_output", "call_id": call_id, "output": output }))
}

fn steps_of(chat: &CodexChat) -> Vec<StepItem> {
    chat.items().iter().filter_map(|i| match i {
        ChatItem::Step(s) => Some(s.clone()),
        _ => None,
    }).collect()
}

#[test]
fn reads_a_turn_with_commands_and_reply() {
    let mut chat = CodexChat::new();
    chat.feed(&[
        row(0, "session_meta", json!({ "id": "s", "cwd": "D:\\app" })),
        item(0, json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "<permissions instructions>" }] })),
        user(1, &["<environment_context>\n  <cwd>D:\\app</cwd>\n</environment_context>"]),
        user(2, &["# AGENTS.md instructions for D:\\app\n\nrules"]),
        user(3, &["fix the login redirect"]),
        said(4, "Looking at the auth code."),
        call(5, "c1", "shell_command", json!({ "command": "npm test", "workdir": "D:\\app" })),
        out(6, "c1", json!("Exit code: 0\nWall time: 2.1 seconds\nOutput:\n12 passed\n")),
        call(7, "c2", "exec_command", json!({ "cmd": "npm run lint" })),
        out(8, "c2", json!("Chunk ID: a1\nWall time: 1.0 seconds\nProcess exited with code 1\nOriginal token count: 3\nOutput:\nlint error\n")),
        said(9, "Fixed."),
    ].join("\n"));
    let kinds: Vec<&str> = chat.items().iter().map(|i| i.kind()).collect();
    assert_eq!(kinds, vec!["user", "text", "step", "step", "text"]);
    match &chat.items()[0] {
        ChatItem::User(u) => assert_eq!(u.text, "fix the login redirect"),
        _ => panic!("expected user"),
    }
    let steps = steps_of(&chat);
    assert_eq!(steps[0].verb, "Ran");
    assert_eq!(steps[0].target, "npm test");
    assert_eq!(steps[0].output.as_deref(), Some("12 passed"));
    assert_eq!(steps[0].error, false);
    assert_eq!(steps[0].done, true);
    assert_eq!(steps[1].output.as_deref(), Some("lint error"));
    assert_eq!(steps[1].error, true);
    assert_eq!(chat.meta().turn, 1);
    assert_eq!(chat.meta().started, Some(1790330400000));
}

#[test]
fn reads_a_patch_per_file() {
    let patch = "*** Begin Patch\n*** Add File: D:/app/new.ts\n+export const a = 1;\n+export const b = 2;\n*** Update File: D:/app/old.ts\n@@ function x\n keep\n-gone\n+here\n@@\n+more\n*** Delete File: D:/app/dead.ts\n*** End Patch";
    let mut chat = CodexChat::new();
    chat.feed(&[
        user(0, &["go"]),
        item(1, json!({ "type": "custom_tool_call", "call_id": "p1", "name": "apply_patch", "input": patch })),
        item(2, json!({ "type": "custom_tool_call_output", "call_id": "p1", "output": json!({ "output": "Success. Updated the following files:\nA D:/app/new.ts\n", "metadata": { "exit_code": 0 } }).to_string() })),
    ].join("\n"));
    let steps = steps_of(&chat);
    let verbs: Vec<(&str, &str, bool, bool)> = steps.iter().map(|s| (s.verb.as_str(), s.target.as_str(), s.done, s.error)).collect();
    assert_eq!(verbs, vec![("Wrote", "new.ts", true, false), ("Edited", "old.ts", true, false), ("Deleted", "dead.ts", true, false)]);
    let files: Vec<(&str, bool, bool, u32)> = chat.meta().files.iter().map(|f| (f.name.as_str(), f.is_new, f.deleted.unwrap_or(false), f.turn)).collect();
    assert_eq!(files, vec![("dead.ts", false, true, 1), ("old.ts", false, false, 1), ("new.ts", true, false, 1)]);
}

#[test]
fn knows_settings_and_context() {
    let mut chat = CodexChat::new();
    chat.feed(&[
        row(0, "turn_context", json!({ "cwd": "D:\\app", "approval_policy": "on-request", "sandbox_policy": { "type": "workspace-write" }, "model": "gpt-5.4", "collaboration_mode": { "mode": "default", "settings": { "reasoning_effort": "medium" } } })),
        row(1, "event_msg", json!({ "type": "token_count", "info": { "total_token_usage": { "input_tokens": 900, "output_tokens": 120 }, "last_token_usage": { "input_tokens": 500, "cached_input_tokens": 300, "output_tokens": 40 } } })),
    ].join("\n"));
    assert_eq!(chat.meta().model.as_deref(), Some("gpt-5.4"));
    assert_eq!(chat.meta().effort.as_deref(), Some("medium"));
    assert_eq!(chat.meta().permission.as_deref(), Some("on-request"));
    assert_eq!(chat.meta().context, 500);
    assert_eq!(chat.meta().output, 120);
    chat.feed(&row(2, "event_msg", json!({ "type": "thread_settings_applied", "thread_settings": { "model": "gpt-5.6-sol", "approval_policy": "never", "reasoning_effort": "high", "collaboration_mode": { "mode": "plan" } } })));
    assert_eq!(chat.meta().model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(chat.meta().effort.as_deref(), Some("high"));
    assert_eq!(chat.meta().permission.as_deref(), Some("plan"));
    chat.feed(&row(3, "event_msg", json!({ "type": "token_count", "info": Value::Null })));
    assert_eq!(chat.meta().context, 500);
}

#[test]
fn keeps_running_commands_as_tasks() {
    let mut chat = CodexChat::new();
    chat.feed(&[
        user(0, &["start the server"]),
        call(1, "r1", "exec_command", json!({ "cmd": "npm run dev" })),
        out(2, "r1", json!("Chunk ID: a\nWall time: 10.0 seconds\nProcess running with session ID 43354\nOriginal token count: 2\nOutput:\nready\n")),
        call(3, "r2", "exec_command", json!({ "cmd": "npm run build" })),
        out(4, "r2", json!("Chunk ID: b\nWall time: 30.0 seconds\nProcess running with session ID 7\nOriginal token count: 0\nOutput:\n")),
        call(5, "w1", "write_stdin", json!({ "session_id": 7, "chars": "" })),
        out(6, "w1", json!("Chunk ID: c\nWall time: 3.0 seconds\nProcess exited with code 2\nOriginal token count: 1\nOutput:\nbuild failed\n")),
    ].join("\n"));
    let tasks: Vec<(&str, &str, BgState)> = chat.meta().tasks.iter().map(|t| (t.id.as_str(), t.label.as_str(), t.state)).collect();
    assert_eq!(tasks, vec![("s43354", "npm run dev", BgState::Running), ("s7", "npm run build", BgState::Failed)]);
}

#[test]
fn marks_stops_compactions_plan_and_search() {
    let mut chat = CodexChat::new();
    chat.feed(&[
        user(0, &["plan it"]),
        call(1, "u1", "update_plan", json!({ "plan": [{ "step": "one", "status": "completed" }, { "step": "two", "status": "in_progress" }] })),
        out(2, "u1", json!("Plan updated")),
        item(3, json!({ "type": "web_search_call", "status": "completed", "action": { "type": "search", "query": "tauri updater" } })),
        row(4, "event_msg", json!({ "type": "turn_aborted", "reason": "interrupted" })),
        row(5, "compacted", json!({ "message": "" })),
    ].join("\n"));
    let todos = chat.meta().todos.clone().unwrap_or_default();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].state, TodoState::Completed);
    assert_eq!(todos[1].state, TodoState::InProgress);
    let drawn: Vec<String> = chat.items().iter().map(|i| match i {
        ChatItem::Step(s) => format!("{} {}", s.verb, s.target),
        ChatItem::User(u) => format!("user:{}", u.text),
        ChatItem::Text(t) => format!("text:{}", t.text),
        ChatItem::Note(n) => format!("note:{}", n.text),
    }).collect();
    assert_eq!(drawn, vec!["user:plan it", "Updated the plan 1 of 2 done", "Searched the web for tauri updater", "note:You stopped the agent", "note:Conversation compacted"]);
}

#[test]
fn reads_js_cells_and_nested_output() {
    let cell = "const r = await tools.shell_command({command:\"rtk git status\",workdir:\"D:\\\\app\"});\nconst s = await tools.exec_command({cmd:\"npm test\"});";
    let mut chat = CodexChat::new();
    chat.feed(&[
        user(0, &["go"]),
        item(1, json!({ "type": "custom_tool_call", "call_id": "j1", "name": "exec", "input": cell })),
        item(2, json!({ "type": "custom_tool_call_output", "call_id": "j1", "output": [{ "type": "input_text", "text": "Script completed\nWall time 3.1 seconds\nOutput:\n" }, { "type": "input_text", "text": "Exit code: 0\nWall time: 1.5 seconds\nOutput:\nclean\nExit code: 1\nWall time: 2 seconds\nOutput:\n1 failed" }] })),
        item(3, json!({ "type": "custom_tool_call", "call_id": "j2", "name": "exec", "input": "await tools.apply_patch(`*** Begin Patch\n*** Add File: D:/app/x.ts\n+hi\n*** End Patch`);" })),
        item(4, json!({ "type": "custom_tool_call_output", "call_id": "j2", "output": [{ "type": "input_text", "text": "Script completed\nOutput:\nSuccess." }] })),
    ].join("\n"));
    let steps = steps_of(&chat);
    assert_eq!(steps[0].output.as_deref(), Some("clean\n1 failed"));
    assert_eq!(steps[0].error, true);
    assert_eq!(steps[1].verb, "Wrote");
    assert_eq!(steps[1].target, "x.ts");
    assert_eq!(steps[1].error, false);
    let files: Vec<(&str, bool)> = chat.meta().files.iter().map(|f| (f.name.as_str(), f.is_new)).collect();
    assert_eq!(files, vec![("x.ts", true)]);
}

#[test]
fn shows_pictures_a_tool_returned() {
    let mut chat = CodexChat::new();
    chat.feed(&[
        user(0, &["look"]),
        call(1, "v1", "view_image", json!({ "path": "C:/t/a.png" })),
        out(2, "v1", json!([{ "type": "input_image", "image_url": "data:image/png;base64,iVBORw0KGgo=" }])),
    ].join("\n"));
    let steps = steps_of(&chat);
    assert_eq!(steps[0].verb, "Looked at");
    assert_eq!(steps[0].target, "a.png");
    assert_eq!(steps[0].done, true);
    let images = steps[0].images.clone().unwrap_or_default();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].media, "image/png");
}
