//! A focused terminal's keys, all of them the program's, sent as Windows
//! Terminal sends them. The pane keeps only what a terminal window keeps for
//! itself: Ctrl+C over a selection copies it, Ctrl+Shift+C and Ctrl+Shift+V
//! copy and paste, Ctrl+Shift+F finds.

use alacritty_terminal::term::TermMode;
use eframe::egui::{self, Key, Modifiers};

/// What one input event asks of the pane.
#[derive(Debug, PartialEq)]
pub enum Action {
    /// Bytes for the program.
    Send(Vec<u8>),
    /// Copy the selection.
    Copy,
    /// Ask the system for the clipboard (it comes back as a Paste event).
    Paste,
    /// Open or close Find.
    Find,
}

/// This frame's events as actions. `mods` are the modifiers held now (Text
/// events carry none), `selected` whether a selection is on screen.
pub fn actions(events: &[egui::Event], mods: Modifiers, mode: TermMode, selected: bool) -> Vec<Action> {
    let bracketed = mode.contains(TermMode::BRACKETED_PASTE);
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let mut out = Vec::new();
    for ev in events {
        let act = match ev {
            egui::Event::Text(t) => text_bytes(t, mods).map(Action::Send),
            egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.ctrl && modifiers.shift && !modifiers.alt => match key {
                Key::F => Some(Action::Find),
                Key::C => Some(Action::Copy),
                Key::V => Some(Action::Paste),
                _ => crate::keys::key_to_bytes(*key, *modifiers, app_cursor).map(Action::Send),
            },
            egui::Event::Key { key, pressed: true, modifiers, .. } => crate::keys::key_to_bytes(*key, *modifiers, app_cursor).map(Action::Send),
            // egui turns Ctrl+C into Copy: a selection copies, else it interrupts.
            egui::Event::Copy if selected => Some(Action::Copy),
            // Shift+Delete also arrives as Cut: it is a key, not a cut.
            egui::Event::Cut if mods.shift => Some(Action::Send(b"\x1b[3;2~".to_vec())),
            egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_) | egui::Event::PasteImage(_) => crate::paste::clipboard_bytes(ev, bracketed).map(Action::Send),
            _ => None,
        };
        out.extend(act);
    }
    out
}

/// Typed text. Alt+letter goes as ESC+letter from its Key event, so the
/// letter Windows also types for it is dropped; any other Alt character
/// (Alt+., Alt+/) goes as ESC and the character. AltGr is Ctrl+Alt and
/// types its characters plainly.
fn text_bytes(t: &str, mods: Modifiers) -> Option<Vec<u8>> {
    if mods.alt && !mods.ctrl {
        let mut c = t.chars();
        if matches!((c.next(), c.next()), (Some(ch), None) if ch.is_ascii_alphanumeric()) {
            return None;
        }
        let mut b = vec![0x1b];
        b.extend_from_slice(t.as_bytes());
        return Some(b);
    }
    Some(t.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    const CTRL_SHIFT: Modifiers = Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };
    const ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };

    #[test]
    fn ctrl_c_interrupts_unless_something_is_selected() {
        let none = Modifiers::NONE;
        assert_eq!(actions(&[egui::Event::Copy], none, TermMode::empty(), false), vec![Action::Send(vec![3])]);
        assert_eq!(actions(&[egui::Event::Copy], none, TermMode::empty(), true), vec![Action::Copy]);
    }

    #[test]
    fn shift_delete_is_a_key_not_a_cut() {
        let shift = Modifiers { shift: true, ..Default::default() };
        assert_eq!(actions(&[egui::Event::Cut], shift, TermMode::empty(), false), vec![Action::Send(b"\x1b[3;2~".to_vec())]);
        assert_eq!(actions(&[egui::Event::Cut], Modifiers::NONE, TermMode::empty(), false), vec![Action::Send(vec![0x18])]);
    }

    #[test]
    fn only_the_terminal_keys_stay_in_the_pane() {
        let evs = [key(Key::F, CTRL_SHIFT), key(Key::C, CTRL_SHIFT), key(Key::V, CTRL_SHIFT)];
        assert_eq!(actions(&evs, CTRL_SHIFT, TermMode::empty(), false), vec![Action::Find, Action::Copy, Action::Paste]);
        // Ctrl+Shift+K, Ctrl+Shift+N: the program's, as Ctrl+K and Ctrl+N.
        let evs = [key(Key::K, CTRL_SHIFT), key(Key::N, CTRL_SHIFT)];
        assert_eq!(actions(&evs, CTRL_SHIFT, TermMode::empty(), false), vec![Action::Send(vec![0x0b]), Action::Send(vec![0x0e])]);
    }

    #[test]
    fn alt_keys_reach_the_program_once() {
        // Alt+b: ESC b from the key, the typed "b" dropped.
        let evs = [key(Key::B, ALT), egui::Event::Text("b".into())];
        assert_eq!(actions(&evs, ALT, TermMode::empty(), false), vec![Action::Send(b"\x1bb".to_vec())]);
        // Alt+. has no letter: ESC and the typed dot.
        let evs = [key(Key::Period, ALT), egui::Event::Text(".".into())];
        assert_eq!(actions(&evs, ALT, TermMode::empty(), false), vec![Action::Send(b"\x1b.".to_vec())]);
    }

    #[test]
    fn arrows_follow_the_cursor_mode() {
        let evs = [key(Key::ArrowUp, Modifiers::NONE)];
        assert_eq!(actions(&evs, Modifiers::NONE, TermMode::APP_CURSOR, false), vec![Action::Send(b"\x1bOA".to_vec())]);
        assert_eq!(actions(&evs, Modifiers::NONE, TermMode::empty(), false), vec![Action::Send(b"\x1b[A".to_vec())]);
    }
}
