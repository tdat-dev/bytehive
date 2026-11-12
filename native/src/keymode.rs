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

/// What a real ConPTY says about the mode over a session's life, measured
/// 2026-11-12: `ESC[?9001h` as it starts, `ESC[?9001l ESC[?1004l` when the
/// pseudoconsole closes (an agent stopped, removed or restarted), nothing as
/// the program alone exits. Run by hand:
/// cargo test conpty_mode_over_a_life -- --ignored --nocapture
#[cfg(test)]
mod conpty {
    use std::sync::{Arc, Mutex};

    #[test]
    #[ignore]
    fn conpty_mode_over_a_life() {
        for (program, args) in [("cmd.exe", vec!["/c", "echo hi"]), ("powershell.exe", vec!["-NoLogo", "-NoProfile", "-Command", "Start-Sleep -Milliseconds 300; exit"])] {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let (into, done) = (seen.clone(), Arc::new(Mutex::new(false)));
            let ended = done.clone();
            let mut spec = agent_core::CommandSpec::new(program);
            spec.args = args.iter().map(|a| a.to_string()).collect();
            let size = portable_pty::PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };
            let mut agent = agent_core::Agent::spawn(&spec, size, move |b: &[u8]| into.lock().unwrap().extend_from_slice(b), move |_| *ended.lock().unwrap() = true).unwrap();
            let mut answered = 0;
            for _ in 0..50 {
                if *done.lock().unwrap() {
                    break;
                }
                // ConPTY asks where the cursor is and waits: answer as a terminal does.
                let asks = String::from_utf8_lossy(&seen.lock().unwrap()).matches("\x1b[6n").count();
                while answered < asks {
                    let _ = agent.write_input(b"\x1b[1;1R");
                    answered += 1;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
            // The app stops an agent by dropping it: the pseudoconsole closes.
            drop(agent);
            std::thread::sleep(std::time::Duration::from_millis(800));
            let bytes = seen.lock().unwrap().clone();
            let text = String::from_utf8_lossy(&bytes).replace('\x1b', "ESC");
            let at = |n: &str| text.match_indices(n).map(|(i, _)| i).collect::<Vec<_>>();
            println!("{program}: 9001h at {:?}, 9001l at {:?}, length {}\n  tail: {:?}", at("ESC[?9001h"), at("ESC[?9001l"), text.len(), &text[text.len().saturating_sub(160)..]);
        }
    }
}
