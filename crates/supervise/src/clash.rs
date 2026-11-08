//! Early warnings while agents still work: two agents changing the same
//! files, or two agents on the same task in one project. Pure: feeds on file
//! lists already polled, no extra git calls. A race (several agents given the
//! same job on purpose) is not a dupe.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::task_state::TaskState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClashKind {
    File,
    Task,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClashNote {
    pub kind: ClashKind,
    /// Stable identity for change detection.
    pub key: String,
    /// Names of the other agents involved, sorted.
    pub with: Vec<String>,
    /// Shared paths in the owner's own spelling (file clashes).
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClashInput {
    pub pane_id: String,
    pub name: String,
    pub ws_id: String,
    pub state: TaskState,
    pub running: bool,
    pub title: Option<String>,
    /// Race id when it runs a job shared on purpose, else none.
    pub race: Option<String>,
    /// Changed paths, relative to the repo.
    pub paths: Vec<String>,
}

fn norm_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

fn spaces_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").unwrap())
}

fn norm_title(t: &str) -> String {
    spaces_re().replace_all(&t.trim().to_lowercase(), " ").to_string()
}

/// Agents that can still surprise each other: stopped ones are frozen and
/// merged work is already in, so only running, unmerged agents count.
fn live<'a>(inputs: &'a [ClashInput]) -> Vec<&'a ClashInput> {
    inputs.iter().filter(|i| i.running && i.state != TaskState::Merged).collect()
}

fn file_notes(inputs: &[ClashInput]) -> HashMap<String, ClashNote> {
    let mut out: HashMap<String, ClashNote> = HashMap::new();
    let mut by_ws: HashMap<&str, Vec<&ClashInput>> = HashMap::new();
    for i in live(inputs) {
        by_ws.entry(i.ws_id.as_str()).or_default().push(i);
    }
    for group in by_ws.values() {
        // normalized path -> panes touching it, in first-touch order
        let mut touches: HashMap<String, Vec<&ClashInput>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for i in group {
            let mut seen = HashSet::new();
            for p in i.paths.iter().map(|p| norm_path(p)) {
                if !seen.insert(p.clone()) {
                    continue;
                }
                touches.entry(p.clone()).or_default().push(*i);
                if !order.contains(&p) {
                    order.push(p);
                }
            }
        }
        let shared: HashSet<&str> = touches.iter().filter(|(_, who)| who.len() > 1).map(|(p, _)| p.as_str()).collect();
        if shared.is_empty() {
            continue;
        }
        for i in group {
            let mine: HashSet<String> = i.paths.iter().map(|p| norm_path(p)).collect();
            let hit: Vec<&str> = order.iter().map(String::as_str).filter(|p| mine.contains(*p) && shared.contains(*p)).collect();
            if hit.is_empty() {
                continue;
            }
            let mut others: Vec<&ClashInput> = Vec::new();
            for p in &hit {
                for o in &touches[*p] {
                    if o.pane_id != i.pane_id && !others.iter().any(|x| x.pane_id == o.pane_id) {
                        others.push(*o);
                    }
                }
            }
            let mut with_names: Vec<String> = others.iter().map(|o| o.name.clone()).collect();
            with_names.sort();
            with_names.dedup();
            let mut own: HashMap<String, &str> = HashMap::new();
            for p in &i.paths {
                own.insert(norm_path(p), p.as_str());
            }
            let files: Vec<String> = hit.iter().filter_map(|p| own.get(*p).map(|s| s.to_string())).take(20).collect();
            let mut ids: Vec<&str> = vec![i.pane_id.as_str()];
            ids.extend(others.iter().map(|o| o.pane_id.as_str()));
            ids.sort();
            let mut hit_sorted = hit.clone();
            hit_sorted.sort();
            out.insert(i.pane_id.clone(), ClashNote { kind: ClashKind::File, key: format!("file:{}:{}", ids.join("+"), hit_sorted.join("+")), with: with_names, files });
        }
    }
    out
}

fn task_notes(inputs: &[ClashInput]) -> HashMap<String, ClashNote> {
    let mut out: HashMap<String, ClashNote> = HashMap::new();
    let mut by_ws_title: HashMap<String, Vec<&ClashInput>> = HashMap::new();
    for i in live(inputs) {
        let t = i.title.as_deref().unwrap_or("").trim();
        if t.is_empty() {
            continue;
        }
        by_ws_title.entry(format!("{}\u{2016}{}", i.ws_id, norm_title(t))).or_default().push(i);
    }
    for group in by_ws_title.values() {
        if group.len() < 2 {
            continue;
        }
        let title = norm_title(group[0].title.as_deref().unwrap_or(""));
        for i in group {
            // same race, same job on purpose
            let others: Vec<&&ClashInput> = group.iter().filter(|o| o.pane_id != i.pane_id && !(i.race.is_some() && o.race == i.race)).collect();
            if others.is_empty() {
                continue;
            }
            let mut with_names: Vec<String> = others.iter().map(|o| o.name.clone()).collect();
            with_names.sort();
            with_names.dedup();
            let mut ids: Vec<&str> = vec![i.pane_id.as_str()];
            ids.extend(others.iter().map(|o| o.pane_id.as_str()));
            ids.sort();
            out.insert(i.pane_id.clone(), ClashNote { kind: ClashKind::Task, key: format!("task:{}:{title}", ids.join("+")), with: with_names, files: Vec::new() });
        }
    }
    out
}

/// One early-warning note per agent: a file clash wins over a task dupe,
/// concrete files beat a bare same title.
pub fn clash_notes(inputs: &[ClashInput]) -> HashMap<String, ClashNote> {
    let mut out = task_notes(inputs);
    for (id, note) in file_notes(inputs) {
        out.insert(id, note);
    }
    out
}

fn capped(items: &[String]) -> String {
    let head: Vec<&str> = items.iter().take(2).map(String::as_str).collect();
    if items.len() > 2 {
        format!("{} +{} more", head.join(", "), items.len() - 2)
    } else {
        head.join(", ")
    }
}

/// Row suffix ("overlaps Bo in src/app.ts", "same task as Bo").
pub fn clash_short(note: &ClashNote) -> String {
    if note.kind == ClashKind::File {
        format!("overlaps {} in {}", capped(&note.with), capped(&note.files))
    } else {
        format!("same task as {}", capped(&note.with))
    }
}

/// History line: the same, capitalized.
pub fn clash_history(note: &ClashNote) -> String {
    let s = clash_short(note);
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
