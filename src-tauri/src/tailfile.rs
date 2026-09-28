//! Reading an append-only JSONL file (the fleet bridge's outbox and spawn
//! requests) a slice at a time, from where the last read stopped.
//!
//! The app used to re-read the whole file every tick through `fs_read_file`,
//! which refuses anything over 2 MB: once an outbox grew past that, messages
//! between agents stopped for good. Here a read starts at a byte offset and
//! hands back whole lines only, so a line the MCP server is still appending is
//! picked up next time instead of being cut in half.

use std::io::{Read, Seek, SeekFrom};

use serde::Serialize;

use crate::core::fs::scoped;
use crate::error::{run_blocking, CommandError};

/// Most bytes one read hands back; the rest comes on the next call.
const SLICE: u64 = 1024 * 1024;

/// Whole lines of a file from an offset on.
#[derive(Serialize, Default, Debug, Clone, PartialEq)]
pub struct LinesChunk {
    /// Complete lines (each ending in `\n`) from `from` on.
    pub text: String,
    /// Where to read from next time.
    pub next: u64,
    /// Where `text` starts: `offset`, or 0 when the file was cut shorter than
    /// `offset` (truncated or replaced), so the caller starts over.
    pub from: u64,
    /// The file's size when read.
    pub size: u64,
}

/// Whole lines of `path` (inside `root`) after byte `offset`. With `offset`
/// None nothing is read: `next` is the end of the file's last whole line, so a
/// first look at a file skips its backlog without reading it.
#[tauri::command]
pub async fn fs_read_lines_from(
    root: String,
    path: String,
    offset: Option<u64>,
    max_bytes: Option<u64>,
) -> Result<LinesChunk, CommandError> {
    run_blocking(move || {
        let file = scoped(&root, &path)?;
        read_lines_from(&file, offset, max_bytes.unwrap_or(SLICE).clamp(1, 8 * SLICE))
            .map_err(|e| CommandError::Failed(e.to_string()))
    })
    .await
}

pub(crate) fn read_lines_from(path: &std::path::Path, offset: Option<u64>, max: u64) -> std::io::Result<LinesChunk> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let Some(offset) = offset else {
        let end = last_line_end(&mut file, size)?;
        return Ok(LinesChunk { text: String::new(), next: end, from: end, size });
    };
    // Cut shorter than where we were: it was truncated or replaced, start over.
    let start = if offset > size { 0 } else { offset };
    let want = (size - start).min(max);
    let mut buf = vec![0u8; want as usize];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut buf)?;
    let mut cut = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    // One line longer than a whole slice would stall the reader forever: hand
    // it over as it is (the caller drops what it cannot parse) and move on.
    if cut == 0 && want == max && want > 0 {
        cut = buf.len();
    }
    buf.truncate(cut);
    Ok(LinesChunk { text: String::from_utf8_lossy(&buf).into_owned(), next: start + cut as u64, from: start, size })
}

/// Just past the last `\n` in the file (0 when there is none).
fn last_line_end(file: &mut std::fs::File, size: u64) -> std::io::Result<u64> {
    let mut end = size;
    let mut buf = vec![0u8; 64 * 1024];
    while end > 0 {
        let at = end.saturating_sub(buf.len() as u64);
        let n = (end - at) as usize;
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut buf[..n])?;
        if let Some(i) = buf[..n].iter().rposition(|&b| b == b'\n') {
            return Ok(at + i as u64 + 1);
        }
        end = at;
    }
    Ok(0)
}

/// Empty `path` (inside `root`) when it is still exactly `size` bytes long,
/// i.e. every line in it has been read. Returns whether it did.
///
/// The file is opened with no sharing while it is checked and cut, so an
/// append that races it either lands before (the size no longer matches and
/// nothing is cut) or waits for the handle to close (the MCP writer retries a
/// busy file), so a message is never cut away unread.
#[tauri::command]
pub async fn fs_truncate_if_size(root: String, path: String, size: u64) -> Result<bool, CommandError> {
    run_blocking(move || {
        let file = scoped(&root, &path)?;
        truncate_if_size(&file, size).map_err(|e| CommandError::Failed(e.to_string()))
    })
    .await
}

pub(crate) fn truncate_if_size(path: &std::path::Path, size: u64) -> std::io::Result<bool> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(0);
    }
    let file = match opts.open(path) {
        Ok(f) => f,
        // Someone has it open (a writer mid-append): leave it for another time.
        Err(_) => return Ok(false),
    };
    if file.metadata()?.len() != size {
        return Ok(false);
    }
    file.set_len(0)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &std::path::Path, s: &str) {
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn reads_whole_lines_and_leaves_a_half_written_one() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        write(&p, "{\"a\":1}\n{\"b\":2}\n{\"c\":");
        let c = read_lines_from(&p, Some(0), SLICE).unwrap();
        assert_eq!(c.text, "{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(c.next, 16);
        // The rest of the line arrives: the next read picks it up whole.
        write(&p, "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n");
        let c2 = read_lines_from(&p, Some(c.next), SLICE).unwrap();
        assert_eq!(c2.text, "{\"c\":3}\n");
        assert_eq!(c2.next, c2.size);
    }

    #[test]
    fn a_first_look_skips_to_the_end_of_the_last_whole_line() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        write(&p, "one\ntwo\nthr");
        let c = read_lines_from(&p, None, SLICE).unwrap();
        assert_eq!((c.text.as_str(), c.next), ("", 8));
        write(&p, "");
        assert_eq!(read_lines_from(&p, None, SLICE).unwrap().next, 0);
    }

    #[test]
    fn a_file_cut_shorter_starts_over() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        write(&p, "new\n");
        let c = read_lines_from(&p, Some(500), SLICE).unwrap();
        assert_eq!((c.text.as_str(), c.from, c.next), ("new\n", 0, 4));
    }

    #[test]
    fn reads_a_big_file_in_slices() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        let line = format!("{}\n", "x".repeat(99)); // 100 bytes
        write(&p, &line.repeat(30_000)); // 3 MB: more than fs_read_file ever took
        let mut at = 0;
        let mut lines = 0;
        loop {
            let c = read_lines_from(&p, Some(at), SLICE).unwrap();
            if c.text.is_empty() {
                break;
            }
            lines += c.text.lines().count();
            at = c.next;
        }
        assert_eq!(lines, 30_000);
    }

    #[test]
    fn a_line_longer_than_a_slice_does_not_stall() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        write(&p, &format!("{}\nok\n", "y".repeat(40)));
        let c = read_lines_from(&p, Some(0), 16).unwrap();
        assert_eq!(c.next, 16);
    }

    #[test]
    fn truncates_only_when_nothing_new_arrived() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        write(&p, "a\nb\n");
        assert!(!truncate_if_size(&p, 2).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nb\n");
        assert!(truncate_if_size(&p, 4).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "");
    }

    #[cfg(windows)]
    #[test]
    fn never_truncates_under_a_writer_that_has_it_open() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("out.jsonl");
        write(&p, "a\n");
        let _writer = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        assert!(!truncate_if_size(&p, 2).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\n");
    }
}
