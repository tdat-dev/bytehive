//! The New agent sheet's parts, after the web app's dialog: the head (title,
//! where it starts, close), the agent choices, how many, the options
//! (project, flow, permissions, own worktree), and the footer (crews,
//! schedule, create more, Cancel, Start). Split from newtask_ui.rs to keep
//! each file one job.

use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke, Vec2};

use crate::icons::{paint_in, Icon};
use crate::newtask::MAX_AGENTS;
use crate::newtask_ui::{NewTaskSheet, SheetOut};
use crate::prefs::Crew;
use crate::projects::{normalize, project_name};
use crate::projects_ui::pick_folder;
use crate::sessions_ui::Cli;
use crate::style::{medium, on_white, tight, w500};
use crate::taskopts::{Flow, Perm};
use crate::theme::{DIM, INK3, TEXT};

fn same_dir(a: &str, b: &str) -> bool {
    normalize(a).eq_ignore_ascii_case(&normalize(b))
}

/// A field label (.im-label).
pub(crate) fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).font(w500(12.5)).color(DIM));
}

/// A choice pill (.im-choice): outlined, white when picked.
fn choice(ui: &mut egui::Ui, text: &str, on: bool, enabled: bool) -> egui::Response {
    let font = if on { on_white(13.0) } else { FontId::proportional(13.0) };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, DIM);
    let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 32.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let fade = if enabled { 1.0 } else { 0.4 };
    let ink = if on {
        p.rect_filled(rect, 16.0, Color32::WHITE);
        Color32::BLACK
    } else {
        p.rect_stroke(rect, 16.0, Stroke::new(1.0, Color32::from_white_alpha(if r.hovered() && enabled { 46 } else { 26 })), egui::StrokeKind::Inside);
        if r.hovered() && enabled { TEXT } else { DIM }
    };
    p.galley_with_override_text_color(rect.center() - g.size() / 2.0, g, ink.gamma_multiply(fade));
    r
}

/// A dialog button (.im-btn): quiet pill, or white when primary.
pub(crate) fn im_btn(ui: &mut egui::Ui, text: &str, primary: bool) -> egui::Response {
    let font = if primary { on_white(13.0) } else { medium(13.0) };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, DIM);
    let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 32.0, 36.0), Sense::click());
    let (fill, ink) = match (primary, r.hovered()) {
        (true, false) => (Color32::WHITE, Color32::BLACK),
        (true, true) => (Color32::from_rgb(0xe9, 0xe9, 0xec), Color32::BLACK),
        (false, false) => (Color32::from_white_alpha(15), DIM),
        (false, true) => (Color32::from_white_alpha(31), TEXT),
    };
    ui.painter().rect_filled(rect, 18.0, fill);
    ui.painter().galley_with_override_text_color(rect.center() - g.size() / 2.0, g, ink);
    r
}

/// A quiet round button with an icon (.im-x).
pub(crate) fn round_x(ui: &mut egui::Ui) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    ui.painter().rect_filled(rect, 15.0, Color32::from_white_alpha(if r.hovered() { 26 } else { 13 }));
    paint_in(ui.painter(), rect, 14.0, Icon::Close, if r.hovered() { TEXT } else { INK3 }, 1.8);
    r
}

impl NewTaskSheet {
    /// "New agent", where it starts (the project is a menu), and close.
    pub(crate) fn head(&mut self, ui: &mut egui::Ui, projects: &[String]) -> bool {
        let mut close = false;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - 40.0);
                let mut job = egui::text::LayoutJob::default();
                job.append("New agent", 0.0, tight(medium(20.0), TEXT, -0.02));
                ui.label(job);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(RichText::new("In").size(12.5).color(INK3));
                    let shown = RichText::new(project_name(&self.form.dir)).size(12.5).color(DIM);
                    egui::ComboBox::from_id_salt("nt-project").selected_text(shown).show_ui(ui, |ui| {
                        for dir in projects {
                            if ui.selectable_label(same_dir(&self.form.dir, dir), project_name(dir)).on_hover_text(dir).clicked() {
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
                    ui.add(egui::Label::new(RichText::new(format!("\u{00B7} {}", self.form.dir)).font(FontId::monospace(12.0)).color(INK3)).truncate());
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                close = round_x(ui).on_hover_text("Close (Esc)").clicked();
            });
        });
        close
    }

    /// The agent choices (.im-choices); missing CLIs are faded.
    pub(crate) fn agents(&mut self, ui: &mut egui::Ui) {
        label(ui, "Agent");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for cli in Cli::all() {
                let here = self.installed.as_ref().is_none_or(|v| v.contains(&cli));
                let r = choice(ui, cli.name(), self.form.cli == cli, here);
                if r.clicked() {
                    self.form.cli = cli;
                }
                if !here {
                    r.on_hover_text(format!("Not installed: {} isn't on PATH", cli.program()));
                }
            }
        });
    }

    /// How many (.im-seg), then how it works: flow and permissions.
    pub(crate) fn options(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.vertical(|ui| {
                label(ui, "How many");
                egui::Frame::new().fill(Color32::from_white_alpha(13)).corner_radius(999.0).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.horizontal(|ui| {
                        for n in 1..=MAX_AGENTS {
                            let on = self.form.count == n;
                            let (rect, r) = ui.allocate_exact_size(Vec2::new(36.0, 28.0), Sense::click());
                            if on || r.hovered() {
                                ui.painter().rect_filled(rect, 14.0, Color32::from_white_alpha(if on { 36 } else { 15 }));
                            }
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, n.to_string(), FontId::monospace(13.0), if on { TEXT } else { DIM });
                            if r.clicked() {
                                self.form.set_count(n);
                                // A race needs a branch per agent, or they write over each other.
                                if n > 1 {
                                    self.form.worktree = true;
                                }
                            }
                        }
                    });
                });
            });
            ui.vertical(|ui| {
                label(ui, "Flow");
                egui::ComboBox::from_id_salt("nt-flow").selected_text(RichText::new(self.form.flow.label()).size(13.0)).show_ui(ui, |ui| {
                    for f in Flow::ALL {
                        ui.selectable_value(&mut self.form.flow, f, f.label());
                    }
                });
            });
            ui.vertical(|ui| {
                label(ui, "Permissions");
                egui::ComboBox::from_id_salt("nt-perm").selected_text(RichText::new(self.form.perm.label()).size(13.0)).show_ui(ui, |ui| {
                    for p in Perm::ALL {
                        ui.selectable_value(&mut self.form.perm, p, p.label());
                    }
                })
                .response
                .on_hover_text("Accept edits: Claude and Codex only. Bypass: no prompts at all. Opencode sets this in its own config.");
            });
        });
        ui.add_space(4.0);
        ui.checkbox(&mut self.form.worktree, RichText::new("Each agent on its own branch (own worktree)").size(13.0).color(DIM));
    }

    /// Saved crews: apply one, or save these settings as one.
    pub(crate) fn crews(&mut self, ui: &mut egui::Ui, crews: &[Crew]) {
        let r = im_btn(ui, "Crews", false).on_hover_text("Apply saved settings, or save these");
        egui::Popup::menu(&r).show(|ui| {
            ui.set_min_width(240.0);
            if crews.is_empty() {
                ui.label(RichText::new("No saved crews yet.").size(12.5).color(INK3));
            }
            for c in crews {
                let line = format!("{}  \u{00B7}  {} \u{00D7} {}", c.name, c.count, c.cli.name());
                if ui.button(line).clicked() {
                    self.form.cli = c.cli;
                    self.form.set_count(c.count);
                    self.form.worktree = c.worktree;
                    self.form.perm = c.perm;
                    self.form.flow = c.flow;
                }
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut self.crew_name).hint_text("Name for these settings").desired_width(220.0));
            let name = self.crew_name.trim().to_string();
            if ui.add_enabled(!name.is_empty(), egui::Button::new("Save as crew")).clicked() {
                let f = &self.form;
                self.out = Some(SheetOut::SaveCrew(Crew { name, cli: f.cli, count: f.count, worktree: f.worktree, perm: f.perm, flow: f.flow }));
                self.crew_name.clear();
            }
        });
    }

    /// "Start at [09:00] [Daily]": start later instead of now.
    pub(crate) fn schedule_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.sched_on, RichText::new("Start later").size(13.0).color(DIM)).on_hover_text("Start it at a time of day, once or every day");
            if self.sched_on {
                ui.add(egui::TextEdit::singleline(&mut self.sched_time).desired_width(52.0).hint_text("09:00"));
                egui::ComboBox::from_id_salt("nt-repeat").selected_text(RichText::new(if self.sched_daily { "Daily" } else { "Once" }).size(13.0)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sched_daily, true, "Daily");
                    ui.selectable_value(&mut self.sched_daily, false, "Once");
                });
            }
            ui.add_space(8.0);
            ui.checkbox(&mut self.more, RichText::new("Create more").size(13.0).color(DIM)).on_hover_text("Keep this open after Start, for the next task");
        });
    }
}
