//! The plan board: `.maestro/board.json`, where agents propose cards over
//! MCP (card_add) and you approve them by moving them to "To do". Edited as
//! raw JSON so fields this module does not know (labels, due, done evidence,
//! anything the MCP server adds later) survive untouched. Every change
//! re-reads the file first, so a card an agent wrote a second ago is not
//! lost, and writes go through a temp file and a rename.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// One card as the board view paints it.
#[derive(Debug, Clone, PartialEq)]
pub struct CardView {
    pub id: String,
    pub title: String,
    pub desc: String,
    /// (item id, text, done) per checklist item.
    pub checks: Vec<(String, String, bool)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListView {
    pub id: String,
    pub title: String,
    pub cards: Vec<CardView>,
}

/// Where a project's plan board lives.
pub fn board_path(project: &str) -> PathBuf {
    Path::new(project).join(".maestro").join("board.json")
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().to_string()
}

/// The lists and cards to paint. Missing pieces read as empty.
pub fn view(board: &Value) -> Vec<ListView> {
    board["lists"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|l| ListView {
            id: s(l, "id"),
            title: s(l, "title"),
            cards: l["cards"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| CardView {
                    id: s(c, "id"),
                    title: s(c, "title"),
                    desc: s(c, "desc"),
                    checks: c["checklist"].as_array().into_iter().flatten().map(|i| (s(i, "id"), s(i, "text"), i["done"].as_bool().unwrap_or(false))).collect(),
                })
                .collect(),
        })
        .collect()
}

/// Move a card to the end of another list. False when either is missing.
pub fn move_card(board: &mut Value, card_id: &str, to_list: &str) -> bool {
    let Some(lists) = board["lists"].as_array_mut() else { return false };
    if !lists.iter().any(|l| l["id"] == to_list) {
        return false;
    }
    let mut taken = None;
    for l in lists.iter_mut() {
        if let Some(cards) = l["cards"].as_array_mut() {
            if let Some(i) = cards.iter().position(|c| c["id"] == card_id) {
                taken = Some(cards.remove(i));
                break;
            }
        }
    }
    let Some(card) = taken else { return false };
    let target = lists.iter_mut().find(|l| l["id"] == to_list).expect("checked above");
    match target["cards"].as_array_mut() {
        Some(cards) => cards.push(card),
        None => target["cards"] = Value::Array(vec![card]),
    }
    true
}

/// Tick or untick one checklist item. False when it is not there.
pub fn set_check(board: &mut Value, card_id: &str, item_id: &str, done: bool) -> bool {
    for l in board["lists"].as_array_mut().into_iter().flatten() {
        for c in l["cards"].as_array_mut().into_iter().flatten() {
            if c["id"] != card_id {
                continue;
            }
            for i in c["checklist"].as_array_mut().into_iter().flatten() {
                if i["id"] == item_id {
                    i["done"] = Value::Bool(done);
                    return true;
                }
            }
        }
    }
    false
}

/// The list to approve into: "To do" by title, case and spacing aside.
pub fn todo_list(lists: &[ListView]) -> Option<&ListView> {
    lists.iter().find(|l| l.title.to_lowercase().replace([' ', '-'], "") == "todo")
}

pub fn load(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Write through a temp file and a rename, so a reader never sees half.
pub fn save(path: &Path, board: &Value) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(board).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("Could not write the board: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Could not save the board: {e}"))
}

/// Read the newest board, change it, write it back.
pub fn edit(path: &Path, change: impl FnOnce(&mut Value) -> bool) -> Result<(), String> {
    let mut board = load(path).ok_or_else(|| format!("No board at {}.", path.display()))?;
    if !change(&mut board) {
        return Err("That card or list is gone (the board changed). Look again.".to_string());
    }
    save(path, &board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn board() -> Value {
        json!({
            "version": 2,
            "lists": [
                { "id": "l1", "title": "Proposed", "cards": [{ "id": "c1", "title": "Fix login", "desc": "d", "labels": ["red"], "due": null, "checklist": [{ "id": "i1", "text": "test", "done": false }] }] },
                { "id": "l2", "title": "To do", "cards": [] },
                { "id": "l3", "title": "Done" }
            ]
        })
    }

    #[test]
    fn reads_lists_cards_and_checklists() {
        let v = view(&board());
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].cards[0].title, "Fix login");
        assert_eq!(v[0].cards[0].checks, [("i1".to_string(), "test".to_string(), false)]);
        assert!(v[2].cards.is_empty(), "a list with no cards key reads empty");
        assert_eq!(todo_list(&v).map(|l| l.id.as_str()), Some("l2"));
    }

    #[test]
    fn moving_keeps_every_field_of_the_card() {
        let mut b = board();
        assert!(move_card(&mut b, "c1", "l2"));
        assert_eq!(b["lists"][0]["cards"].as_array().unwrap().len(), 0);
        let moved = &b["lists"][1]["cards"][0];
        assert_eq!(moved["labels"], json!(["red"]), "unknown fields survive");
        assert!(move_card(&mut b, "c1", "l3"), "into a list that had no cards key");
        assert!(!move_card(&mut b, "nope", "l2"));
        assert!(!move_card(&mut b, "c1", "nolist"));
    }

    #[test]
    fn ticks_a_checklist_item() {
        let mut b = board();
        assert!(set_check(&mut b, "c1", "i1", true));
        assert_eq!(b["lists"][0]["cards"][0]["checklist"][0]["done"], json!(true));
        assert!(!set_check(&mut b, "c1", "zz", true));
    }

    #[test]
    fn edits_go_through_the_file() {
        let dir = std::env::temp_dir().join(format!("maestro-plan-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".maestro")).unwrap();
        let path = board_path(&dir.to_string_lossy());
        save(&path, &board()).unwrap();
        edit(&path, |b| move_card(b, "c1", "l2")).unwrap();
        assert_eq!(view(&load(&path).unwrap())[1].cards.len(), 1);
        assert!(edit(&path, |b| move_card(b, "gone", "l2")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
