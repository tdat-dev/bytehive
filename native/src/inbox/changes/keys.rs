//! The keyboard over a diff (onKey in inboxreview.ts): up and down walk the
//! line numbers, Home and End go to the first and last, Enter comments on the
//! line, Shift+Enter on the run from the last line picked. M merges (view.rs).

use crate::review::text::{Kind, number_hunk};

use super::*;

/// What to bring into view at the next paint, once its place is known.
#[derive(Clone, Copy, Default)]
pub(super) enum Reveal {
    #[default]
    Nothing,
    /// The keyboard's line.
    Line(usize),
    /// The comment box being written.
    Box,
}

/// From the diff's paint: scroll the least that shows `want` whole.
pub(super) fn reveal(scroll: &ScrollHandle, want: Option<Bounds<Pixels>>, window: &mut Window) {
    let Some(b) = want else { return };
    let view = scroll.bounds();
    let mut off = scroll.offset();
    if b.bottom() > view.bottom() {
        off.y -= (b.bottom() - view.bottom()).min(b.top() - view.top());
    } else if b.top() < view.top() {
        off.y += view.top() - b.top();
    } else {
        return;
    }
    scroll.set_offset(off);
    window.refresh();
}

/// Each drawn line's side and number, in order, as changes_diff draws them.
pub(super) fn rows_of(f: &DiffFile, show_all: bool) -> Vec<(Side, u32)> {
    f.hunks
        .iter()
        .flat_map(|h| number_hunk(&h.header, &h.lines))
        .take(if show_all { usize::MAX } else { MAX_LINES })
        .map(|l| if l.kind == Kind::Del { (Side::Old, l.old.unwrap_or(0)) } else { (Side::New, l.new.unwrap_or(0)) })
        .collect()
}

impl Inbox {
    fn diff_rows(&self) -> Vec<(Side, u32)> {
        let Some(c) = &self.changes else { return Vec::new() };
        c.files.as_ref().and_then(|f| f.get(c.file)).map(|f| rows_of(f, c.show_all)).unwrap_or_default()
    }

    /// A key on the drawer; true when it was the diff's.
    pub(super) fn diff_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let k = &event.keystroke;
        if k.modifiers.control || k.modifiers.alt || self.changes.as_ref().is_none_or(|c| c.editing.is_some()) {
            return false;
        }
        let rows = self.diff_rows();
        let Some(last) = rows.len().checked_sub(1) else { return false };
        let at = self.changes.as_ref().and_then(|c| c.line).map(|i| i.min(last));
        let next = match k.key.as_str() {
            "down" => at.map_or(0, |i| (i + 1).min(last)),
            "up" => at.map_or(0, |i| i.saturating_sub(1)),
            "home" => 0,
            "end" => last,
            "enter" => {
                let Some(i) = at else { return false };
                let (side, num) = rows[i];
                self.start_comment(side, num, k.modifiers.shift, window, cx);
                return true;
            }
            _ => return false,
        };
        if let Some(c) = &mut self.changes {
            c.line = Some(next);
            c.reveal.set(Reveal::Line(next));
        }
        cx.notify();
        true
    }

    /// The keyboard's line goes to this one (after a comment is kept or put away).
    pub(super) fn line_to(&mut self, side: Side, num: u32) {
        let at = self.diff_rows().iter().position(|r| *r == (side, num));
        if let Some(c) = &mut self.changes {
            c.line = at.or(c.line);
        }
    }
}
