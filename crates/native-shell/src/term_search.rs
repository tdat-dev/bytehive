//! Find in a terminal's output (Ctrl+Shift+F, as in the web app): a small bar
//! over the pane; typing finds the newest match, Enter goes to an older one,
//! Shift+Enter to a newer one, Esc closes. The match is selected (so Ctrl+C
//! copies it) and scrolled into view. Plain text, case-insensitive unless
//! the query has a capital, the way alacritty's own search reads it.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::search::{Match, RegexSearch};
use alacritty_terminal::term::Term;
use eframe::egui::{self, Key, Modifiers, RichText};

use crate::theme::{INK3, SHEET};

#[derive(Default)]
pub struct FindBar {
    pub open: bool,
    pub text: String,
    last: Option<Match>,
    focus: bool,
    pub missing: bool,
}

/// The next match of `query`: older (up) from `from`, or newer (down); from
/// the bottom of the screen when there is no last match.
pub fn find<T: EventListener>(term: &Term<T>, query: &str, from: Option<&Match>, older: bool) -> Option<Match> {
    if query.is_empty() {
        return None;
    }
    let mut re = RegexSearch::new(&regex::escape(query)).ok()?;
    let bottom = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns().saturating_sub(1)));
    let (origin, dir, side) = match (from, older) {
        (Some(m), true) => (m.start().sub(term, Boundary::None, 1), Direction::Left, Side::Right),
        (Some(m), false) => (m.end().add(term, Boundary::None, 1), Direction::Right, Side::Left),
        (None, _) => (bottom, Direction::Left, Side::Right),
    };
    term.search_next(&mut re, origin, dir, side, None)
}

impl FindBar {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.focus = self.open;
        self.last = None;
    }

    fn go<T: EventListener>(&mut self, term: &mut Term<T>, older: bool, fresh: bool) {
        let m = find(term, &self.text, if fresh { None } else { self.last.as_ref() }, older);
        self.missing = m.is_none() && !self.text.is_empty();
        if let Some(m) = &m {
            let mut s = Selection::new(SelectionType::Simple, *m.start(), Side::Left);
            s.update(*m.end(), Side::Right);
            term.selection = Some(s);
            term.scroll_to_point(*m.start());
        }
        if m.is_some() {
            self.last = m;
        }
    }

    /// The bar over the pane's top-right corner. False when it closed.
    pub fn show<T: EventListener>(&mut self, ui: &mut egui::Ui, pane_id: &str, rect: egui::Rect, term: &mut Term<T>) {
        if !self.open {
            return;
        }
        let mut close = false;
        egui::Area::new(egui::Id::unique(("find", pane_id))).order(egui::Order::Foreground).fixed_pos(egui::pos2(rect.right() - 300.0, rect.top() + 6.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(SHEET).corner_radius(8.0).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.text).hint_text("Find in output").desired_width(170.0));
                    if self.focus {
                        r.request_focus();
                        self.focus = false;
                    }
                    if r.changed() {
                        self.go(term, true, true);
                    }
                    if r.has_focus() || r.lost_focus() {
                        let (enter, shift, esc) = ui.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::SHIFT, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape)));
                        if enter || shift {
                            self.go(term, !shift, false);
                            r.request_focus();
                        }
                        close |= esc;
                    }
                    if self.missing {
                        ui.label(RichText::new("No match").size(11.5).color(INK3));
                    }
                    if ui.small_button("\u{2191}").on_hover_text("Older (Enter)").clicked() {
                        self.go(term, true, false);
                    }
                    if ui.small_button("\u{2193}").on_hover_text("Newer (Shift+Enter)").clicked() {
                        self.go(term, false, false);
                    }
                    close |= ui.small_button("\u{2715}").on_hover_text("Close (Esc)").clicked();
                });
            });
        });
        if close {
            self.open = false;
            self.last = None;
            term.selection = None;
            term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term_pane::GridSize;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    #[test]
    fn finds_older_and_newer_matches() {
        let mut term = Term::new(Config { scrolling_history: 100, ..Config::default() }, &GridSize { rows: 3, cols: 30 }, VoidListener);
        let mut p = Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new();
        p.advance(&mut term, b"error one\r\nok\r\nok\r\nError two\r\nok\r\nok");
        let newest = find(&term, "error", None, true).expect("a match");
        assert_eq!(newest.start().column.0, 0);
        let older = find(&term, "error", Some(&newest), true).expect("an older one");
        assert!(older.start().line < newest.start().line, "{older:?} above {newest:?}");
        let back = find(&term, "error", Some(&older), false).expect("newer again");
        assert_eq!(back.start().line, newest.start().line);
        assert!(find(&term, "Error", None, true).is_some_and(|m| m.start().line == newest.start().line), "a capital matches case");
        assert!(find(&term, "nothing", None, true).is_none());
        let mut bar = FindBar { text: "one".into(), ..Default::default() };
        bar.go(&mut term, true, true);
        assert_eq!(term.selection_to_string().as_deref(), Some("one"));
    }
}
