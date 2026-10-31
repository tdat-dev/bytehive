//! The project chip row on the top bar: "All projects", then one chip per
//! open project with how many of its agents need you, then "+" to open a
//! folder. Right-click a chip to close that project. Same chips as the
//! approved inbox's queue head (`.chip`: small, round, quiet until pressed).

use eframe::egui::{self, Color32, RichText};

use crate::projects::{project_name, Projects};
use crate::theme::{AMBER, INK3, TEXT};

pub enum ProjectAction {
    Filter(Option<String>),
    Open(String),
    Close(String),
}

/// Pick a folder with the system dialog; none when cancelled.
pub fn pick_folder(start: &str) -> Option<String> {
    let mut d = rfd::FileDialog::new().set_title("Open a project folder");
    if !start.trim().is_empty() {
        d = d.set_directory(start);
    }
    d.pick_folder().map(|p| p.to_string_lossy().into_owned())
}

fn chip(ui: &mut egui::Ui, on: bool, label: &str, needs: usize) -> egui::Response {
    let mut text = egui::text::LayoutJob::default();
    let color = if on { TEXT } else { INK3 };
    let font = egui::FontId::proportional(12.0);
    text.append(label, 0.0, egui::TextFormat::simple(font.clone(), color));
    if needs > 0 {
        text.append(&format!(" \u{00B7} {needs}"), 0.0, egui::TextFormat::simple(font, AMBER));
    }
    let fill = if on { Color32::from_white_alpha(26) } else { Color32::from_white_alpha(8) };
    ui.add(egui::Button::new(text).fill(fill).corner_radius(999.0).stroke(egui::Stroke::NONE))
}

/// `homes`: each agent's home folder and whether it needs you.
pub fn show_chips(ui: &mut egui::Ui, projects: &Projects, homes: &[(String, bool)]) -> Option<ProjectAction> {
    let mut out = None;
    let needs_in = |dir: Option<&str>| {
        homes
            .iter()
            .filter(|(home, needs)| *needs && dir.is_none_or(|d| projects.owner(home) == Some(d)))
            .count()
    };
    if chip(ui, projects.filter.is_none(), "All projects", needs_in(None)).clicked() {
        out = Some(ProjectAction::Filter(None));
    }
    for dir in &projects.dirs {
        let on = projects.filter.as_deref() == Some(dir.as_str());
        let r = chip(ui, on, &project_name(dir), needs_in(Some(dir))).on_hover_text(dir);
        if r.clicked() {
            out = Some(ProjectAction::Filter(Some(dir.clone())));
        }
        r.context_menu(|ui| {
            if ui.button("Close project").clicked() {
                out = Some(ProjectAction::Close(dir.clone()));
                ui.close();
            }
        });
    }
    let start = projects.filter.clone().or_else(|| projects.dirs.last().cloned()).unwrap_or_default();
    if ui.add(egui::Button::new(RichText::new("+").color(INK3)).fill(Color32::from_white_alpha(8)).corner_radius(999.0)).on_hover_text("Open a project folder").clicked() {
        if let Some(dir) = pick_folder(&start) {
            out = Some(ProjectAction::Open(dir));
        }
    }
    out
}
