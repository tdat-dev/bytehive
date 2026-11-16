//! An OpenCode session as chat items for the Remote store, from the events
//! agent_core::opencode_events reads out of its database.

use agent_core::opencode_events::{Event, session_events};
use serde_json::Value;

use crate::transcript::Item;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn first_line(t: &str) -> String {
    let line = t.trim().lines().next().unwrap_or("");
    if line.chars().count() <= 120 { line.to_string() } else { format!("{}\u{2026}", line.chars().take(119).collect::<String>()) }
}

fn base_name(p: &str) -> String {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// How a tool call reads: (verb, target, target is code).
fn describe(name: &str, input: &Value) -> (String, String, bool) {
    let file = || base_name(if s(input, "filePath").is_empty() { s(input, "path") } else { s(input, "filePath") });
    let (verb, target, code) = match name {
        "bash" | "shell" => ("Ran", first_line(s(input, "command")), true),
        "edit" | "multiedit" | "patch" | "apply_patch" => ("Edited", file(), true),
        "write" => ("Wrote", file(), true),
        "read" => ("Read", file(), true),
        "grep" => ("Searched for", first_line(s(input, "pattern")), true),
        "glob" | "list" => ("Looked for", first_line(if s(input, "pattern").is_empty() { s(input, "path") } else { s(input, "pattern") }), true),
        "webfetch" => ("Read", s(input, "url").to_string(), false),
        "websearch" => ("Searched the web for", s(input, "query").to_string(), false),
        "todowrite" | "todoread" => ("Updated the plan", String::new(), false),
        "question" => ("Asked you", "a question".into(), false),
        "task" => ("Asked a helper agent to", first_line(s(input, "description")), false),
        _ => return ("Used".into(), name.replace('_', " "), false),
    };
    (verb.to_string(), target, code)
}

pub fn items(events: Vec<Event>) -> Vec<Item> {
    events
        .into_iter()
        .enumerate()
        .map(|(n, e)| {
            let id = format!("o{n}");
            match e {
                Event::User(text) => Item::User { id, text },
                Event::Text(text) => Item::Text { id, text },
                Event::Tool { name, input, failed, done } => {
                    let (verb, target, code) = describe(&name, &input);
                    Item::Step { id, verb, target, error: failed, done, code }
                }
            }
        })
        .collect()
}

/// An OpenCode session's items, from its database.
pub fn read(session: &str) -> Option<Vec<Item>> {
    let db = agent_core::opencode::db_path()?;
    Some(items(session_events(&db, session)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_events_into_chat_items() {
        let ev = vec![
            Event::User("fix it".into()),
            Event::Tool { name: "bash".into(), input: serde_json::json!({ "command": "npm test\n--watch=false" }), failed: true, done: true },
            Event::Tool { name: "edit".into(), input: serde_json::json!({ "filePath": "D:\\p\\src\\a.ts" }), failed: false, done: false },
            Event::Tool { name: "maestro_board_get".into(), input: Value::Null, failed: false, done: true },
            Event::Text("Fixed.".into()),
        ];
        let got = items(ev);
        assert_eq!(got[0], Item::User { id: "o0".into(), text: "fix it".into() });
        assert_eq!(got[1], Item::Step { id: "o1".into(), verb: "Ran".into(), target: "npm test".into(), error: true, done: true, code: true });
        assert_eq!(got[2], Item::Step { id: "o2".into(), verb: "Edited".into(), target: "a.ts".into(), error: false, done: false, code: true });
        assert_eq!(got[3], Item::Step { id: "o3".into(), verb: "Used".into(), target: "maestro board get".into(), error: false, done: true, code: false });
        assert_eq!(got[4], Item::Text { id: "o4".into(), text: "Fixed.".into() });
    }

    /// A real session on this machine: OC_SESSION=ses_... cargo test cloud::opencode -- --ignored
    #[test]
    #[ignore]
    fn reads_a_real_session() {
        let id = std::env::var("OC_SESSION").expect("set OC_SESSION to an OpenCode session id");
        let got = read(&id).expect("no OpenCode database here");
        assert!(got.iter().any(|i| matches!(i, Item::User { .. })), "nothing read from {id}");
        for i in got.iter().take(12) {
            eprintln!("{i:?}");
        }
    }
}
