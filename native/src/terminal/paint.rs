//! One frame of the grid: backgrounds, glyphs, box drawing, the cursor, an IME's preedit.

use super::*;

impl Terminal {
    /// One frame of the grid as flat lists, so the lock is not held while painting.
    pub(super) fn snapshot(&self) -> (Vec<Fill>, Vec<Span>, Vec<(usize, usize, char, Rgb)>, Option<(usize, usize, CursorShape)>) {
        let term = self.term.lock().unwrap();
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let selection = content.selection;
        let selected: Hsla = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.22 }.into();

        let mut fills: Vec<Fill> = Vec::new();
        let mut spans: Vec<Span> = Vec::new();
        let mut drawn: Vec<(usize, usize, char, Rgb)> = Vec::new();
        let push_fill = |fills: &mut Vec<Fill>, row: usize, col: usize, cols: usize, color: Hsla| match fills.last_mut() {
            Some(last) if last.row == row && last.col + last.cols == col && last.color == color => last.cols += cols,
            _ => fills.push(Fill { row, col, cols, color }),
        };

        for indexed in content.display_iter {
            let cell = indexed.cell;
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let row = (indexed.point.line.0 + offset) as usize;
            let col = indexed.point.column.0;
            let wide = cell.flags.contains(Flags::WIDE_CHAR);
            let width = if wide { 2 } else { 1 };
            let (mut fg, mut bg) = (resolve(cell.fg, content.colors), resolve(cell.bg, content.colors));
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.flags.contains(Flags::DIM) {
                fg = dimmed(fg);
            }
            if bg != BACKGROUND {
                push_fill(&mut fills, row, col, width, hsla(bg));
            }
            if selection.is_some_and(|s| s.contains(indexed.point)) {
                push_fill(&mut fills, row, col, width, selected);
            }
            if cell.c == ' ' || cell.flags.contains(Flags::HIDDEN) {
                continue;
            }
            let weight = if cell.flags.contains(Flags::BOLD) { FontWeight::MEDIUM } else { FontWeight::NORMAL };
            let italic = cell.flags.contains(Flags::ITALIC);
            let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);
            let mut text = String::from(cell.c);
            if let Some(marks) = cell.zerowidth() {
                text.extend(marks);
            }
            if boxdraw::is_drawn(cell.c) {
                drawn.push((row, col, cell.c, fg));
                continue;
            }
            // Only plain ASCII runs together: anything else may come from a
            // fallback font with its own width, and must stay on its cell.
            let ascii = cell.c.is_ascii();
            match spans.last_mut() {
                Some(last)
                    if ascii
                        && last.text.is_ascii()
                        && last.row == row
                        && last.col + last.text.chars().count() == col
                        && last.fg == fg
                        && last.weight == weight
                        && last.italic == italic
                        && last.underline == underline
                        && text.chars().count() == 1 =>
                {
                    last.text.push_str(&text)
                }
                _ => spans.push(Span { row, col, text, fg, weight, italic, underline }),
            }
        }

        let cursor = content.cursor;
        let at = (cursor.shape != CursorShape::Hidden && content.display_offset == 0)
            .then(|| ((cursor.point.line.0 + offset) as usize, cursor.point.column.0, cursor.shape));
        (fills, spans, drawn, at)
    }

    pub(super) fn paint_grid(&self, bounds: Bounds<Pixels>, font_base: &Font, window: &mut Window, cx: &mut App) {
        let (cell_w, cell_h) = (self.cell_w, self.cell_h);
        let at = |row: usize, col: usize| bounds.origin + point(cell_w * col as f32, cell_h * row as f32);
        window.paint_quad(fill(bounds, hsla(BACKGROUND)));

        let (fills, spans, drawn, cursor) = self.snapshot();
        for f in fills {
            window.paint_quad(fill(Bounds::new(at(f.row, f.col), size(cell_w * f.cols as f32, cell_h)), f.color));
        }

        let focused = self.focus.is_focused(window);
        if let Some((row, col, shape)) = cursor {
            let origin = at(row, col);
            let color = hsla(CURSOR);
            let quad = match shape {
                CursorShape::Beam => fill(Bounds::new(origin, size(px(1.5), cell_h)), color),
                CursorShape::Underline => fill(Bounds::new(origin + point(px(0.0), cell_h - px(2.0)), size(cell_w, px(2.0))), color),
                _ if focused => fill(Bounds::new(origin, size(cell_w, cell_h)), color),
                _ => outline(Bounds::new(origin, size(cell_w, cell_h)), color, BorderStyle::Solid),
            };
            window.paint_quad(quad);
        }

        for (row, col, c, color) in drawn {
            boxdraw::paint(c, at(row, col), size(cell_w, cell_h), hsla(color), window);
        }

        for span in spans {
            let mut font = font_base.clone();
            if span.text.chars().next().is_some_and(is_emoji) {
                font.fallbacks = None;
            }
            font.weight = span.weight;
            if span.italic {
                font.style = FontStyle::Italic;
            }
            let mut color = span.fg;
            if focused && cursor.is_some_and(|(row, col, shape)| matches!(shape, CursorShape::Block) && row == span.row && col == span.col && span.text.chars().count() == 1) {
                color = BACKGROUND;
            }
            let color = hsla(color);
            let underline = span.underline.then_some(UnderlineStyle { thickness: px(1.0), color: Some(color), wavy: false });
            let run = TextRun { len: span.text.len(), font, color, background_color: None, underline, strikethrough: None };
            let line = window.text_system().shape_line(span.text.into(), px(font_size()), &[run], None);
            let _ = line.paint(at(span.row, span.col), cell_h, TextAlign::Left, None, window, cx);
        }

        if let Some((row, col, _)) = cursor.filter(|_| !self.preedit.is_empty()) {
            let color = hsla(FOREGROUND);
            let underline = Some(UnderlineStyle { thickness: px(1.0), color: Some(color), wavy: false });
            let run = TextRun { len: self.preedit.len(), font: font_base.clone(), color, background_color: Some(hsla(BACKGROUND)), underline, strikethrough: None };
            let line = window.text_system().shape_line(self.preedit.clone().into(), px(font_size()), &[run], None);
            let _ = line.paint(at(row, col), cell_h, TextAlign::Left, None, window, cx);
        }
    }
}
