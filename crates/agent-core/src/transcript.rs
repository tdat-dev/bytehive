//! Reading agent transcripts in slices: the shared chunk shape plus the two
//! helpers every CLI reader leans on (where a read starts; what an id is).

use serde::Serialize;

/// One resumable conversation, for session lists.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct SessionItem {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub modified_ms: u64,
}

/// New lines of one transcript, for the chat view.
#[derive(Serialize, Default, Clone)]
pub struct TranscriptChunk {
    /// The file read, or "" when none was found yet.
    pub path: String,
    /// Complete JSONL lines from `offset` on (a half-written last line waits).
    pub text: String,
    /// Where to read from next time.
    pub next: u64,
    /// Where this text starts in the file: more than 0 on a first read that
    /// was asked for only the end of a long conversation.
    pub from: u64,
}

/// Where reading a transcript starts: at `offset`; or, on a first read
/// (`offset` 0) asked for only the end, at the first whole line within the
/// last `tail` bytes, so a long conversation opens without reading all of it.
pub fn read_start(file: &mut std::fs::File, size: u64, offset: u64, tail: Option<u64>) -> u64 {
    use std::io::{Read, Seek, SeekFrom};
    // the file was replaced or cut: start over
    if offset > size {
        return 0;
    }
    if offset != 0 {
        return offset;
    }
    let Some(t) = tail.filter(|t| *t > 0 && size > *t) else { return 0 };
    let mut at = size - t;
    let mut buf = vec![0u8; 64 * 1024];
    while at < size {
        if file.seek(SeekFrom::Start(at)).is_err() {
            return 0;
        }
        let n = match file.read(&mut buf) {
            Ok(0) | Err(_) => return size,
            Ok(n) => n,
        };
        if let Some(i) = buf[..n].iter().position(|&b| b == b'\n') {
            return at + i as u64 + 1;
        }
        at += n as u64;
    }
    size
}

/// A session id is a UUID; anything else never reaches the filesystem.
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}
