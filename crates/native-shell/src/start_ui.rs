//! The start screen, shown instead of the grid while no project is open,
//! after the approved inbox's "Start in a folder." view: one line saying what
//! happens, a Choose folder box, the recently closed projects, and the saved
//! crews to start with.

use eframe::egui::{self, Color32, RichText};

use crate::prefs::Crew;
use crate::projects::project_name;
use crate::projects_ui::pick_folder;
use crate::theme::{INK3, PRIMARY, PRIMARY_INK, R3, SURFACE, TEXT};

pub enum StartAction {
    /// Open this folder as a project.
    Open(String),
    /// Open this folder and start a New task with a saved crew.
    Crew(String, Crew),
}

fn semibold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).size(size).family(egui::FontFamily::Name("semibold".into()))
}

pub fn show_start(ui: &mut egui::Ui, recent: &[String], crews: &[Crew]) -> Option<StartAction> {
    let mut out = None;
    let width = 640.0_f32.min(ui.available_width() - 32.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.12).max(24.0));
            ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
                ui.set_width(width);
                ui.vertical(|ui| {
                    ui.label(semibold("Maestro", 13.0).color(INK3));
                    ui.add_space(10.0);
                    let mut h1 = egui::text::LayoutJob::default();
                    let font = egui::FontId::new(28.0, egui::FontFamily::Name("semibold".into()));
                    h1.append("Start in a folder. ", 0.0, egui::TextFormat::simple(font.clone(), TEXT));
                    h1.append("Agents work there, each on its own branch.", 0.0, egui::TextFormat::simple(font, INK3));
                    h1.wrap.max_width = width;
                    ui.label(h1);
                    ui.add_space(8.0);
                    ui.label(RichText::new("Pick a project you've used before, or choose a new folder. You can open more than one; the queue shows every project together.").size(13.5).color(INK3));
                    ui.add_space(18.0);
                    egui::Frame::new().fill(SURFACE).corner_radius(R3 + 2.0).inner_margin(egui::Margin::symmetric(18, 16)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(semibold("Choose a folder", 14.0).color(TEXT));
                                ui.label(RichText::new("Agents start there; nothing is copied or moved.").size(12.5).color(INK3));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let b = egui::Button::new(RichText::new("Choose folder\u{2026}").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0).min_size(egui::vec2(0.0, 36.0));
                                if ui.add(b).clicked() {
                                    if let Some(dir) = pick_folder(recent.first().map_or("", String::as_str)) {
                                        out = Some(StartAction::Open(dir));
                                    }
                                }
                            });
                        });
                    });
                    if !recent.is_empty() {
                        section(ui, "Recent", recent.len());
                        for dir in recent {
                            if row(ui, &project_name(dir), dir).clicked() {
                                out = Some(StartAction::Open(dir.clone()));
                            }
                        }
                    }
                    if !crews.is_empty() {
                        section(ui, "Start with a crew", crews.len());
                        for c in crews {
                            let what = format!("{} \u{00D7} {} \u{00B7} {} \u{00B7} {}", c.count, c.cli.name(), c.perm.label(), c.flow.label());
                            if row(ui, &c.name, &what).clicked() {
                                let start = recent.first().cloned().unwrap_or_default();
                                if let Some(dir) = pick_folder(&start) {
                                    out = Some(StartAction::Crew(dir, c.clone()));
                                }
                            }
                        }
                    }
                });
            });
        });
    });
    out
}

fn section(ui: &mut egui::Ui, name: &str, n: usize) {
    ui.add_space(22.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).size(12.5).color(INK3));
        ui.label(RichText::new(n.to_string()).size(12.5).color(INK3).monospace());
    });
    ui.add_space(4.0);
}

/// A clickable row: a bold name and a quiet second line.
fn row(ui: &mut egui::Ui, name: &str, sub: &str) -> egui::Response {
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.contains_pointer());
    let r = egui::Frame::new()
        .fill(if hovered { Color32::from_white_alpha(14) } else { Color32::from_white_alpha(6) })
        .corner_radius(R3)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(semibold(name, 13.5).color(TEXT));
            ui.label(RichText::new(sub).size(12.0).color(INK3));
        })
        .response;
    ui.add_space(4.0);
    ui.interact(r.rect, id, egui::Sense::click())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_screen_paints_with_and_without_history() {
        let ctx = egui::Context::default();
        for recent in [vec![], vec![r"D:\maestro".to_string()]] {
            let mut got = None;
            ctx.run_ui(egui::RawInput::default(), |ui| {
                got = show_start(ui, &recent, &[]);
            })
            .drop_without_applying_deltas();
            assert!(got.is_none(), "nothing happens without a click");
        }
    }
}
