//! The dock of the approved design, along the bottom: the views (Queue, one
//! agent on the stage; Split, every agent side by side; Board), the command
//! bar that opens Ctrl+K, New task, and Files, Memory, Settings.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::ShellApp;
use crate::style::{hbtn, primary, DOCK_H};
use crate::theme::{DIM, INK3};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Queue,
    Split,
}

const BAR: Color32 = Color32::from_rgb(24, 25, 29);

/// A floating group (.views): pills on the dock's dark glass.
fn group(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(BAR)
        .corner_radius(999.0)
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(20)))
        .shadow(egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) })
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.horizontal(add);
        });
}

/// The command bar (.kbtn): a search mark, the hint, the key.
fn command_bar(ui: &mut egui::Ui) -> bool {
    let (rect, r) = ui.allocate_exact_size(Vec2::new(400.0_f32.min(ui.available_width() * 0.4), 40.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 13.0, BAR);
    p.rect_stroke(rect, 13.0, Stroke::new(1.0, Color32::from_white_alpha(if r.hovered() { 36 } else { 20 })), egui::StrokeKind::Inside);
    let ink = if r.hovered() { DIM } else { INK3 };
    let c = Pos2::new(rect.left() + 21.0, rect.center().y - 1.0);
    p.circle_stroke(c, 5.0, Stroke::new(1.5, ink));
    p.line_segment([c + Vec2::new(3.6, 3.6), c + Vec2::new(7.5, 7.5)], Stroke::new(1.5, ink));
    p.text(Pos2::new(rect.left() + 36.0, rect.center().y), egui::Align2::LEFT_CENTER, "Ask, jump, or run a command", FontId::proportional(13.0), ink);
    let k = p.layout_no_wrap("Ctrl K".into(), FontId::monospace(11.0), INK3);
    let kr = Rect::from_min_size(Pos2::new(rect.right() - 12.0 - k.size().x - 10.0, rect.center().y - 10.0), Vec2::new(k.size().x + 10.0, 20.0));
    p.rect_stroke(kr, 5.0, Stroke::new(1.0, Color32::from_white_alpha(23)), egui::StrokeKind::Inside);
    p.galley(kr.center() - k.size() / 2.0, k, INK3);
    r.on_hover_text("Agents, commands and settings (Ctrl+K)").clicked()
}

impl ShellApp {
    pub(crate) fn dock(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("dock").exact_size(DOCK_H).frame(egui::Frame::NONE).show_separator_line(false).show(ui, |ui| {
            let w = self.extras.dock_w;
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - w) / 2.0).max(16.0));
                let start = ui.cursor().left();
                ui.spacing_mut().item_spacing.x = 10.0;
                group(ui, |ui| {
                    let board = self.board.open;
                    for (label, view) in [("Queue", View::Queue), ("Split", View::Split)] {
                        if hbtn(ui, label, !board && self.extras.view == view).clicked() {
                            self.extras.view = view;
                            self.board.open = false;
                        }
                    }
                    if hbtn(ui, "Board", board).on_hover_text("Agents by state, and the plan board").clicked() {
                        self.board.open = !board;
                    }
                });
                if command_bar(ui) {
                    self.palette.toggle();
                }
                if primary(ui, "New task", Some("Ctrl N"), 40.0).clicked() {
                    self.run_command("new");
                }
                group(ui, |ui| {
                    if hbtn(ui, "Files", self.files.open).on_hover_text("Files on this branch").clicked() {
                        self.run_command("files");
                    }
                    if hbtn(ui, "Memory", self.extras.memory.open).on_hover_text("What every agent in this project knows").clicked() {
                        self.run_command("memory");
                    }
                    if hbtn(ui, "Settings", self.settings.open).on_hover_text("Settings (Ctrl+,)").clicked() {
                        self.run_command("settings");
                    }
                    self.update_button(ui);
                });
                self.extras.dock_w = ui.cursor().left() - start;
            });
        });
    }
}
