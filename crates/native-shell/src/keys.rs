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
}
