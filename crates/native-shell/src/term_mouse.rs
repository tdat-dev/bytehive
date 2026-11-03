//! Clicks for a program that asked for the mouse (OpenCode, vim with
//! mouse=a, htop): a real terminal reports them to it instead of selecting.
//! Shift held still selects, as in every terminal.

use alacritty_terminal::term::TermMode;
use eframe::egui::{self, Pos2, Rect};

/// The button a press went down with, and the last cell reported.
#[derive(Default)]
pub struct MouseReport {
    down: Option<u8>,
    last: Option<(usize, usize)>,
}

impl MouseReport {
    /// Whether this frame's pointer belongs to the program: it asked for the
    /// mouse and Shift is not held, or a press it got is still down.
    pub fn takes(&self, mode: TermMode, shift: bool) -> bool {
        self.down.is_some() || (mode.intersects(TermMode::MOUSE_MODE) && !shift)
    }

    /// This frame's pointer events over `rect` as the program's reports.
    pub fn report(&mut self, events: &[egui::Event], mode: TermMode, rect: Rect, cell: (f32, f32), size: (usize, usize)) -> Vec<u8> {
        let at = |p: Pos2| {
            let col = (((p.x - rect.left()) / cell.0).max(0.0) as usize).min(size.0.saturating_sub(1));
            let row = (((p.y - rect.top()) / cell.1).max(0.0) as usize).min(size.1.saturating_sub(1));
            (col + 1, row + 1)
        };
        let mut out = Vec::new();
        for ev in events {
            match ev {
                egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                    let Some(b) = button_code(*button) else { continue };
                    let b = b | mods_code(*modifiers);
                    if *pressed && rect.contains(*pos) && self.down.is_none() {
                        self.down = Some(b);
                        self.last = Some(at(*pos));
                        out.extend(encode(mode, b, at(*pos), true));
                    } else if !*pressed && self.down.is_some_and(|d| d & 3 == b & 3) {
                        self.down = None;
                        out.extend(encode(mode, b, at(*pos), false));
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    let c = at(*pos);
                    if self.last == Some(c) {
                        continue;
                    }
                    let b = match self.down {
                        Some(d) if mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) => d + 32,
                        None if mode.contains(TermMode::MOUSE_MOTION) && rect.contains(*pos) => 35,
                        _ => continue,
                    };
                    self.last = Some(c);
                    out.extend(encode(mode, b, c, true));
                }
                _ => {}
            }
        }
        out
    }
}

fn button_code(b: egui::PointerButton) -> Option<u8> {
    match b {
        egui::PointerButton::Primary => Some(0),
        egui::PointerButton::Middle => Some(1),
        egui::PointerButton::Secondary => Some(2),
        _ => None,
    }
}

fn mods_code(m: egui::Modifiers) -> u8 {
    8 * u8::from(m.alt) + 16 * u8::from(m.ctrl)
}

/// One report, SGR when the program asked for it, else the X10 bytes.
/// `cell` is 1-based (column, row).
fn encode(mode: TermMode, b: u8, cell: (usize, usize), pressed: bool) -> Vec<u8> {
    if mode.contains(TermMode::SGR_MOUSE) {
        return format!("\x1b[<{b};{};{}{}", cell.0, cell.1, if pressed { 'M' } else { 'm' }).into_bytes();
    }
    let b = if pressed { b } else { (b & !3) | 3 };
    let c = |v: usize| (32 + v.min(223)) as u8;
    vec![0x1b, b'[', b'M', 32 + b, c(cell.0), c(cell.1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(x: f32, y: f32, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: Pos2::new(x, y), button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    #[test]
    fn a_click_and_drag_reach_a_program_that_asked() {
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 100.0));
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::SGR_MOUSE;
        let mut m = MouseReport::default();
        assert!(m.takes(mode, false));
        assert!(!m.takes(mode, true), "Shift selects");
        assert!(!m.takes(TermMode::empty(), false));
        let out = m.report(&[press(15.0, 25.0, true)], mode, rect, (10.0, 10.0), (20, 10));
        assert_eq!(out, b"\x1b[<0;2;3M");
        let out = m.report(&[egui::Event::PointerMoved(Pos2::new(35.0, 25.0))], mode, rect, (10.0, 10.0), (20, 10));
        assert_eq!(out, b"\x1b[<32;4;3M");
        let out = m.report(&[press(35.0, 25.0, false)], mode, rect, (10.0, 10.0), (20, 10));
        assert_eq!(out, b"\x1b[<0;4;3m");
        assert!(!m.takes(TermMode::empty(), false), "released");
    }

    #[test]
    fn x10_reports_release_as_button_three() {
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 100.0));
        let mut m = MouseReport::default();
        let mode = TermMode::MOUSE_REPORT_CLICK;
        assert_eq!(m.report(&[press(0.0, 0.0, true)], mode, rect, (10.0, 10.0), (20, 10)), [0x1b, b'[', b'M', 32, 33, 33]);
        assert_eq!(m.report(&[press(0.0, 0.0, false)], mode, rect, (10.0, 10.0), (20, 10)), [0x1b, b'[', b'M', 35, 33, 33]);
    }
}
