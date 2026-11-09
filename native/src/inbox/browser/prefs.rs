//! "Ask me before an agent sends, posts, pays or deletes": on unless turned
//! off (askRisky in browserprefs.ts). Kept on its own like the old app's
//! `maestro.browserAsk`, in the data folder's browser.json; until that file
//! exists, the old app's choice is read from its hand-over export.

use std::path::PathBuf;

use serde_json::{Value, json};

fn store() -> Option<PathBuf> {
    Some(crate::fleet::data_dir()?.join("browser.json"))
}

/// The choice in browser.json, or None when there is none yet.
pub fn ask_in(raw: &str) -> Option<bool> {
    serde_json::from_str::<Value>(raw).ok()?.get("askRisky")?.as_bool()
}

/// The old app's choice in its export: off only when it says "0".
pub fn ask_in_export(raw: &str) -> Option<bool> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let s = v.get("storage")?.get("maestro.browserAsk")?.as_str()?;
    Some(s != "0")
}

pub fn load_ask() -> bool {
    if let Some(on) = store().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|r| ask_in(&r)) {
        return on;
    }
    crate::import::export_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|r| ask_in_export(&r)).unwrap_or(true)
}

pub fn save_ask(on: bool) {
    let Some(p) = store() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, json!({ "askRisky": on }).to_string());
}

#[cfg(test)]
mod tests {
    use super::{ask_in, ask_in_export};

    #[test]
    fn reads_the_choice_and_the_old_apps() {
        assert_eq!(ask_in(r#"{"askRisky":false}"#), Some(false));
        assert_eq!(ask_in("{}"), None);
        assert_eq!(ask_in("broken"), None);
        assert_eq!(ask_in_export(r#"{"format":1,"storage":{"maestro.browserAsk":"0"}}"#), Some(false));
        assert_eq!(ask_in_export(r#"{"format":1,"storage":{"maestro.browserAsk":"1"}}"#), Some(true));
        assert_eq!(ask_in_export(r#"{"format":1,"storage":{}}"#), None);
    }
}
