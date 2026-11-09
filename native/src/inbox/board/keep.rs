//! What the board keeps per project folder, under the keys kanban.ts gives
//! localStorage: `maestro.kanban.conductor.<folder>` (the conductor's mode)
//! and `maestro.kanban.planseen.<folder>` (plan tasks already imported), in
//! the data folder's kanban.json.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{Map, Value};

use crate::conductor::Mode;

fn store() -> Option<PathBuf> {
    Some(crate::fleet::data_dir()?.join("kanban.json"))
}

fn load() -> Map<String, Value> {
    store().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(key: String, value: Value) {
    let Some(p) = store() else { return };
    let mut all = load();
    all.insert(key, value);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, Value::Object(all).to_string());
}

fn conductor_key(dir: &str) -> String {
    format!("maestro.kanban.conductor.{dir}")
}

fn seen_key(dir: &str) -> String {
    format!("maestro.kanban.planseen.{dir}")
}

/// This folder's conductor mode, off until one is chosen.
pub(super) fn load_mode(dir: &str) -> Mode {
    load().get(&conductor_key(dir)).and_then(Value::as_str).and_then(Mode::from_key).unwrap_or_default()
}

pub(super) fn save_mode(dir: &str, mode: Mode) {
    save(conductor_key(dir), Value::from(mode.key()));
}

pub(super) fn load_seen(dir: &str) -> HashSet<String> {
    let all = load();
    let list = all.get(&seen_key(dir)).and_then(Value::as_array);
    list.into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect()
}

pub(super) fn save_seen(dir: &str, seen: &HashSet<String>) {
    let mut list: Vec<&String> = seen.iter().collect();
    list.sort();
    save(seen_key(dir), Value::from(list.into_iter().cloned().collect::<Vec<_>>()));
}
