//! The agent list folded down to a rail: each agent as its face with a state
//! dot, the one on stage ringed. The sidebar button on the top bar folds and
//! unfolds it; a face puts its agent on stage.

use std::collections::HashSet;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::ShellApp;
use crate::inbox::panel;
use crate::queue_ui::QueueRow;
use crate::style::GUTTER;
use crate::theme::{state_dot, state_word};

/// The rail's width, faces and margins together.
pub const RAIL_W: f32 = 60.0;

impl ShellApp {
    pub(crate) fn inbox_rail(&mut self, ui: &mut egui::Ui, rows: &[QueueRow], hidden: &HashSet<String>) {
        let mut pick = None;
        let frame = egui::Frame::NONE.inner_margin(egui::Margin { left: GUTTER as i8, right: 0, top: 0, bottom: 0 });
        egui::Panel::left("inbox-rail").exact_size(RAIL_W + GUTTER).resizable(false).show_separator_line(false).frame(frame).show(ui, |ui| {
            panel(ui, |ui| {
                let top = ui.available_rect_before_wrap().top() + 10.0;
                let cx = ui.available_rect_before_wrap().center().x;
                let mut y = top;
                for r in rows.iter().filter(|r| !hidden.contains(&r.pane_id)) {
                    let Some(p) = self.panes.iter().find(|p| p.id == r.pane_id) else { continue };
                    let at = Rect::from_center_size(Pos2::new(cx, y + 20.0), Vec2::splat(36.0));
                    let resp = ui.interact(at.expand(4.0), ui.make_persistent_id(("rail-face", &p.id)), Sense::click());
                    let on = self.stage.as_deref() == Some(p.id.as_str());
                    if on || resp.hovered() {
                        ui.painter().rect_filled(at.expand(4.0), 12.0, Color32::from_white_alpha(if on { 22 } else { 12 }));
                    }
                    crate::face::avatar(ui.painter(), at, p.cli, &p.name, 10.0, p.exit_code.is_some());
                    let dot = at.right_bottom() - Vec2::splat(3.0);
                    ui.painter().circle(dot, 4.5, state_dot(r.status.state), Stroke::new(2.0, crate::style::PANEL));
                    if resp.on_hover_text(format!("{} \u{00B7} {}", p.name, state_word(r.status.state))).clicked() {
                        pick = Some(p.id.clone());
                    }
                    y += 48.0;
                }
            });
        });
        if let Some(id) = pick {
            self.stage = Some(id.clone());
            self.focus_pane = Some(id);
            self.board.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run one frame of the top bar with the pointer pressed or let go at `at`.
    fn frame(ctx: &egui::Context, app: &mut ShellApp, at: Pos2, down: Option<bool>) {
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 46.0))), ..Default::default() };
        input.events.push(egui::Event::PointerMoved(at));
        if let Some(pressed) = down {
            input.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
        }
        ctx.run_ui(input, |ui| app.top_bar(ui, (0, 0, 0))).drop_without_applying_deltas();
    }

    #[test]
    fn the_sidebar_button_folds_the_list() {
        let mut app = ShellApp::new().unwrap();
        let ctx = egui::Context::default();
        let at = Pos2::new(67.0, 31.0);
        frame(&ctx, &mut app, at, None);
        frame(&ctx, &mut app, at, Some(true));
        frame(&ctx, &mut app, at, Some(false));
        frame(&ctx, &mut app, at, None);
        assert!(app.extras.side_mini, "a click on the sidebar button folds the list");
    }
}
