//! The top bar: the house button (back to the start screen), one sentence
//! that says what needs you, Resume all for stopped agents, any notice, and
//! the window's own buttons. The bar is where the window is dragged. Also
//! what a project action does to the app (filter, open, close).

use std::collections::HashSet;

use eframe::egui::{self, Color32, Rect, Sense, Vec2};

use crate::app::ShellApp;
use crate::icons::{paint_in, Icon};
use crate::projects::pane_home;
use crate::projects_ui::ProjectAction;
use crate::style::medium;
use crate::theme::{AMBER, INK3, TEXT};

/// The sentence on the top bar, as (lead, rest): "Nothing needs you." or
/// "2 agents need you.", then "1 is ready to review, 3 are working."
/// (needs you, ready to review, working, agents in all, plain shells open).
pub fn pulse(needs: usize, review: usize, working: usize, agents: usize, shells: usize) -> (String, String) {
    let are = |n: usize, what: &str| if n == 1 { format!("1 is {what}") } else { format!("{n} are {what}") };
    let lead = match needs {
        _ if agents == 0 => "No agents yet.".to_string(),
        0 => "Nothing needs you.".to_string(),
        1 => "1 agent needs you.".to_string(),
        n => format!("{n} agents need you."),
    };
    let mut rest = Vec::new();
    if review > 0 {
        rest.push(are(review, "ready to review"));
    }
    if working > 0 {
        rest.push(are(working, "working"));
    }
    // Shells are no agents, but "No agents yet." over four of them reads wrong.
    if agents == 0 && shells > 0 {
        return (lead, if shells == 1 { " 1 terminal open.".to_string() } else { format!(" {shells} terminals open.") });
    }
    let tail = if rest.is_empty() { String::new() } else { format!(" {}.", rest.join(", ")) };
    (lead, tail)
}

/// The Resume all pill: the arrow, the words, the count in a chip.
fn resume_pill(ui: &mut egui::Ui, n: usize) -> egui::Response {
    let p = ui.painter();
    let words = p.layout_no_wrap("Resume all".into(), medium(12.5), TEXT);
    let count = p.layout_no_wrap(n.to_string(), egui::FontId::new(11.0, egui::FontFamily::Name("bold".into())), TEXT);
    let chip_w = (count.size().x + 10.0).max(16.0);
    let w = 14.0 + 13.0 + 5.0 + words.size().x + 5.0 + chip_w + 14.0;
    let (rect, r) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 16.0, Color32::from_white_alpha(if r.hovered() { 31 } else { 15 }));
    let icon = Rect::from_min_size(egui::pos2(rect.left() + 14.0, rect.center().y - 6.5), Vec2::splat(13.0));
    paint_in(p, icon, 13.0, Icon::Refresh, TEXT, 2.0);
    let x = icon.right() + 5.0;
    p.galley(egui::pos2(x, rect.center().y - words.size().y / 2.0), words.clone(), TEXT);
    let chip = Rect::from_min_size(egui::pos2(x + words.size().x + 5.0, rect.center().y - 8.0), Vec2::new(chip_w, 16.0));
    p.rect_filled(chip, 8.0, Color32::from_rgba_unmultiplied(244, 244, 245, 66));
    p.galley(chip.center() - count.size() / 2.0, count, TEXT);
    r
}

impl ShellApp {
    /// The top bar; `counts` is (needs you, ready to review, working).
    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui, counts: (usize, usize, usize)) {
        crate::chrome::drag_area(ui, ui.max_rect());
        let agents = self.panes.iter().filter(|p| p.cli.is_some()).count();
        let shells = self.panes.iter().filter(|p| p.cli.is_none() && p.exit_code.is_none()).count();
        let (lead, tail) = pulse(counts.0, counts.1, counts.2, agents, shells);
        let stopped: Vec<usize> = (0..self.panes.len()).filter(|&i| self.panes[i].cli.is_some() && self.panes[i].exit_code.is_some()).collect();
        let summary = self.summary_line();
        let mut resume = false;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(crate::chrome::CONTROLS_W + 12.0);
            ui.spacing_mut().item_spacing.x = 10.0;
            if !stopped.is_empty() && resume_pill(ui, stopped.len()).on_hover_text("Start every stopped agent again, in its own session").clicked() {
                resume = true;
            }
            self.update_button(ui);
            if let Some(n) = self.notice.clone() {
                if ui.add(egui::Label::new(egui::RichText::new(n).size(12.5).color(AMBER)).sense(Sense::click()).truncate()).on_hover_text("Click to dismiss").clicked() {
                    self.notice = None;
                }
            }
            if !self.launching.is_empty() {
                ui.label(egui::RichText::new(format!("Making worktrees for {}\u{2026}", self.launching.join(", "))).size(12.5).color(INK3));
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                let home = crate::style::icon_btn(ui, Icon::House, 30.0, self.extras.home, false);
                if home.on_hover_text("Projects").clicked() {
                    self.extras.home = !self.extras.home;
                }
                ui.add_space(8.0);
                let mut job = egui::text::LayoutJob::default();
                job.append(&lead, 0.0, crate::style::tight(medium(24.0), TEXT, -0.02));
                job.append(&tail, 0.0, crate::style::tight(crate::style::w500(24.0), INK3, -0.02));
                ui.add(egui::Label::new(job).truncate()).on_hover_text(summary);
            });
        });
        crate::chrome::controls(ui);
        if resume {
            for i in stopped {
                self.restart_pane(i);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::pulse;

    #[test]
    fn says_what_needs_you_and_counts_shells() {
        assert_eq!(pulse(0, 0, 1, 1, 0), ("Nothing needs you.".into(), " 1 is working.".into()));
        assert_eq!(pulse(2, 1, 0, 3, 0), ("2 agents need you.".into(), " 1 is ready to review.".into()));
        assert_eq!(pulse(0, 0, 0, 0, 4), ("No agents yet.".into(), " 4 terminals open.".into()));
        assert_eq!(pulse(0, 0, 0, 0, 0), ("No agents yet.".into(), String::new()));
    }
}