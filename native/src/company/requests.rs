//! requests.jsonl: what the MCP server sends the app for people (drainJsonl in
//! fleetbridge.ts). Read from the byte offset the last read stopped at, never
//! the whole file, and only whole lines: one the server is still writing is
//! left for the next tick. Once all of a big file is read, it is emptied.

use super::types::CompanyRequest;
use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Most bytes one read takes; the rest comes in the next slice.
const SLICE: u64 = 1024 * 1024;
/// Slices one drain reads at most.
const MAX_SLICES: usize = 8;
/// A fully read file bigger than this is emptied, so it can't grow forever.
pub const COMPACT_BYTES: u64 = 1024 * 1024;

/// Where the next read starts. None until the file is first looked at.
#[derive(Debug, Default)]
pub struct Drain {
    at: Option<u64>,
}

/// Whole lines from `at` on, and where the next read starts.
fn read_from(path: &Path, at: u64) -> std::io::Result<(Vec<u8>, u64, u64)> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    // Cut shorter than where we were: it was emptied or replaced, start over.
    let start = if at > size { 0 } else { at };
    let want = (size - start).min(SLICE);
    let mut buf = vec![0u8; want as usize];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut buf)?;
    let mut cut = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    // One line longer than a slice would stall the reader: hand it over as it is.
    if cut == 0 && want == SLICE {
        cut = buf.len();
    }
    buf.truncate(cut);
    Ok((buf, start + cut as u64, size))
}

/// Just past the last whole line: where a first look starts, skipping the backlog.
fn last_line_end(path: &Path) -> std::io::Result<u64> {
    let bytes = std::fs::read(path)?;
    Ok(bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i as u64 + 1))
}

/// Empty the file, only if nothing was appended since it was read. Opened with
/// no sharing, so an append racing it lands before (the size no longer matches)
/// or waits for this to close.
fn empty_if_size(path: &Path, size: u64) -> bool {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(0);
    }
    let Ok(file) = opts.open(path) else { return false };
    file.metadata().is_ok_and(|m| m.len() == size) && file.set_len(0).is_ok()
}

impl Drain {
    /// The new whole lines, blank ones left out. A file not there yet starts at
    /// 0, so the line that creates it is read; one seen for the first time is
    /// skipped to its last whole line.
    pub fn lines(&mut self, path: &Path) -> Vec<String> {
        let Some(mut at) = self.at else {
            self.at = Some(if path.exists() { last_line_end(path).unwrap_or(0) } else { 0 });
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut size = 0;
        for _ in 0..MAX_SLICES {
            let Ok((buf, next, len)) = read_from(path, at) else { return out };
            // Moved on before handing over, so a line is never handed over twice.
            self.at = Some(next);
            size = len;
            let text = String::from_utf8_lossy(&buf);
            out.extend(text.split('\n').map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()).map(str::to_string));
            let moved = next != at;
            at = next;
            if !moved || next >= len {
                break;
            }
        }
        if at == size && size > COMPACT_BYTES && empty_if_size(path, size) {
            self.at = Some(0);
        }
        out
    }

    /// Back to not having looked: another app ran the company meanwhile.
    pub fn reset(&mut self) {
        self.at = None;
    }
}

/// A line that is not a request this app knows, and who sent it when the line says.
#[derive(Debug, PartialEq)]
pub struct Unread {
    pub from: Option<String>,
    pub why: String,
}

pub fn parse(line: &str) -> Result<CompanyRequest, Unread> {
    let value: Value = serde_json::from_str(line).map_err(|e| Unread { from: None, why: format!("it is not JSON ({e})") })?;
    let from = value.get("from").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    serde_json::from_value(value).map_err(|e| Unread { from, why: e.to_string() })
}

/// What the sender of a line that could not be read is told, so nothing is
/// dropped without a word.
pub fn not_understood(bad: &Unread) -> String {
    format!("Your request was not understood, so nothing was done: {}. Send it again through the ByteHive tools.", bad.why)
}

#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
