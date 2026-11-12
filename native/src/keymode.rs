//! Whether one terminal's ConPTY wants Windows key records (win32-input-mode,
//! `ESC [ ? 9001 h` on, `ESC [ ? 9001 l` off). Each terminal follows its own:
//! one flag for the whole app let any agent that ended (its ConPTY turns the
//! mode off as it goes) or started flip the keys of the one you were typing
//! in, so after an hour of agents coming and going arrows, Enter, shortcuts
//! and UniKey's backspaces went missing, and came back when another started.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const ON: &[u8] = b"\x1b[?9001h";
const OFF: &[u8] = b"\x1b[?9001l";

/// Reads a terminal's output for the switch, a sequence cut between two reads
/// included.
pub struct ModeWatch {
    on: Arc<AtomicBool>,
    /// The last few bytes of the previous read, in case a switch began there.
    tail: Vec<u8>,
}

impl ModeWatch {
    pub fn new() -> Self {
        Self { on: Arc::new(AtomicBool::new(false)), tail: Vec::new() }
    }

    /// The flag the key hook reads while this terminal has the keyboard.
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.on.clone()
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut seen = std::mem::take(&mut self.tail);
        seen.extend_from_slice(bytes);
        let last = |needle: &[u8]| seen.windows(needle.len()).rposition(|w| w == needle);
        match (last(ON), last(OFF)) {
            (Some(on), Some(off)) => self.on.store(on > off, Ordering::Relaxed),
            (Some(_), None) => self.on.store(true, Ordering::Relaxed),
            (None, Some(_)) => self.on.store(false, Ordering::Relaxed),
            (None, None) => {}
        }
        let keep = seen.len().min(ON.len() - 1);
        self.tail = seen[seen.len() - keep..].to_vec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_terminal_turning_the_mode_off_leaves_the_others_on() {
        let (mut typing, mut ending) = (ModeWatch::new(), ModeWatch::new());
        typing.feed(b"\x1b[?9001h\x1b[?1004h> ");
        ending.feed(b"\x1b[?9001h");
        ending.feed(b"bye\x1b[?9001l\x1b[?1004l");
        assert!(typing.flag().load(Ordering::Relaxed), "the terminal you type in keeps its keys");
        assert!(!ending.flag().load(Ordering::Relaxed));
    }

    #[test]
    fn a_switch_cut_between_two_reads_is_still_seen() {
        let mut w = ModeWatch::new();
        w.feed(b"hello\x1b[?90");
        w.feed(b"01h prompt");
        assert!(w.flag().load(Ordering::Relaxed));
        w.feed(b"...\x1b[?9001");
        w.feed(b"l");
        assert!(!w.flag().load(Ordering::Relaxed));
    }

    #[test]
    fn the_later_of_two_switches_wins() {
        let mut w = ModeWatch::new();
        w.feed(b"\x1b[?9001l\x1b[?9001h");
        assert!(w.flag().load(Ordering::Relaxed));
        w.feed(b"\x1b[?9001h\x1b[?9001l");
        assert!(!w.flag().load(Ordering::Relaxed));
    }
}
