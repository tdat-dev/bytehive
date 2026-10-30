// Muse conversations, through the public Chat API: feed envelope records,
// read prompts, steps with results, replies, and usage.

use serde_json::{json, Value};

use supervise::muse_chat::MuseChat;
use supervise::types::{Chat, ChatItem, StepItem, TodoState};

/// One envelope record, shaped like the real session log.
fn rec(id: &str, ms: u64, event: Value) -> String {
    json!({ "schema_version": 1, "id": id, "recorded_at": ms * 1000, "record_type": "event", "payload": { "kind": "run", "event": event } }).to_string()
}

fn step_of(item: &ChatItem) -> &StepItem {
    match item {
        ChatItem::Step(s) => s,
        _ => panic!("expected step"),
    }
}

#[test]
fn reads_prompts_steps_replies_and_usage() {
    let macro_lines = [
        rec("e1", 1000, json!({ "kind": "started", "prompt": "fix the bug" })),
        rec("e2", 1100, json!({ "kind": "started", "task_id": "t-1" })),
        rec("e3", 1200, json!({
            "kind": "assistant_tool_calls_committed", "message_id": "mm1",
            "tool_calls": [{ "id": "fc1", "call_id": "call1", "name": "read_file", "args": json!({ "path": "D:/app/a.ts" }).to_string() }],
        })),
        rec("e4", 1300, json!({
            "kind": "tool_result_batch_committed", "batch_id": "mm1",
            "results": [{ "tool_call_index": 0, "tool_call_id": "call1", "text": "const x = 1;\n" }],
        })),
        rec("e5", 1400, json!({ "kind": "assistant_message_committed", "message_id": "mm2", "text": "Found it." })),
        rec("e6", 1500, json!({ "kind": "assistant_message_committed", "message_id": "mm2", "text": "Found it, fixing." })),
        rec("e7", 1600, json!({ "kind": "model_completed", "usage": { "input_tokens": 100, "output_tokens": 20 }, "model": "muse-spark-x" })),
        rec("e7", 1600, json!({ "kind": "model_completed", "usage": { "input_tokens": 100, "output_tokens": 20 }, "model": "muse-spark-x" })),
    ];
    let mut chat = MuseChat::new();
    chat.feed(&macro_lines.join("\n"));
    let kinds: Vec<&str> = chat.items().iter().map(|i| i.kind()).collect();
    assert_eq!(kinds, vec!["user", "step", "text"]);
    match &chat.items()[0] {
        ChatItem::User(u) => {
            assert_eq!(u.text, "fix the bug");
            assert_eq!(u.at, 1000);
        }
        _ => panic!("expected user"),
    }
    let s = step_of(&chat.items()[1]);
    assert_eq!(s.verb, "Read");
    assert_eq!(s.target, "a.ts");
    assert_eq!(s.done, true);
    assert_eq!(s.output.as_deref(), Some("const x = 1;"));
    match &chat.items()[2] {
        ChatItem::Text(t) => assert_eq!(t.text, "Found it, fixing."),
        _ => panic!("expected text"),
    }
    assert_eq!(chat.items().len(), 3);
    assert_eq!(chat.meta().model.as_deref(), Some("muse-spark-x"));
    assert_eq!(chat.meta().context, 100);
    assert_eq!(chat.meta().output, 20);
    assert_eq!(chat.meta().turn, 1);
    assert_eq!(chat.meta().started, Some(1000));
}

#[test]
fn shows_mid_run_steers_but_not_background_deliveries() {
    let mut chat = MuseChat::new();
    chat.feed(&[
        rec("q1", 100, json!({ "kind": "inbox_item_queued", "source": { "source": "user_steer" }, "body": "wait", "payload": { "prompt": "hold on", "reasoning_effort": "max" } })),
        rec("q2", 200, json!({ "kind": "inbox_item_queued", "source": { "source": "cron" }, "body": "tick", "payload": { "prompt": "tick" } })),
    ].join("\n"));
    assert_eq!(chat.items().len(), 1);
    match &chat.items()[0] {
        ChatItem::User(u) => assert_eq!(u.text, "hold on"),
        _ => panic!("expected user"),
    }
    assert_eq!(chat.meta().effort.as_deref(), Some("max"));
}

#[test]
fn skips_results_for_calls_from_before_the_tail() {
    let mut chat = MuseChat::new();
    chat.feed(&rec("r1", 100, json!({
        "kind": "tool_result_batch_committed", "batch_id": "mm9",
        "results": [{ "tool_call_index": 0, "tool_call_id": "call-old", "text": "orphan" }],
    })));
    assert!(chat.items().is_empty());
}

#[test]
fn answers_asked_questions_when_the_prompt_settles() {
    let questions = json!([{ "id": "q1", "header": "NEXT", "question": "Which?", "options": [{ "label": "A", "description": "first" }, { "label": "B" }] }]);
    let mut chat = MuseChat::new();
    chat.feed(&[
        rec("c1", 100, json!({
            "kind": "assistant_tool_calls_committed", "message_id": "mm3",
            "tool_calls": [{ "id": "fc2", "call_id": "call2", "name": "request_user_input", "args": json!({ "questions": questions }).to_string() }],
        })),
        rec("s1", 200, json!({
            "kind": "user_input_prompt_settled", "prompt_id": "p1", "tool_call_id": "call2",
            "outcome": "answered", "answers": [{ "id": "q1", "selected_label": "B" }],
        })),
    ].join("\n"));
    let step = step_of(&chat.items()[0]);
    assert_eq!(step.verb, "Asked you");
    let questions = step.questions.clone().unwrap_or_default();
    assert_eq!(questions[0].answer.as_deref(), Some("B"));
}

#[test]
fn files_edits_and_keeps_the_plan() {
    let mut chat = MuseChat::new();
    chat.feed(&[
        rec("c1", 100, json!({
            "kind": "assistant_tool_calls_committed", "message_id": "mm4",
            "tool_calls": [{ "id": "fc3", "call_id": "call3", "name": "edit_file", "args": json!({ "path": "D:/app/b.ts", "find": "a", "replace": "b" }).to_string() }],
        })),
        rec("r1", 200, json!({
            "kind": "tool_result_batch_committed", "batch_id": "mm4",
            "results": [{ "tool_call_index": 0, "tool_call_id": "call3", "text": "edited" }],
        })),
        rec("t1", 300, json!({ "kind": "todo_snapshot_updated", "revision": 2, "items": [{ "text": "Ship", "status": "in_progress" }, { "text": "Mop", "status": "weird" }] })),
        "not json at all".to_string(),
    ].join("\n"));
    let files: Vec<(&str, u32, u32)> = chat.meta().files.iter().map(|f| (f.name.as_str(), f.added, f.removed)).collect();
    assert_eq!(files, vec![("b.ts", 1, 1)]);
    let todos = chat.meta().todos.clone().unwrap_or_default();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].state, TodoState::InProgress);
    assert_eq!(todos[1].state, TodoState::Pending);
}
