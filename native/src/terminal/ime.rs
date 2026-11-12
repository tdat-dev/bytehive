//! An IME's text (Telex through the Windows IME) reaches the CLI as typed characters.

use super::*;

impl EntityInputHandler for Terminal {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.preedit.is_empty()).then(|| 0..self.preedit.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        cx.notify();
    }

    /// An IME committed: its text goes to the program as typed characters.
    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        self.note_input();
        self.write(keys::text_records(text, self.win32.load(std::sync::atomic::Ordering::Relaxed)));
        cx.notify();
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit = text.to_string();
        cx.notify();
    }

    /// Where the IME's candidate window should sit: at the cursor.
    fn bounds_for_range(&mut self, _: Range<usize>, element: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let term = self.term.lock().unwrap();
        let cursor = term.grid().cursor.point;
        let origin = element.origin + point(self.cell_w * cursor.column.0 as f32, self.cell_h * cursor.line.0 as f32);
        Some(Bounds::new(origin, size(self.cell_w, self.cell_h)))
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}
