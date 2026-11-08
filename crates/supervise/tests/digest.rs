// Handoff briefs: the task, plan, files and last words a new agent starts with.

use serde_json::json;
use supervise::claude::ClaudeChat;
use supervise::digest::{handoff_brief, HandoffFrom};
use supervise::types::Chat;

const T: &str = "2026-09-27T10:00:00.000Z";

fn line(v: serde_json::Value) -> String {
    let mut o = v.as_object().cloned().unwrap();
    o.insert("timestamp".to_string(), json!(T));
    serde_json::Value::Object(o).to_string() + "\n"
}

fn user(text: &str) -> String {
    line(json!({ "type": "user", "origin": { "kind": "human" }, "message": { "content": text } }))
}

fn said(text: &str, id: &str) -> String {
    line(json!({ "type": "assistant", "message": { "id": id, "content": [{ "type": "text", "text": text }] } }))
}

fn from(name: &str, cli: &str, why: &str) -> HandoffFrom {
    HandoffFrom { name: name.to_string(), cli: cli.to_string(), why: why.to_string(), title: None, first_asks: vec![], git_files: vec![] }
}

#[test]
fn carries_task_plan_files_and_where_it_got_to() {
    let mut chat = ClaudeChat::new();
    chat.feed(&format!(
        "{}{}{}{}{}",
        user("Make the header sticky and add a dark mode toggle"),
        line(json!({ "type": "assistant", "message": { "id": "a1", "content": [{ "type": "tool_use", "id": "t1", "name": "TodoWrite", "input": { "todos": [
            { "content": "Sticky header", "status": "completed", "activeForm": "x" },
            { "content": "Dark mode toggle", "status": "in_progress", "activeForm": "x" },
            { "content": "Tests", "status": "pending", "activeForm": "x" },
        ] } }] } })),
        line(json!({ "type": "assistant", "message": { "id": "a2", "content": [{ "type": "tool_use", "id": "t2", "name": "Edit", "input": { "file_path": "D:/app/src/header.css", "old_string": "a", "new_string": "b" } }] } })),
        line(json!({ "type": "user", "message": { "content": [{ "type": "tool_result", "tool_use_id": "t2", "content": "ok" }] } })),
        said("The header is sticky now. Next I'll wire the toggle into the settings menu.", "m1"),
    ));
    let brief = handoff_brief(&from("Ana", "Claude Code", "it hit its usage limit"), Some((chat.items(), chat.meta())));
    assert!(brief.contains("taking over a task from Ana (Claude Code)"), "{brief}");
    assert!(brief.contains("It stopped because it hit its usage limit"), "{brief}");
    assert!(brief.contains("1. Make the header sticky and add a dark mode toggle"), "{brief}");
    assert!(brief.contains("- [done] Sticky header\n- [doing] Dark mode toggle\n- [to do] Tests"), "{brief}");
    assert!(brief.contains("- D:/app/src/header.css"), "{brief}");
    assert!(brief.contains("> The header is sticky now."), "{brief}");
    assert!(brief.contains("Don't redo what's already done"), "{brief}");
}

#[test]
fn keeps_first_and_latest_asks_of_a_long_conversation() {
    let mut chat = ClaudeChat::new();
    let feed: String = (1..=10).map(|k| user(&format!("ask number {k}"))).collect();
    chat.feed(&feed);
    let brief = handoff_brief(&from("Bob", "Codex", "you asked"), Some((chat.items(), chat.meta())));
    assert!(brief.contains("(the first, then the latest 5 of 10)"), "{brief}");
    assert!(brief.contains("1. ask number 1\n2. ask number 6"), "{brief}");
    assert!(!brief.contains("ask number 5\n"), "{brief}");
}

#[test]
fn works_from_the_task_alone_when_unreadable() {
    let mut f = from("Cid", "Aider", "it hit its usage limit");
    f.title = Some("Fix the login redirect".to_string());
    let brief = handoff_brief(&f, None);
    assert!(brief.contains("Its task: Fix the login redirect"), "{brief}");
    assert!(brief.contains("look at the folder first"), "{brief}");
}

#[test]
fn adds_git_files_capped() {
    let mut f = from("Ana", "Claude Code", "it stopped");
    f.title = Some("T".to_string());
    f.git_files = vec!["a.ts".to_string(), "b.ts".to_string()];
    let brief = handoff_brief(&f, None);
    assert!(brief.contains("Changed on disk (git, 2 files)"), "{brief}");
    assert!(brief.contains("- a.ts"), "{brief}");
    let mut f = from("A", "C", "x");
    f.git_files = (0..40).map(|i| format!("f{i}.ts")).collect();
    let many = handoff_brief(&f, None);
    assert!(many.contains("and 10 more"), "{many}");
    assert!(!many.contains("f39.ts"), "{many}");
}
