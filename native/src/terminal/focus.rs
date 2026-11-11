//! Focus reports, as a console window gives them: once the program turns on
//! DECSET 1004 (ConPTY asks for it at start), the terminal says ESC [ I when
//! it takes the keyboard and ESC [ O when it loses it, to another view or to
//! another window. ConPTY hands the program a FOCUS_EVENT record for each.

use super::*;

impl Terminal {
    /// Watch this terminal's focus and the window's, once.
    pub(super) fn watch_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_watch.is_empty() {
            return;
        }
        let focus = self.focus.clone();
        self.focus_watch = vec![
            cx.on_focus_in(&focus, window, |this, window, _| this.report_focus(window)),
            cx.on_focus_out(&focus, window, |this, _, window, _| this.report_focus(window)),
            cx.observe_window_activation(window, |this, window, _| this.report_focus(window)),
        ];
    }

    fn report_focus(&mut self, window: &Window) {
        let focused = self.focus.is_focused(window) && window.is_window_active();
        if let Some(bytes) = focus_report(self.had_focus, focused, self.term.lock().unwrap().mode().contains(TermMode::FOCUS_IN_OUT)) {
            self.write(bytes);
        }
        self.had_focus = focused;
    }
}

/// What to tell the program when focus goes from `was` to `now`.
pub(super) fn focus_report(was: bool, now: bool, asked: bool) -> Option<&'static str> {
    match (asked, was, now) {
        (true, false, true) => Some("\x1b[I"),
        (true, true, false) => Some("\x1b[O"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::focus_report;

    #[test]
    fn reports_only_a_change_and_only_when_asked() {
        assert_eq!(focus_report(false, true, true), Some("\x1b[I"));
        assert_eq!(focus_report(true, false, true), Some("\x1b[O"));
        assert_eq!(focus_report(true, true, true), None);
        assert_eq!(focus_report(false, true, false), None, "a program that never asked gets nothing");
    }
}
