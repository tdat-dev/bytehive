//! The project board (board.ts, boardfile.ts, kanban.ts in today's app; the
//! bytehive-mcp server's board.ts and ops.ts): lists of cards in
//! `<project>/.bytehive/board.json`. Agents change the same file through the
//! MCP tools, so it is read and written exactly as they do: the same
//! normalising and the same `JSON.stringify(.., null, 2)` bytes; file.rs
//! reads and writes it, blocking on the disk: call that off the UI thread.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChecklistItem {
    pub id: String,
    pub text: String,
    pub done: bool,
}

/// Evidence kept when a task is finished: which files changed and a note.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DoneInfo {
    #[serde(rename = "repoRoot")]
    pub repo_root: String,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(serialize_with = "js_number")]
    pub at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

/// Fields in the order normalizeCard builds them, so the bytes match.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub desc: String,
    pub labels: Vec<String>,
    /// yyyy-mm-dd, or null.
    pub due: Option<String>,
    pub checklist: Vec<ChecklistItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(rename = "assigneeId", skip_serializing_if = "Option::is_none")]
    pub assignee_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "js_opt_number")]
    pub returns: Option<f64>,
    /// Its Job was stopped: nobody works it any more.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stopped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<DoneInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct List {
    pub id: String,
    pub title: String,
    pub cards: Vec<Card>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub lists: Vec<List>,
}

/// Trello-flavoured labels: key, colour, name (LABELS in board.ts).
pub const LABELS: [(&str, u32, &str); 6] = [
    ("green", 0x3ad29f, "Green"),
    ("yellow", 0xe6c84a, "Yellow"),
    ("orange", 0xe0913a, "Orange"),
    ("red", 0xff6b6b, "Red"),
    ("purple", 0xb18cf0, "Purple"),
    ("blue", 0x5ec2f0, "Blue"),
];

pub fn label_hex(key: &str) -> u32 {
    LABELS.iter().find(|l| l.0 == key).map_or(0x7d8b99, |l| l.1)
}

/// A number as JSON.stringify writes it: whole numbers without ".0".
fn js_number<S: Serializer>(n: &f64, s: S) -> Result<S::Ok, S::Error> {
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 { s.serialize_i64(*n as i64) } else { s.serialize_f64(*n) }
}

fn js_opt_number<S: Serializer>(n: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    js_number(&n.unwrap_or(0.0), s)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// `${prefix}${Date.now().toString(36)}${seq}`, as both sides make ids.
pub fn uid(prefix: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    format!("{prefix}{}{seq}", base36(now_ms()))
}

pub fn mk_card(title: &str) -> Card {
    Card { id: uid("c"), title: title.to_string(), desc: String::new(), labels: Vec::new(), due: None, checklist: Vec::new(), assignee: None, job: None, assignee_id: None, returns: None, stopped: false, done: None }
}

pub fn default_board() -> Board {
    let list = |title: &str| List { id: uid("l"), title: title.to_string(), cards: Vec::new() };
    Board { lists: vec![list("To do"), list("Doing"), list("Done")] }
}

fn text<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

fn filled(o: &Map<String, Value>, key: &str) -> Option<String> {
    text(o, key).filter(|s| !s.is_empty()).map(str::to_string)
}

fn normalize_card(c: &Value) -> Card {
    let empty = Map::new();
    let o = c.as_object().unwrap_or(&empty);
    let checklist = o
        .get("checklist")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let i = i.as_object()?;
            Some(ChecklistItem { id: filled(i, "id").unwrap_or_else(|| uid("i")), text: text(i, "text")?.to_string(), done: i.get("done") == Some(&Value::Bool(true)) })
        })
        .collect();
    let done = o.get("done").and_then(Value::as_object).and_then(|d| {
        let files = d.get("files")?.as_array()?;
        Some(DoneInfo {
            repo_root: text(d, "repoRoot")?.to_string(),
            files: files.iter().filter_map(|f| f.as_str().map(str::to_string)).collect(),
            summary: text(d, "summary").map(str::to_string),
            at: d.get("at").and_then(Value::as_f64).unwrap_or(0.0),
            by: text(d, "by").filter(|s| !s.trim().is_empty()).map(str::to_string),
        })
    });
    Card {
        id: filled(o, "id").unwrap_or_else(|| uid("c")),
        title: text(o, "title").unwrap_or("").to_string(),
        desc: text(o, "desc").unwrap_or("").to_string(),
        labels: o.get("labels").and_then(Value::as_array).into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect(),
        due: text(o, "due").map(str::to_string),
        checklist,
        assignee: text(o, "assignee").filter(|s| !s.trim().is_empty()).map(str::to_string),
        job: filled(o, "job"),
        assignee_id: filled(o, "assigneeId"),
        returns: o.get("returns").and_then(Value::as_f64).filter(|n| *n > 0.0),
        stopped: o.get("stopped") == Some(&Value::Bool(true)),
        done,
    }
}

/// A board from the file's text, normalised as the MCP server does. Err when
/// it is not JSON or has no "lists" array: such a file is never written over
/// unless you change the board yourself.
pub fn parse(raw: &str) -> Result<Board, String> {
    let data: Value = serde_json::from_str(raw).map_err(|_| "board.json is not valid JSON".to_string())?;
    let lists = data.get("lists").and_then(Value::as_array).ok_or_else(|| "board.json has no lists array".to_string())?;
    let empty = Map::new();
    Ok(Board {
        lists: lists
            .iter()
            .map(|l| {
                let o = l.as_object().unwrap_or(&empty);
                List {
                    id: filled(o, "id").unwrap_or_else(|| uid("l")),
                    title: text(o, "title").unwrap_or("List").to_string(),
                    cards: o.get("cards").and_then(Value::as_array).into_iter().flatten().map(normalize_card).collect(),
                }
            })
            .collect(),
    })
}

/// `JSON.stringify({ version: 2, lists }, null, 2)`, no newline at the end.
pub fn serialize(board: &Board) -> String {
    #[derive(Serialize)]
    struct File<'a> {
        version: u8,
        lists: &'a [List],
    }
    serde_json::to_string_pretty(&File { version: 2, lists: &board.lists }).expect("board serializes")
}

/// Where a card sits: its list's index and its own.
pub fn find_card(board: &Board, id: &str) -> Option<(usize, usize)> {
    board.lists.iter().enumerate().find_map(|(l, list)| list.cards.iter().position(|c| c.id == id).map(|c| (l, c)))
}

pub fn card_mut<'a>(board: &'a mut Board, id: &str) -> Option<&'a mut Card> {
    board.lists.iter_mut().flat_map(|l| l.cards.iter_mut()).find(|c| c.id == id)
}

fn is(title: &str, word: &str) -> bool {
    title.trim().to_lowercase() == word
}

/// Ids of every card in a list titled "Done", in any case.
pub fn done_card_ids(board: &Board) -> Vec<String> {
    board.lists.iter().filter(|l| is(&l.title, "done")).flat_map(|l| l.cards.iter().map(|c| c.id.clone())).collect()
}

/// The list with this title, made first on the board when there is none
/// (findOrCreateListIn: Proposed sits first).
pub fn find_or_create_list(board: &mut Board, title: &str) -> usize {
    if let Some(i) = board.lists.iter().position(|l| l.title.to_lowercase() == title.to_lowercase()) {
        return i;
    }
    board.lists.insert(0, List { id: uid("l"), title: title.to_string(), cards: Vec::new() });
    0
}

pub fn add_list(board: &mut Board, title: &str) {
    let t = title.trim();
    if !t.is_empty() {
        board.lists.push(List { id: uid("l"), title: t.to_string(), cards: Vec::new() });
    }
}

pub fn rename_list(board: &mut Board, id: &str, title: &str) {
    let t = title.trim();
    if let Some(l) = board.lists.iter_mut().find(|l| l.id == id).filter(|_| !t.is_empty()) {
        l.title = t.to_string();
    }
}

pub fn delete_list(board: &mut Board, id: &str) {
    board.lists.retain(|l| l.id != id);
}

pub fn add_card(board: &mut Board, list: &str, title: &str) {
    let t = title.trim();
    if let Some(l) = board.lists.iter_mut().find(|l| l.id == list).filter(|_| !t.is_empty()) {
        l.cards.push(mk_card(t));
    }
}

pub fn delete_card(board: &mut Board, id: &str) {
    if let Some((l, c)) = find_card(board, id) {
        board.lists[l].cards.remove(c);
    }
}

/// Move a card into a list, before another card or at its end. The card or
/// the list gone (an agent deleted it) leaves the board as it is.
pub fn move_card(board: &mut Board, id: &str, to: &str, before: Option<&str>) {
    let Some((l, c)) = find_card(board, id) else { return };
    if !board.lists.iter().any(|x| x.id == to) || before == Some(id) {
        return;
    }
    let card = board.lists[l].cards.remove(c);
    let target = board.lists.iter_mut().find(|x| x.id == to).expect("checked above");
    let at = before.and_then(|b| target.cards.iter().position(|x| x.id == b)).unwrap_or(target.cards.len());
    target.cards.insert(at, card);
}

/// Move a list before another one, or to the end.
pub fn move_list(board: &mut Board, id: &str, before: Option<&str>) {
    if before == Some(id) {
        return;
    }
    let Some(from) = board.lists.iter().position(|l| l.id == id) else { return };
    let list = board.lists.remove(from);
    let at = before.and_then(|b| board.lists.iter().position(|l| l.id == b)).unwrap_or(board.lists.len());
    board.lists.insert(at, list);
}

/// Record who a card went to, and pull it into Doing (assignCard).
pub fn assign_card(board: &mut Board, id: &str, name: Option<&str>, to_doing: bool) {
    let Some((l, c)) = find_card(board, id) else { return };
    board.lists[l].cards[c].assignee = name.map(str::to_string);
    if to_doing && board.lists[l].title.to_lowercase() != "doing" {
        let card = board.lists[l].cards.remove(c);
        let doing = find_or_create_list(board, "Doing");
        board.lists[doing].cards.push(card);
    }
}

/// A due date as the field takes it: yyyy-mm-dd.
pub fn valid_due(s: &str) -> bool {
    s.len() == 10 && NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

/// A card's due badge: "Nov 9", and whether it is past or due within a day.
pub fn fmt_due(iso: &str, today: NaiveDate) -> (String, bool, bool) {
    match NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        Ok(d) => {
            let diff = (d - today).num_days();
            (d.format("%b %-d").to_string(), diff < 0, (0..=1).contains(&diff))
        }
        Err(_) => ("Invalid Date".to_string(), false, false),
    }
}

/// The task typed into an agent: title, description, open checklist items.
pub fn card_to_agent_text(card: &Card) -> String {
    let mut lines = vec![format!("Task: {}", card.title.trim())];
    let desc = card.desc.trim();
    if !desc.is_empty() {
        lines.push(String::new());
        lines.push(desc.to_string());
    }
    let todo: Vec<&ChecklistItem> = card.checklist.iter().filter(|i| !i.done).collect();
    if !todo.is_empty() {
        lines.push(String::new());
        lines.extend(todo.iter().map(|i| format!("- [ ] {}", i.text)));
    }
    lines.join("\n")
}

/// The task plus how to keep the board in step (dispatchPrompt).
pub fn dispatch_prompt(card: &Card) -> String {
    let r = serde_json::to_string(card.title.trim()).unwrap_or_default();
    format!(
        "{}\n\nWhen you start, move this card to \"Doing\" with the bytehive MCP tool card_move (card: {r}). When finished, call card_done (card: {r}) with a one-line summary. If you don't have bytehive tools, just do the task.",
        card_to_agent_text(card)
    )
}

/// The Markdown mirror agents without the MCP tools read (.bytehive/board.md).
pub fn board_to_markdown(board: &Board) -> String {
    let mut out = vec!["# Board".to_string(), String::new()];
    for list in &board.lists {
        out.push(format!("## {} ({})", list.title, list.cards.len()));
        out.push(String::new());
        if list.cards.is_empty() {
            out.push("_(empty)_".to_string());
            out.push(String::new());
            continue;
        }
        for c in &list.cards {
            out.push(format!("- [{}] {}", if c.done.is_some() { "x" } else { " " }, c.title.trim()));
            let desc = c.desc.trim();
            if !desc.is_empty() {
                out.push(format!("  {}", desc.replace('\n', "\n  ")));
            }
            out.extend(c.checklist.iter().map(|i| format!("  - [{}] {}", if i.done { "x" } else { " " }, i.text)));
        }
        out.push(String::new());
    }
    out.join("\n")
}

mod file;
pub mod round;

pub use file::{change, mtime, read, save, write_mirror};
#[cfg(test)]
use file::{ReadError, board_path};

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
