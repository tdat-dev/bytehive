//! The app's own keys, taken before a terminal sees them only where keys.rs
//! lets them through (Ctrl+K, Ctrl+Shift+T); the rest work when no terminal
//! has the keyboard (Alt+S and Alt+P for the Grid among them). Esc puts away
//! the topmost thing open.

use super::*;

impl Inbox {
    pub(super) fn app_keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        let m = k.modifiers;
        // The owner is at work: the boot no longer moves them to the Company.
        self.company.touched = true;
        if m.control && !m.alt && !m.shift && k.key == "k" {
            if self.palette.is_some() { self.close_palette(window, cx) } else { self.open_palette(window, cx) }
        } else if m.control && m.shift && !m.alt && k.key == "t" {
            self.palette = None;
            self.open_sheet(window, cx);
        } else if m.control && m.shift && !m.alt && k.key == "e" {
            self.toggle_files(window, cx);
        } else if m.control && m.shift && !m.alt && k.key == "k" {
            if self.board.is_some() { self.close_board(window, cx) } else { self.open_board(window, cx) }
        } else if m.control && m.shift && !m.alt && k.key == "o" {
            self.toggle_company(window, cx);
        } else if k.key == "escape" && self.company.open && self.company_escape(cx) {
            // The Company's confirm or menu went away.
        } else if m.control && !m.alt && !m.shift && k.key == "," {
            self.open_settings(window, cx);
        } else if m.alt && !m.control && !m.shift && k.key == "o" {
            self.open_sessions(window, cx);
        } else if m.alt && !m.control && !m.shift && k.key == "s" && !self.dialog_up() {
            self.set_grid(!self.grid.on, window, cx);
        } else if m.alt && !m.control && !m.shift && k.key == "p" && !self.dialog_up() {
            self.pin_current(window, cx);
        } else if k.key == "escape" && self.dialog_open() {
            self.dialog_escape(window, cx);
        } else if k.key == "escape" && self.settings.is_some() {
            if self.popup.take().is_none() {
                self.close_settings(window, cx);
            }
            cx.notify();
        } else if k.key == "escape" && self.sessions.is_some() {
            self.close_sessions(window, cx);
        } else if k.key == "escape" && self.board.is_some() {
            self.board_escape(window, cx);
        } else if k.key == "escape" && self.changes.as_ref().is_some_and(|c| c.discarding) {
            if let Some(c) = &mut self.changes {
                c.discarding = false;
            }
            cx.notify();
        } else if k.key == "escape" && self.changes.as_ref().is_some_and(|c| c.editing.is_some()) {
            // Esc puts the comment away, not the whole drawer.
            self.cancel_comment(window, cx);
        } else if k.key == "escape" && self.changes.is_some() {
            self.close_changes(window, cx);
        } else if k.key == "escape" && self.memory.is_some() {
            self.memory_escape(window, cx);
        } else if k.key == "escape" && self.history.open.is_some() {
            self.close_history(window, cx);
        } else if m.alt && !m.control && !m.shift && k.key == "h" {
            if let Some(index) = self.fleet.focused {
                self.open_history(index, window, cx);
            }
        } else if m.alt && !m.control && !m.shift && k.key == "r" {
            if let Some(index) = self.fleet.focused {
                self.open_changes(index, window, cx);
            }
        }
    }
}
