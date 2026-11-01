//! The New task sheet's pill row: project, agent CLI, how many, own
//! worktree, permission mode, flow, and saved crews (apply one, or save the
//! row as a new one). Split from newtask_ui.rs to keep each file one job.

use eframe::egui::{self, RichText};

use crate::newtask::MAX_AGENTS;
use crate::newtask_ui::{NewTaskSheet, SheetOut};
use crate::prefs::Crew;
use crate::projects::{normalize, project_name};
use crate::projects_ui::pick_folder;
use crate::sessions_ui::Cli;
use crate::taskopts::{Flow, Perm};
use crate::theme::{DIM, INK3, PILL, PILL_HOVER, TEXT};

fn same_dir(a: &str, b: &str) -> bool {
    normalize(a).eq_ignore_ascii_case(&normalize(b))
}

fn lbl(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(INK3));
}

impl NewTaskSheet {
    pub(crate) fn props(&mut self, ui: &mut egui::Ui, projects: &[String], crews: &[Crew]) {
        pill(ui, |ui| {
            lbl(ui, "Project");
            let shown = RichText::new(project_name(&self.form.dir)).size(12.5);
            let r = egui::ComboBox::from_id_salt("nt-project").selected_text(shown).show_ui(ui, |ui| {
                for dir in projects {
                    let on = same_dir(&self.form.dir, dir);
                    if ui.selectable_label(on, project_name(dir)).on_hover_text(dir).clicked() {
                        self.form.dir = dir.clone();
                    }
                }
                ui.separator();
                if ui.button("Choose folder\u{2026}").clicked() {
                    if let Some(dir) = pick_folder(&self.form.dir) {
                        self.form.dir = dir;
                    }
                }
            });
            r.response.on_hover_text(&self.form.dir);
        });
        pill(ui, |ui| {
            lbl(ui, "Agent");
            egui::ComboBox::from_id_salt("nt-cli").selected_text(RichText::new(self.form.cli.name()).size(12.5)).show_ui(ui, |ui| {
                for cli in Cli::all() {
                    let here = self.installed.as_ref().is_none_or(|v| v.contains(&cli));
                    let r = ui.add_enabled(here, egui::Button::selectable(self.form.cli == cli, cli.name()));
                    if r.clicked() {
                        self.form.cli = cli;
                    }
                    r.on_disabled_hover_text(format!("Not installed: {} isn't on PATH", cli.program()));
                }
            });
        });
        pill(ui, |ui| {
            let n = self.form.count;
            if ui.add_enabled(n > 1, egui::Button::new("\u{2212}").frame(false)).on_hover_text("Fewer agents").clicked() {
                self.form.set_count(n - 1);
            }
            ui.label(RichText::new(n.to_string()).monospace().strong().size(12.5).color(TEXT));
            if ui.add_enabled(n < MAX_AGENTS, egui::Button::new("+").frame(false)).on_hover_text("More agents: a race").clicked() {
                self.form.set_count(n + 1);
            }
            lbl(ui, if self.form.count == 1 { "agent" } else { "agents" });
        });
        pill(ui, |ui| {
            ui.checkbox(&mut self.form.worktree, RichText::new("Own worktree").size(12.5).color(DIM));
        });
        pill(ui, |ui| {
            lbl(ui, "Permissions");
            egui::ComboBox::from_id_salt("nt-perm").selected_text(RichText::new(self.form.perm.label()).size(12.5)).show_ui(ui, |ui| {
                for p in Perm::ALL {
                    ui.selectable_value(&mut self.form.perm, p, p.label());
                }
            });
        })
        .on_hover_text("Accept edits: Claude and Codex only. Bypass: no prompts at all. Opencode sets this in its own config.");
        pill(ui, |ui| {
            lbl(ui, "Flow");
            egui::ComboBox::from_id_salt("nt-flow").selected_text(RichText::new(self.form.flow.label()).size(12.5)).show_ui(ui, |ui| {
                for f in Flow::ALL {
                    ui.selectable_value(&mut self.form.flow, f, f.label());
                }
            });
        });
        pill(ui, |ui| self.crew_pill(ui, crews));
    }

    /// Apply a saved crew, or save the current row under a name.
    fn crew_pill(&mut self, ui: &mut egui::Ui, crews: &[Crew]) {
        lbl(ui, "Crew");
        egui::ComboBox::from_id_salt("nt-crew").selected_text(RichText::new("Saved").size(12.5)).show_ui(ui, |ui| {
            if crews.is_empty() {
                ui.label(RichText::new("No saved crews yet.").size(12.5).color(INK3));
            }
            for c in crews {
                let line = format!("{}  \u{00B7}  {} \u{00D7} {}", c.name, c.count, c.cli.name());
                if ui.selectable_label(false, line).clicked() {
                    self.form.cli = c.cli;
                    self.form.set_count(c.count);
                    self.form.worktree = c.worktree;
                    self.form.perm = c.perm;
                    self.form.flow = c.flow;
                }
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut self.crew_name).hint_text("Name for these settings").desired_width(200.0));
            let name = self.crew_name.trim().to_string();
            if ui.add_enabled(!name.is_empty(), egui::Button::new("Save as crew")).clicked() {
                let f = &self.form;
                self.out = Some(SheetOut::SaveCrew(Crew { name, cli: f.cli, count: f.count, worktree: f.worktree, perm: f.perm, flow: f.flow }));
                self.crew_name.clear();
            }
        });
    }
}

/// A rounded property pill; brighter while hovered.
fn pill(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.contains_pointer());
    let r = egui::Frame::NONE
        .fill(if hovered { PILL_HOVER } else { PILL })
        .corner_radius(999.0)
        .inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 4 })
        .show(ui, |ui| {
            ui.set_min_height(24.0);
            ui.horizontal_centered(add);
        })
        .response;
    ui.interact(r.rect, id, egui::Sense::hover())
}
