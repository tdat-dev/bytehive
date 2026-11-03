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
            Key::Num2 => b"\x00",
            Key::Num3 | Key::OpenBracket => b"\x1b",
            Key::Num4 | Key::Backslash => b"\x1c",
            Key::Num5 => b"\x1d",
            Key::Num6 => b"\x1e",
            // Ctrl+- is Ctrl+_ (Claude Code's undo), Ctrl+7 the same byte.
            Key::Num7 | Key::Minus => b"\x1f",
            Key::Num8 => b"\x7f",
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

/// Where a focused terminal notes the frame it had the keys in.
pub fn term_focus_id() -> eframe::egui::Id {
    eframe::egui::Id::unique("term-has-keys")
}

/// Whether a terminal had the keys last frame: then every key is the
/// program's and ByteHive's own shortcuts stand aside.
pub fn terminal_has_keys(ctx: &eframe::egui::Context) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<u64>(term_focus_id())).is_some_and(|f| f + 1 >= now)
}

/// A key as the bytes Windows Terminal sends for it, `app_cursor` when the
/// program turned on application cursor mode. None for what Text carries.
pub fn key_to_bytes(key: Key, mods: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    let app = if app_cursor { app_cursor_bytes(key, mods) } else { None };
    fkey_bytes(key, mods).or_else(|| modified_bytes(key, mods)).or_else(|| app.or_else(|| key_bytes(key, mods)).map(<[u8]>::to_vec))
}

/// xterm's modifier parameter: 1 plain, +1 Shift, +2 Alt, +4 Ctrl.
fn xterm_mod(mods: Modifiers) -> u8 {
    1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.ctrl)
}

/// F1 to F12, plain or held with modifiers, as xterm sends them.
pub fn fkey_bytes(key: Key, mods: Modifiers) -> Option<Vec<u8>> {
    let (ss3, n) = match key {
        Key::F1 => (Some('P'), 0),
        Key::F2 => (Some('Q'), 0),
        Key::F3 => (Some('R'), 0),
        Key::F4 => (Some('S'), 0),
        Key::F5 => (None, 15),
        Key::F6 => (None, 17),
        Key::F7 => (None, 18),
        Key::F8 => (None, 19),
        Key::F9 => (None, 20),
        Key::F10 => (None, 21),
        Key::F11 => (None, 23),
        Key::F12 => (None, 24),
        _ => return None,
    };
    let m = xterm_mod(mods);
    Some(
        match (ss3, m) {
            (Some(c), 1) => format!("O{c}"),
            (Some(c), m) => format!("[1;{m}{c}"),
            (None, 1) => format!("[{n}~"),
            (None, m) => format!("[{n};{m}~"),
        }
        .into_bytes(),
    )
}

/// Keys held with Ctrl, Alt or Shift, as xterm (and Windows Terminal) send
/// them: Ctrl+Left is `ESC [1;5D`, which shells and CLIs read as a word
/// back. Alt+letter is ESC then the letter. None when the plain tables say it.
pub fn modified_bytes(key: Key, mods: Modifiers) -> Option<Vec<u8>> {
    let m = xterm_mod(mods);
    let csi = |end: &str| Some(format!("\x1b[1;{m}{end}").into_bytes());
    let tilde = |n: u8| Some(format!("\x1b[{n};{m}~").into_bytes());
    if m == 1 {
        return None;
    }
    match key {
        Key::ArrowUp => csi("A"),
        Key::ArrowDown => csi("B"),
        Key::ArrowRight => csi("C"),
        Key::ArrowLeft => csi("D"),
        Key::Home => csi("H"),
        Key::End => csi("F"),
        Key::Insert => tilde(2),
        Key::Delete => tilde(3),
        Key::PageUp => tilde(5),
        Key::PageDown => tilde(6),
        Key::Tab if mods.shift && !mods.ctrl && !mods.alt => Some(b"\x1b[Z".to_vec()),
        Key::Enter if mods.alt || mods.shift => Some(b"\x1b\r".to_vec()),
        Key::Backspace if mods.alt => Some(b"\x1b\x7f".to_vec()),
        Key::Space if mods.ctrl => Some(vec![0]),
        Key::Slash if mods.ctrl => Some(vec![0x1f]),
        _ if mods.alt && !mods.ctrl => {
            let name = key.name();
            let c = name.chars().next().filter(|c| name.len() == 1 && c.is_ascii_alphanumeric())?;
            let c = if mods.shift { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() };
            Some(vec![0x1b, c as u8])
        }
        _ => None,
    }
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
    fn modified_keys_speak_xterm() {
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        let alt = Modifiers { alt: true, ..Default::default() };
        let shift = Modifiers { shift: true, ..Default::default() };
        assert_eq!(modified_bytes(Key::ArrowLeft, ctrl), Some(b"\x1b[1;5D".to_vec()));
        assert_eq!(modified_bytes(Key::ArrowRight, ctrl), Some(b"\x1b[1;5C".to_vec()));
        assert_eq!(modified_bytes(Key::Delete, ctrl), Some(b"\x1b[3;5~".to_vec()));
        assert_eq!(modified_bytes(Key::Tab, shift), Some(b"\x1b[Z".to_vec()));
        assert_eq!(modified_bytes(Key::B, alt), Some(b"\x1bb".to_vec()));
        assert_eq!(modified_bytes(Key::ArrowLeft, Modifiers::default()), None);
        // Ctrl+letter stays a control byte from the plain table.
        assert_eq!(modified_bytes(Key::C, ctrl), None);
    }

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
    fn function_and_digit_keys_reach_the_program() {
        let plain = Modifiers::default();
        let shift = Modifiers { shift: true, ..Default::default() };
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(key_to_bytes(Key::F1, plain, false), Some(b"OP".to_vec()));
        assert_eq!(key_to_bytes(Key::F1, shift, false), Some(b"[1;2P".to_vec()));
        assert_eq!(key_to_bytes(Key::F5, plain, false), Some(b"[15~".to_vec()));
        assert_eq!(key_to_bytes(Key::F12, ctrl, false), Some(b"[24;5~".to_vec()));
        assert_eq!(key_to_bytes(Key::Minus, ctrl, false), Some(vec![0x1f]), "Ctrl+_ undoes in Claude Code");
        assert_eq!(key_to_bytes(Key::Num2, ctrl, false), Some(vec![0]));
        assert_eq!(key_to_bytes(Key::Num8, ctrl, false), Some(vec![0x7f]));
        assert_eq!(key_to_bytes(Key::ArrowUp, plain, true), Some(b"OA".to_vec()));
        assert_eq!(key_to_bytes(Key::K, Modifiers { ctrl: true, shift: true, ..Default::default() }, false), Some(vec![0x0b]));
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
