//! Data from before the rename (Maestro) moves to its ByteHive place, once.

use std::path::{Path, PathBuf};

/// A project from before the rename keeps its board, memory, fleet and
/// recordings in `.maestro`: it becomes `.bytehive`, unless one is there.
#[tauri::command]
pub fn adopt_old_folder(project: String) {
    adopt_folder(Path::new(&project));
}

fn adopt_folder(project: &Path) {
    let (old, now) = (project.join(".maestro"), project.join(".bytehive"));
    if old.is_dir() && !now.exists() {
        let _ = std::fs::rename(old, now);
    }
}

/// `~/.bytehive` from `~/.maestro` the first time ByteHive runs. Copied, not
/// moved, since an older Maestro may still be installed beside it.
pub fn adopt_old_home() {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else { return };
    let (old, now) = (PathBuf::from(&home).join(".maestro"), PathBuf::from(&home).join(".bytehive"));
    if old.is_dir() && !now.exists() {
        let _ = copy_dir(&old, &now);
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bytehive-adopt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".maestro")).unwrap();
        std::fs::write(dir.join(".maestro").join("board.json"), "old").unwrap();
        dir
    }

    #[test]
    fn an_old_folder_moves_over() {
        let dir = project("moves");
        adopt_folder(&dir);
        assert!(!dir.join(".maestro").exists());
        assert_eq!(std::fs::read_to_string(dir.join(".bytehive").join("board.json")).unwrap(), "old");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_existing_bytehive_folder_is_left_alone() {
        let dir = project("keeps");
        std::fs::create_dir_all(dir.join(".bytehive")).unwrap();
        std::fs::write(dir.join(".bytehive").join("board.json"), "new").unwrap();
        adopt_folder(&dir);
        assert_eq!(std::fs::read_to_string(dir.join(".bytehive").join("board.json")).unwrap(), "new");
        assert!(dir.join(".maestro").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
