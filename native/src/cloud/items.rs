//! A session as the store keeps it: its chat items cut into segments of 200,
//! and a header. remote-web/src/store/format.ts reads the same shapes.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::transcript::Item;

/// Items per segment.
pub const SEGMENT: usize = 200;

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Wire<'a> {
    User { id: &'a str, text: &'a str },
    Text { id: &'a str, text: &'a str },
    Step { id: &'a str, verb: &'a str, target: &'a str, code: bool, error: bool, done: bool },
}

fn wire(i: &Item) -> Wire<'_> {
    match i {
        Item::User { id, text } => Wire::User { id, text },
        Item::Text { id, text } => Wire::Text { id, text },
        Item::Step { id, verb, target, error, done, code } => Wire::Step { id, verb, target, code: *code, error: *error, done: *done },
    }
}

/// The items as JSON, one string per segment.
pub fn segments(items: &[Item]) -> Vec<String> {
    items
        .chunks(SEGMENT)
        .map(|c| serde_json::to_string(&c.iter().map(wire).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into()))
        .collect()
}

/// A short fingerprint of a segment or header, to send only what changed.
pub fn digest(s: &str) -> String {
    Sha256::digest(s.as_bytes())[..12].iter().map(|b| format!("{b:02x}")).collect()
}

/// A session's header, sealed in slot `<sid>/meta`.
#[derive(Serialize, Debug, PartialEq)]
pub struct Meta {
    pub title: String,
    pub cli: String,
    pub project: String,
    pub machine: String,
    pub agent: String,
    pub started: u64,
    pub updated: u64,
    pub items: usize,
}

/// What the session was asked first, on one line, as its title.
pub fn title(items: &[Item]) -> String {
    let first = items.iter().find_map(|i| match i {
        Item::User { text, .. } if !text.trim().is_empty() => Some(text.as_str()),
        _ => None,
    });
    let line = first.unwrap_or("").lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() <= 80 {
        return line.to_string();
    }
    let cut: String = line.chars().take(80).collect();
    format!("{}\u{2026}", cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, text: &str) -> Item {
        Item::User { id: id.into(), text: text.into() }
    }

    #[test]
    fn writes_items_the_way_the_web_reads_them() {
        let items = vec![
            user("c0", "Fix the build"),
            Item::Text { id: "c1".into(), text: "On it.".into() },
            Item::Step { id: "c2".into(), verb: "Ran".into(), target: "cargo test".into(), error: true, done: true, code: true },
        ];
        assert_eq!(
            segments(&items),
            vec![r#"[{"kind":"user","id":"c0","text":"Fix the build"},{"kind":"text","id":"c1","text":"On it."},{"kind":"step","id":"c2","verb":"Ran","target":"cargo test","code":true,"error":true,"done":true}]"#]
        );
    }

    #[test]
    fn cuts_long_sessions_into_segments_of_200() {
        let items: Vec<Item> = (0..450).map(|n| user(&format!("c{n}"), "x")).collect();
        assert_eq!(segments(&items).len(), 3);
        assert!(segments(&[]).is_empty());
    }

    #[test]
    fn titles_a_session_by_its_first_ask() {
        assert_eq!(title(&[Item::Text { id: "a".into(), text: "hi".into() }, user("b", "\n  Add a pricing page\nwith three plans")]), "Add a pricing page");
        let long = "word ".repeat(30);
        let t = title(&[user("a", &long)]);
        assert!(t.ends_with('\u{2026}') && t.chars().count() <= 81, "{t}");
        assert_eq!(title(&[]), "");
    }

    #[test]
    fn a_changed_segment_has_another_digest() {
        assert_eq!(digest("a"), digest("a"));
        assert_ne!(digest("a"), digest("b"));
        assert_eq!(digest("a").len(), 24);
    }
}
