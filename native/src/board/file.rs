//! board.json on disk (boardfile.ts in today's app, board.ts in bytehive-mcp):
//! read forgivingly, written whole through a temp file, and a change applied
//! to a fresh read so an agent's write in between is never lost.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{Board, board_to_markdown, parse, serialize};

pub fn board_path(dir: &str) -> PathBuf {
    Path::new(dir).join(".bytehive").join("board.json")
}

pub fn mtime(dir: &str) -> Option<SystemTime> {
    std::fs::metadata(board_path(dir)).and_then(|m| m.modified()).ok()
}

/// Why the file could not be used.
#[derive(Debug, PartialEq)]
pub enum ReadError {
    /// Bad JSON or no lists: shown as a fresh board, replaced only by your change.
    Corrupt(String),
    /// Could not be read at all: never written over.
    Unreadable(String),
}

/// The board and its file's time, or None when there is no file yet.
pub fn read(dir: &str) -> Result<Option<(Board, Option<SystemTime>)>, ReadError> {
    let path = board_path(dir);
    let raw = |p: &Path| match std::fs::read_to_string(p) {
        Ok(raw) => Ok(Some(raw)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ReadError::Unreadable(e.to_string())),
    };
    // The time first: a write landing after it reads as changed next time.
    let at = mtime(dir);
    let Some(text) = raw(&path)? else { return Ok(None) };
    match parse(&text) {
        Ok(b) => Ok(Some((b, at))),
        // The old app writes in place: give a torn read one short retry.
        Err(_) => {
            std::thread::sleep(Duration::from_millis(50));
            let at = mtime(dir);
            let Some(text) = raw(&path)? else { return Ok(None) };
            parse(&text).map(|b| Some((b, at))).map_err(ReadError::Corrupt)
        }
    }
}

/// Write the whole file through a temp file, as the MCP server does.
pub fn save(dir: &str, board: &Board) -> Result<Option<SystemTime>, String> {
    let path = board_path(dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serialize(board)).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(mtime(dir))
}

/// The Markdown mirror, kept in step with every change; failing is harmless.
pub fn write_mirror(dir: &str, board: &Board) {
    let path = Path::new(dir).join(".bytehive").join("board.md");
    let _ = std::fs::create_dir_all(Path::new(dir).join(".bytehive"));
    let _ = std::fs::write(path, board_to_markdown(board));
}

/// Read, change, write (withBoard): the change lands on the file as it is
/// now, and when an agent wrote between the read and the write it is redone
/// once on the agent's copy. With no file, or a corrupt one, `fallback` (the
/// board on screen) is changed and written.
pub fn change(dir: &str, fallback: &Board, f: &dyn Fn(&mut Board)) -> Result<(Board, Option<SystemTime>), String> {
    for _ in 0..2 {
        let (mut board, seen) = match read(dir) {
            Ok(Some(found)) => found,
            Ok(None) => (fallback.clone(), None),
            Err(ReadError::Corrupt(_)) => (fallback.clone(), mtime(dir)),
            Err(ReadError::Unreadable(e)) => return Err(format!("board.json is unreadable, the change was not saved: {e}")),
        };
        f(&mut board);
        if mtime(dir) != seen {
            continue;
        }
        let at = save(dir, &board)?;
        write_mirror(dir, &board);
        return Ok((board, at));
    }
    Err("The board changed while saving. Try again.".to_string())
}
