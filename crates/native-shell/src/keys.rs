//! Special keys as the bytes a PTY program expects. Printable keys travel as
//! Text events instead, so this only covers what Text cannot carry.

use eframe::egui::{Key, Modifiers};

pub fn key_bytes(key: Key, mods: Modifiers) -> Option<&'static [u8]> {
    if mods.ctrl && !mods.alt {
        // Single control bytes, spelt out so the table stays readable.
        return Some(match key {
            Key::A => b"\x01",
            Key::B => b"\x02",
            Key::C => b"\x03",
            Key::D => b"\x04",
            Key::E => b"\x05",
            Key::F => b"\x06",
            Key::G => b"\x07",
            Key::H => b"\x08",
            Key::J => b"\x0a",
            Key::K => b"\x0b",
            Key::L => b"\x0c",
            Key::N => b"\x0e",
            Key::O => b"\x0f",
            Key::P => b"\x10",
            Key::Q => b"\x11",
            Key::R => b"\x12",
            Key::S => b"\x13",
            Key::T => b"\x14",
            Key::U => b"\x15",
            Key::V => b"\x16",
            Key::W => b"\x17",
            Key::X => b"\x18",
            Key::Y => b"\x19",
            Key::Z => b"\x1a",
            Key::OpenBracket => b"\x1b",
            Key::Backslash => b"\x1c",
            Key::CloseBracket => b"\x1d",
            // Ctrl+Backspace rubs out the word before the cursor: Ctrl+W,
            // which Claude Code, Codex and readline all read.
            Key::Backspace => b"\x17",
            _ => return None,
        });
    }
    Some(match key {
        Key::Enter => b"\r",
        Key::Tab => b"\t",
        Key::Backspace => b"\x7f",
        Key::Escape => b"\x1b",
        Key::ArrowUp => b"\x1b[A",
        Key::ArrowDown => b"\x1b[B",
        Key::ArrowRight => b"\x1b[C",
        Key::ArrowLeft => b"\x1b[D",
        Key::Home => b"\x1b[H",
        Key::End => b"\x1b[F",
        Key::Insert => b"\x1b[2~",
        Key::Delete => b"\x1b[3~",
        Key::PageUp => b"\x1b[5~",
        Key::PageDown => b"\x1b[6~",
        _ => return None,
    })
}

/// The arrows a program asked for in application cursor mode (DECCKM):
/// full-screen CLIs read `ESC O A`, not `ESC [ A`.
pub fn app_cursor_bytes(key: Key, mods: Modifiers) -> Option<&'static [u8]> {
    if !mods.is_none() {
        return None;
    }
    Some(match key {
        Key::ArrowUp => b"\x1bOA",
        Key::ArrowDown => b"\x1bOB",
        Key::ArrowRight => b"\x1bOC",
        Key::ArrowLeft => b"\x1bOD",
        Key::Home => b"\x1bOH",
        Key::End => b"\x1bOF",
        _ => return None,
    })
}

/// What the mouse wheel does to a program, as a real terminal does it: a
/// program that took the mouse gets wheel reports, a full-screen one gets
/// arrow keys, and otherwise (None) the view scrolls through history.
/// `cell` is the 1-based (column, row) under the pointer.
pub fn wheel_bytes(mode: alacritty_terminal::term::TermMode, up: bool, lines: usize, cell: (usize, usize)) -> Option<Vec<u8>> {
    use alacritty_terminal::term::TermMode;
    let mut out = Vec::new();
    if mode.intersects(TermMode::MOUSE_MODE) {
        let button = if up { 64 } else { 65 };
        for _ in 0..lines {
            if mode.contains(TermMode::SGR_MOUSE) {
                out.extend_from_slice(format!("\x1b[<{button};{};{}M", cell.0, cell.1).as_bytes());
            } else {
                let c = |v: usize| (32 + v.min(223)) as u8;
                out.extend_from_slice(&[0x1b, b'[', b'M', 32 + button, c(cell.0), c(cell.1)]);
            }
        }
        return Some(out);
    }
    if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
        let key: &[u8] = match (up, mode.contains(TermMode::APP_CURSOR)) {
            (true, true) => b"\x1bOA",
            (false, true) => b"\x1bOB",
            (true, false) => b"\x1b[A",
            (false, false) => b"\x1b[B",
        };
        for _ in 0..lines {
            out.extend_from_slice(key);
        }
        return Some(out);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_keys_map_to_pty_bytes() {
        let plain = Modifiers::default();
        assert_eq!(key_bytes(Key::Enter, plain), Some(b"\r".as_slice()));
        assert_eq!(key_bytes(Key::ArrowUp, plain), Some(b"\x1b[A".as_slice()));
        assert_eq!(key_bytes(Key::A, plain), None);
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(key_bytes(Key::C, ctrl), Some(b"\x03".as_slice()));
        assert_eq!(key_bytes(Key::Enter, ctrl), None);
    }

    #[test]
    fn all_special_keys_map_to_pty_bytes() {
        let plain = Modifiers::default();
        let cases = [
            (Key::Tab, "\t"),
            (Key::Backspace, "\x7f"),
            (Key::Escape, "\x1b"),
            (Key::ArrowDown, "\x1b[B"),
            (Key::ArrowRight, "\x1b[C"),
            (Key::ArrowLeft, "\x1b[D"),
            (Key::Home, "\x1b[H"),
            (Key::End, "\x1b[F"),
            (Key::Insert, "\x1b[2~"),
            (Key::Delete, "\x1b[3~"),
            (Key::PageUp, "\x1b[5~"),
            (Key::PageDown, "\x1b[6~"),
        ];
        for (key, want) in cases {
            assert_eq!(key_bytes(key, plain).unwrap(), want.as_bytes(), "{key:?}");
        }
        assert_eq!(key_bytes(Key::Space, plain), None);
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(key_bytes(Key::A, ctrl), Some(b"\x01".as_slice()));
        assert_eq!(key_bytes(Key::Z, ctrl), Some(b"\x1a".as_slice()));
        assert_eq!(key_bytes(Key::OpenBracket, ctrl), Some(b"\x1b".as_slice()));
    }

    #[test]
    fn ctrl_backspace_rubs_out_a_word() {
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(key_bytes(Key::Backspace, ctrl), Some(b"\x17".as_slice()));
    }

    #[test]
    fn the_wheel_goes_where_a_real_terminal_sends_it() {
        use alacritty_terminal::term::TermMode;
        // A plain prompt (Claude Code): scroll the history.
        assert_eq!(wheel_bytes(TermMode::default(), true, 3, (1, 1)), None);
        // A program that took the mouse gets SGR wheel reports.
        let sgr = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        assert_eq!(wheel_bytes(sgr, true, 1, (5, 7)).unwrap(), b"\x1b[<64;5;7M");
        assert_eq!(wheel_bytes(sgr, false, 2, (5, 7)).unwrap(), b"\x1b[<65;5;7M\x1b[<65;5;7M");
        // Full screen without the mouse: arrow keys.
        let alt = TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL | TermMode::APP_CURSOR;
        assert_eq!(wheel_bytes(alt, false, 2, (1, 1)).unwrap(), b"\x1bOB\x1bOB");
    }

    #[test]
    fn arrows_follow_application_cursor_mode() {
        assert_eq!(app_cursor_bytes(Key::ArrowUp, Modifiers::default()), Some(b"\x1bOA".as_slice()));
        assert_eq!(app_cursor_bytes(Key::Enter, Modifiers::default()), None);
    }
}
