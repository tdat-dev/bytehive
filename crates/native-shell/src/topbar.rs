//! The top bar of the approved design: the brand, one sentence that says
//! what needs you, what the agents on show cost, and any notice. Also what a
//! project chip does to the app (filter, open, close).

use std::collections::HashSet;

use eframe::egui;

use crate::app::ShellApp;
use crate::projects::pane_home;
use crate::projects_ui::ProjectAction;
use crate::style::{medium, GUTTER};
use crate::theme::{AMBER, INK3, TEXT};

/// The sentence on the top bar: what needs you first, then the rest.
/// (needs you, ready to review, working, agents in all).
pub fn pulse(needs: usize, review: usize, working: usize, agents: usize) -> (String, String) {
    let are = |n: usize, what: &str| if n == 1 { format!("1 is {what}") } else { format!("{n} are {what}") };
    let mut rest = Vec::new();
    let lead = if needs > 0 {
        if review > 0 {
            rest.push(are(review, "ready to review"));
        }
        if working > 0 {
            rest.push(are(working, "working"));
        }
        if needs == 1 { "1 agent needs you.".to_string() } else { format!("{needs} agents need you.") }
    } else if review > 0 {
        if working > 0 {
            rest.push(are(working, "working"));
        }
        if review == 1 { "1 agent is ready to review.".to_string() } else { format!("{review} agents are ready to review.") }
    } else if working > 0 {
        if working == 1 { "1 agent is working.".to_string() } else { format!("{working} agents are working.") }
    } else if agents == 0 {
        "No agents yet.".to_string()
    } else {
        "All clear.".to_string()
    };
    let tail = if rest.is_empty() { String::new() } else { format!(" {}.", rest.join(", ")) };
    (lead, tail)
}

impl ShellApp {
    fn logo(&mut self, ctx: &egui::Context) -> egui::TextureId {
        if self.extras.logo.is_none() {
            let img = image::load_from_memory(include_bytes!("../assets/icons/32x32.png")).map(|i| i.into_rgba8()).unwrap_or_default();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            self.extras.logo = Some(ctx.load_texture("logo", color, egui::TextureOptions::LINEAR));
        }
        self.extras.logo.as_ref().map(|t| t.id()).unwrap_or_default()
    }

    /// The top bar of the approved design: the brand, the sentence that says
    /// what needs you, and on the right what it cost, plus any notice.
    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui, counts: (usize, usize, usize)) {
        let agents = self.panes.iter().filter(|p| p.cli.is_some()).count();
        let (lead, tail) = pulse(counts.0, counts.1, counts.2, agents);
        let logo = self.logo(ui.ctx());
        let summary = self.summary_line();
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.add(egui::Image::new((logo, egui::vec2(22.0, 22.0))).corner_radius(6.0));
            ui.label(egui::RichText::new("ByteHive").font(medium(15.0)).color(TEXT));
            ui.add_space(22.0);
            let mut job = egui::text::LayoutJob::default();
            job.append(&lead, 0.0, egui::TextFormat::simple(medium(22.0), TEXT));
            job.append(&tail, 0.0, egui::TextFormat::simple(medium(22.0), INK3));
            ui.add(egui::Label::new(job).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(GUTTER);
                ui.label(egui::RichText::new(&summary).font(egui::FontId::monospace(12.0)).color(INK3))
                    .on_hover_text("Cost is what the same work would cost at API list prices (Claude), or what the CLI reports. A subscription does not bill per token.");
                if let Some(n) = self.notice.clone() {
                    ui.add_space(14.0);
                    if ui.add(egui::Label::new(egui::RichText::new(n).size(12.5).color(AMBER)).sense(egui::Sense::click()).truncate()).on_hover_text("Click to dismiss").clicked() {
                        self.notice = None;
                    }
                }
                if !self.launching.is_empty() {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(format!("Making worktrees for {}\u{2026}", self.launching.join(", "))).size(12.5).color(INK3));
                }
            });
        });
    }

    pub(crate) fn apply_project(&mut self, action: ProjectAction) {
        self.stash_stage();
        match action {
            ProjectAction::Filter(f) => self.projects.filter = f,
            ProjectAction::Open(dir) => {
                let d = self.projects.open(&dir);
                self.projects.filter = Some(d);
            }
            ProjectAction::Close(dir) => self.projects.close(&dir),
        }
        // Sessions and new tasks follow the project on show.
        if let Some(f) = self.projects.filter.clone() {
            self.sessions.set_dir(&f);
        }
        self.restore_stage();
        self.save_projects();
    }

    pub(crate) fn save_projects(&self) {
        if let Some(path) = &self.projects_path {
            self.projects.save(path);
        }
    }

    /// "$1.23 · 4 agents · 2 projects" for what is on show.
    fn summary_line(&self) -> String {
        let hidden = self.hidden_panes();
        let agents = self.panes.iter().filter(|p| p.cli.is_some() && !hidden.contains(&p.id)).count();
        let n = self.projects.dirs.len();
        let mut parts = Vec::new();
        if let Some(usd) = self.total_usd(&hidden) {
            parts.push(supervise::pricing::money(usd));
        }
        parts.push(format!("{agents} agent{}", if agents == 1 { "" } else { "s" }));
        parts.push(format!("{n} project{}", if n == 1 { "" } else { "s" }));
        parts.join(" \u{00B7} ")
    }

    /// Panes the project filter hides from the grid and the Queue.
    pub(crate) fn hidden_panes(&self) -> HashSet<String> {
        self.panes.iter().filter(|p| !self.projects.shows(&pane_home(p))).map(|p| p.id.clone()).collect()
    }
}
