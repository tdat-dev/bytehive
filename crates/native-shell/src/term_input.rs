//! A focused terminal's keys, all of them the program's, sent as Windows
//! Terminal sends them: whole Windows key events once ConPTY asked for them
//! (term_win32.rs), xterm bytes before that. The pane keeps only what a terminal window keeps for
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
/// events carry none), `selected` whether a selection is on screen, `win32`
/// whether ConPTY turned win32-input-mode on.
pub fn actions(events: &[egui::Event], mods: Modifiers, mode: TermMode, selected: bool, win32: bool) -> Vec<Action> {
    if win32 {
        return win32_actions(events, mods, mode, selected);
    }
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

/// The Ctrl+Shift keys a terminal window keeps for itself.
fn local(key: Key, m: Modifiers) -> Option<Action> {
    if !(m.ctrl && m.shift && !m.alt) {
        return None;
    }
    match key {
        Key::F => Some(Action::Find),
        Key::C => Some(Action::Copy),
        Key::V => Some(Action::Paste),
        _ => None,
    }
}

/// The same events as whole Windows key events: every press and release,
/// with the character Windows makes of it.
fn win32_actions(events: &[egui::Event], mods: Modifiers, mode: TermMode, selected: bool) -> Vec<Action> {
    use crate::term_win32 as w;
    let bracketed = mode.contains(TermMode::BRACKETED_PASTE);
    let mut out = Vec::new();
    for ev in events {
        let act = match ev {
            // Alt+key goes with its character in the key's own event.
            egui::Event::Text(_) if mods.alt && !mods.ctrl => None,
            egui::Event::Text(t) => Some(Action::Send(t.as_bytes().to_vec())),
            egui::Event::Key { key, pressed: true, modifiers, .. } if local(*key, *modifiers).is_some() => local(*key, *modifiers),
            egui::Event::Key { key, pressed: false, modifiers, .. } if local(*key, *modifiers).is_some() => None,
            egui::Event::Key { key, physical_key, pressed, modifiers, .. } => w::key_record(*key, *physical_key, *modifiers, *pressed).map(Action::Send),
            egui::Event::Copy if selected => Some(Action::Copy),
            egui::Event::Copy => Some(Action::Send(w::ctrl_tap(0x43, 0x03))),
            egui::Event::Cut if mods.shift => Some(Action::Send(w::shift_delete())),
            egui::Event::Cut => Some(Action::Send(w::ctrl_tap(0x58, 0x18))),
            egui::Event::Paste(_) | egui::Event::PasteImage(_) => crate::paste::clipboard_bytes(ev, bracketed).map(Action::Send),
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
        assert_eq!(actions(&[egui::Event::Copy], none, TermMode::empty(), false, false), vec![Action::Send(vec![3])]);
        assert_eq!(actions(&[egui::Event::Copy], none, TermMode::empty(), true, false), vec![Action::Copy]);
    }

    #[test]
    fn shift_delete_is_a_key_not_a_cut() {
        let shift = Modifiers { shift: true, ..Default::default() };
        assert_eq!(actions(&[egui::Event::Cut], shift, TermMode::empty(), false, false), vec![Action::Send(b"\x1b[3;2~".to_vec())]);
        assert_eq!(actions(&[egui::Event::Cut], Modifiers::NONE, TermMode::empty(), false, false), vec![Action::Send(vec![0x18])]);
    }

    #[test]
    fn only_the_terminal_keys_stay_in_the_pane() {
        let evs = [key(Key::F, CTRL_SHIFT), key(Key::C, CTRL_SHIFT), key(Key::V, CTRL_SHIFT)];
        assert_eq!(actions(&evs, CTRL_SHIFT, TermMode::empty(), false, false), vec![Action::Find, Action::Copy, Action::Paste]);
        // Ctrl+Shift+K, Ctrl+Shift+N: the program's, as Ctrl+K and Ctrl+N.
        let evs = [key(Key::K, CTRL_SHIFT), key(Key::N, CTRL_SHIFT)];
        assert_eq!(actions(&evs, CTRL_SHIFT, TermMode::empty(), false, false), vec![Action::Send(vec![0x0b]), Action::Send(vec![0x0e])]);
    }

    #[test]
    fn alt_keys_reach_the_program_once() {
        // Alt+b: ESC b from the key, the typed "b" dropped.
        let evs = [key(Key::B, ALT), egui::Event::Text("b".into())];
        assert_eq!(actions(&evs, ALT, TermMode::empty(), false, false), vec![Action::Send(b"\x1bb".to_vec())]);
        // Alt+. has no letter: ESC and the typed dot.
        let evs = [key(Key::Period, ALT), egui::Event::Text(".".into())];
        assert_eq!(actions(&evs, ALT, TermMode::empty(), false, false), vec![Action::Send(b"\x1b.".to_vec())]);
    }

    #[test]
    fn under_conpty_keys_go_whole_and_text_as_typed() {
        let evs = [key(Key::K, CTRL_SHIFT), egui::Event::Text("é".into()), key(Key::F, CTRL_SHIFT)];
        let out = actions(&evs, CTRL_SHIFT, TermMode::empty(), false, true);
        assert!(matches!(&out[0], Action::Send(b) if b.starts_with(b"\x1b[75;")), "Ctrl+Shift+K whole: {out:?}");
        assert_eq!(out[1], Action::Send("é".as_bytes().to_vec()));
        assert_eq!(out[2], Action::Find);
        let out = actions(&[egui::Event::Copy], Modifiers::NONE, TermMode::empty(), false, true);
        assert!(matches!(&out[0], Action::Send(b) if b.starts_with(b"\x1b[67;")), "Ctrl+C as the key: {out:?}");
    }

    #[test]
    fn arrows_follow_the_cursor_mode() {
        let evs = [key(Key::ArrowUp, Modifiers::NONE)];
        assert_eq!(actions(&evs, Modifiers::NONE, TermMode::APP_CURSOR, false, false), vec![Action::Send(b"\x1bOA".to_vec())]);
        assert_eq!(actions(&evs, Modifiers::NONE, TermMode::empty(), false, false), vec![Action::Send(b"\x1b[A".to_vec())]);
    }
}
