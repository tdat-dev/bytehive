//! A key log for chasing a typing bug: every key message the hook takes, what
//! it did with it, and every byte it sent to the terminal. Off unless a file
//! named `keylog.on` sits in the data folder (~/.bytehive/native) when the app
//! starts; then it writes `keylog.txt` beside it. It holds what you type, so it
//! is for a few test words, and both files go once the bug is found.

use std::fs::File;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static LOG: OnceLock<Option<(Mutex<File>, Instant)>> = OnceLock::new();

fn open() -> Option<(Mutex<File>, Instant)> {
    let dir = crate::fleet::data_dir()?;
    if !dir.join("keylog.on").exists() {
        return None;
    }
    let file = File::create(dir.join("keylog.txt")).ok()?;
    Some((Mutex::new(file), Instant::now()))
}

/// Write one line, built only when the log is on.
pub fn line(make: impl FnOnce() -> String) {
    if let Some((file, start)) = LOG.get_or_init(open) {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let _ = writeln!(file.lock().unwrap(), "{ms:10.1} {}", make());
    }
}

/// Bytes as text a person can read: ESC as `\e`, other control bytes as hex.
pub fn shown(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\x1b' => out.push_str("\\e"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("<{:02x}>", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn shows_control_bytes_plainly() {
        assert_eq!(super::shown(b"\x1b[0;0;7911;1;0;1_\x08a"), "\\e[0;0;7911;1;0;1_<08>a");
    }
}
