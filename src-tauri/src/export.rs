//! The hand-over to the native app: what this app keeps in the WebView's
//! storage (the fleet, settings, recent folders) written to
//! `~/.bytehive/export/web-state.json`, which the native app imports once.
//! The native app cannot read WebView2's storage itself.

use std::path::PathBuf;

use crate::error::CommandError;

fn export_file() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".bytehive").join("export").join("web-state.json"))
}

/// Write the state, whole or not at all: a half-written file never stands.
#[tauri::command]
pub fn export_web_state(state: String) -> Result<(), CommandError> {
    serde_json::from_str::<serde_json::Value>(&state).map_err(|e| CommandError::Failed(format!("not JSON: {e}")))?;
    let path = export_file().ok_or_else(|| CommandError::Failed("no home folder".into()))?;
    write_whole(&path, &state).map_err(|e| CommandError::Failed(format!("write {}: {e}", path.display())))
}

fn write_whole(path: &std::path::Path, body: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::write_whole;

    #[test]
    fn writes_the_whole_file_in_place() {
        let dir = std::env::temp_dir().join(format!("bytehive-export-{}", std::process::id()));
        let path = dir.join("export").join("web-state.json");
        write_whole(&path, "{\"a\":1}").unwrap();
        write_whole(&path, "{\"a\":2}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
