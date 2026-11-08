//! Mouse, selection, the wheel and Ctrl+V: what today's xterm pane does with them.

use super::*;

pub(super) fn mouse_code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => 0,
    }
}

impl Terminal {
    /// The cell under a window position, and which half of it.
    pub(super) fn cell_at(&self, position: Point<Pixels>) -> (usize, usize, Side) {
        let x = f32::from(position.x - self.origin.x).max(0.0) / f32::from(self.cell_w);
        let y = f32::from(position.y - self.origin.y).max(0.0) / f32::from(self.cell_h);
        let col = (x as usize).min(self.cols - 1);
        let row = (y as usize).min(self.rows - 1);
        (col, row, if x.fract() < 0.5 { Side::Left } else { Side::Right })
    }

    /// A program holding the mouse gets it, unless Shift asks for a selection.
    pub(super) fn program_has_mouse(&self, modifiers: &Modifiers) -> bool {
        !modifiers.shift && self.term.lock().unwrap().mode().intersects(TermMode::MOUSE_MODE)
    }

    pub(super) fn report_mouse(&self, button: u8, col: usize, row: usize, pressed: bool) {
        self.write(format!("\x1b[<{button};{};{}{}", col + 1, row + 1, if pressed { 'M' } else { 'm' }));
    }

    pub(super) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        let (col, row, side) = self.cell_at(event.position);
        if event.button == MouseButton::Right {
            self.copy_selection(cx);
            return;
        }
        if self.program_has_mouse(&event.modifiers) {
            self.report_mouse(mouse_code(event.button), col, row, true);
            return;
        }
        if event.button == MouseButton::Left {
            let mut term = self.term.lock().unwrap();
            let point = Cell::new(Line(row as i32 - term.grid().display_offset() as i32), Column(col));
            let kind = match event.click_count {
                2 => SelectionType::Semantic,
                3 => SelectionType::Lines,
                _ => SelectionType::Simple,
            };
            term.selection = Some(Selection::new(kind, point, side));
            self.selecting = true;
            cx.notify();
        }
    }

    pub(super) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (col, row, side) = self.cell_at(event.position);
        if self.selecting {
            let mut term = self.term.lock().unwrap();
            let point = Cell::new(Line(row as i32 - term.grid().display_offset() as i32), Column(col));
            if let Some(selection) = term.selection.as_mut() {
                selection.update(point, side);
            }
            cx.notify();
        } else if event.pressed_button.is_some() && self.term.lock().unwrap().mode().intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
            self.report_mouse(mouse_code(event.pressed_button.unwrap_or(MouseButton::Left)) + 32, col, row, true);
        }
    }

    pub(super) fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.selecting = false;
            self.copy_selection(cx);
        } else if self.program_has_mouse(&event.modifiers) {
            let (col, row, _) = self.cell_at(event.position);
            self.report_mouse(mouse_code(event.button), col, row, false);
        }
    }

    pub(super) fn copy_selection(&self, cx: &mut Context<Self>) {
        let text = self.term.lock().unwrap().selection_to_string();
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub(super) fn on_wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.wheel += match event.delta {
            ScrollDelta::Lines(lines) => lines.y * 3.0,
            ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / f32::from(self.cell_h),
        };
        let lines = self.wheel as i32;
        if lines == 0 {
            return;
        }
        self.wheel -= lines as f32;
        let (col, row, _) = self.cell_at(event.position);
        let mode = *self.term.lock().unwrap().mode();
        if self.program_has_mouse(&event.modifiers) {
            for _ in 0..lines.abs() {
                self.report_mouse(if lines > 0 { 64 } else { 65 }, col, row, true);
            }
        } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let arrow = match (lines > 0, mode.contains(TermMode::APP_CURSOR)) {
                (true, false) => "\x1b[A",
                (false, false) => "\x1b[B",
                (true, true) => "\x1bOA",
                (false, true) => "\x1bOB",
            };
            self.write(arrow.repeat(lines.unsigned_abs() as usize));
        } else {
            self.term.lock().unwrap().scroll_display(Scroll::Delta(lines));
            cx.notify();
        }
    }

    /// Only Ctrl+V reaches here: keys.rs hands every other key to the program.
    pub(super) fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control && keystroke.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.paste_text(&text);
            }
            cx.stop_propagation();
        }
    }

    pub(super) fn paste_text(&self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.term.lock().unwrap().mode().contains(TermMode::BRACKETED_PASTE) {
            self.write(format!("\x1b[200~{text}\x1b[201~"));
        } else {
            self.write(text);
        }
    }
}
