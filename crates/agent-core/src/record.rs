//! Session recording: an agent's raw output appended to a JSONL file with a
//! millisecond timestamp per chunk, so a player can replay the terminal with
//! its original timing. Each frame is one line `{"t":<ms since start>,"d":"<b64>"}`;
//! the first line is a header `{"v":1}`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::Result;

/// An in-progress recording. Buffered; flushed every second and on stop/drop,
/// so an app that dies mid-recording still leaves a playable file.
pub(crate) struct Rec {
    w: BufWriter<File>,
    start: Instant,
    flushed: Instant,
}

impl Rec {
    /// Start a recording at `path`; `snapshot` (the screen so far) becomes
    /// the first frame at t=0 so the file is self-contained.
    pub(crate) fn create(path: &str, snapshot: &[u8]) -> Result<Rec> {
        let mut w = BufWriter::new(File::create(Path::new(path))?);
        writeln!(w, "{{\"v\":1}}")?;
        if !snapshot.is_empty() {
            writeln!(w, "{{\"t\":0,\"d\":\"{}\"}}", b64(snapshot))?;
        }
        w.flush()?;
        Ok(Rec { w, start: Instant::now(), flushed: Instant::now() })
    }

    /// Append one frame (called by the reader thread under the output lock).
    /// A failed write must not kill the reader thread; the frame is dropped.
    pub(crate) fn frame(&mut self, bytes: &[u8]) {
        let t = self.start.elapsed().as_millis();
        let _ = writeln!(self.w, "{{\"t\":{t},\"d\":\"{}\"}}", b64(bytes));
        if self.flushed.elapsed().as_millis() >= 1000 {
            let _ = self.w.flush();
            self.flushed = Instant::now();
        }
    }

    pub(crate) fn finish(mut self) {
        let _ = self.w.flush();
    }
}

/// Standard base64 (no line breaks). Small dependency-free encoder.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_writes_frames() {
        assert_eq!(b64(b"hi!"), "aGkh");
        assert_eq!(b64(b"h"), "aA==");
        let path = std::env::temp_dir().join(format!("maestro-rec-{}.jsonl", std::process::id()));
        let p = path.to_string_lossy().to_string();
        let mut r = Rec::create(&p, b"x").unwrap();
        r.frame(b"y");
        r.finish();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "{\"v\":1}");
        assert_eq!(lines[1], "{\"t\":0,\"d\":\"eA==\"}");
        assert!(lines[2].ends_with("\"d\":\"eQ==\"}"));
        std::fs::remove_file(path).ok();
    }
}
