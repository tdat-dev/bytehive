//! The Queue panel of the approved design: a floating panel down the left
//! with the project chips, then every agent grouped by what it needs from
//! you (Needs you, Ready to review, Working, Free, Merged). A row is the
//! agent's avatar, its task, how long since it last moved, and one line in
//! the state's color. Picking a row puts the agent on the stage. Sessions,
//! Flow and Review open in the same panel, with a way back to the Queue.

use std::collections::HashSet;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Vec2};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::commands::SideTab;
use crate::projects::pane_home;
use crate::projects_ui::show_chips;
use crate::queue_ui::{is_attention, QueueRow};
use crate::states::pane_status;
use crate::style::{cli_hue, medium, GUTTER, PANEL, RAISE};
use crate::term_pane::Pane;
use crate::theme::{ADD, AMBER, DEL, FOCUS, INK3, TEXT};

pub const QUEUE_W: f32 = 340.0;

/// The groups, in order: title and the states in it.
const GROUPS: [(&str, &[TaskState]); 5] = [
    ("Needs you", &[TaskState::Needs, TaskState::Blocked]),
    ("Ready to review", &[TaskState::Review]),
    ("Working", &[TaskState::Working]),
    ("Free", &[TaskState::Idle, TaskState::Stopped]),
    ("Merged", &[TaskState::Merged]),
];

/// "now", "4m", "2h", "3d" since the agent last moved.
pub fn since(ms: u64, now: u64) -> String {
    let s = now.saturating_sub(ms) / 1000;
    match s {
        0..60 => "now".into(),
        60..3600 => format!("{}m", s / 60),
        3600..86_400 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

/// The color a row's line takes for its state.
pub fn line_color(state: TaskState) -> Color32 {
    match state {
        TaskState::Needs => AMBER,
        TaskState::Blocked => DEL,
        TaskState::Review => FOCUS,
        _ => INK3,
    }
}

/// A floating panel filling `ui`: the design's --panel at --r-xl.
pub fn panel<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(PANEL)
        .corner_radius(18.0)
        .stroke(egui::Stroke::new(1.0, Color32::from_white_alpha(12)))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            add(ui)
        })
        .inner
}

/// The avatar square: the agent's first letter on its CLI's hue.
pub fn avatar(ui: &mut egui::Ui, at: Rect, cli: Option<crate::sessions_ui::Cli>, name: &str, radius: f32) {
    ui.painter().rect_filled(at, radius, cli_hue(cli));
    let letter: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    ui.painter().text(at.center(), egui::Align2::CENTER_CENTER, letter, FontId::new(at.height() * 0.45, egui::FontFamily::Name("bold".into())), Color32::from_rgb(5, 5, 6));
}

/// One queue row; true when clicked.
fn row(ui: &mut egui::Ui, p: &Pane, r: &QueueRow, files: Option<usize>, on: bool, now: u64) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 54.0), Sense::click());
    if on {
        ui.painter().rect_filled(rect, 12.0, RAISE);
        ui.painter().rect_stroke(rect, 12.0, egui::Stroke::new(1.0, Color32::from_white_alpha(26)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 12.0, Color32::from_white_alpha(8));
    }
    let inner = rect.shrink2(Vec2::new(10.0, 9.0));
    avatar(ui, Rect::from_min_size(inner.min + Vec2::new(0.0, 1.0), Vec2::splat(24.0)), p.cli, &p.name, 7.0);
    let left = inner.left() + 34.0;
    let time = since(p.last_output_ms, now);
    let tw = ui.painter().layout_no_wrap(time.clone(), FontId::monospace(12.0), INK3).rect.width();
    ui.painter().text(Pos2::new(inner.right(), inner.top() + 1.0), egui::Align2::RIGHT_TOP, time, FontId::monospace(12.0), INK3);
    let title = p.task.clone().unwrap_or_else(|| p.name.clone());
    let title = ui.painter().layout(title, medium(13.5), TEXT, (inner.right() - tw - 10.0 - left).max(40.0));
    let title = if title.rows.len() > 1 { ui.painter().layout_no_wrap(fit(&title.text(), 34), medium(13.5), TEXT) } else { title };
    ui.painter().galley(Pos2::new(left, inner.top()), title, TEXT);
    let mut sub = egui::text::LayoutJob::default();
    let name = if p.task.is_some() { format!("{} \u{00B7} ", p.name) } else { String::new() };
    sub.append(&name, 0.0, egui::TextFormat::simple(FontId::proportional(12.5), INK3));
    let line = r.line.strip_prefix(&format!("{} ", p.name)).unwrap_or(&r.line).to_string();
    sub.append(&line, 0.0, egui::TextFormat::simple(FontId::proportional(12.5), line_color(r.status.state)));
    sub.wrap = egui::text::TextWrapping::truncate_at_width(inner.right() - left - 70.0);
    ui.painter().galley(Pos2::new(left, inner.top() + 21.0), ui.painter().layout_job(sub), INK3);
    if let Some(n) = files.filter(|n| *n > 0 && r.status.state == TaskState::Review) {
        ui.painter().text(Pos2::new(inner.right(), inner.top() + 21.0), egui::Align2::RIGHT_TOP, format!("{n} file{}", if n == 1 { "" } else { "s" }), FontId::monospace(12.0), ADD);
    }
    resp.clicked()
}

fn fit(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}\u{2026}", s.chars().take(n).collect::<String>()) }
}

impl ShellApp {
    /// The left panel; a picked agent goes on the stage.
    pub(crate) fn inbox_panel(&mut self, ui: &mut egui::Ui, rows: &[QueueRow], hidden: &HashSet<String>) {
        let mut pick = None;
        let mut project = None;
        let now = now_ms();
        egui::Panel::left("inbox").exact_size(QUEUE_W + GUTTER).resizable(false).show_separator_line(false).frame(egui::Frame::NONE.inner_margin(egui::Margin { left: GUTTER as i8, right: 0, top: 0, bottom: 0 })).show(ui, |ui| {
            panel(ui, |ui| {
                egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 12, top: 12, bottom: 10 }).show(ui, |ui| {
                    if self.side_tab == SideTab::History {
                        // History lives on the stage now.
                        self.extras.stage.history = true;
                        self.side_tab = SideTab::Queue;
                    }
                    if self.side_tab != SideTab::Queue {
                        self.side_mode(ui);
                        return;
                    }
                    let homes: Vec<(String, bool)> = self.panes.iter().map(|p| (pane_home(p), is_attention(pane_status(p, now, None).state))).collect();
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                        project = show_chips(ui, &self.projects, &homes);
                    });
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 4.0).show(ui, |ui| {
                        self.flow.show_recent(ui, now);
                        let shown: Vec<&QueueRow> = rows.iter().filter(|r| !hidden.contains(&r.pane_id)).collect();
                        if shown.is_empty() {
                            ui.add_space(14.0);
                            ui.label(RichText::new("No agents here yet. Press Ctrl+N to start a task.").size(12.5).color(INK3));
                        }
                        for (title, states) in GROUPS {
                            let mine: Vec<&&QueueRow> = shown.iter().filter(|r| states.contains(&r.status.state)).collect();
                            if mine.is_empty() {
                                continue;
                            }
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                ui.label(RichText::new(title).size(12.0).color(INK3));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(10.0);
                                    ui.label(RichText::new(mine.len().to_string()).font(FontId::monospace(12.0)).color(INK3));
                                });
                            });
                            for r in mine {
                                let Some(p) = self.panes.iter().find(|p| p.id == r.pane_id) else { continue };
                                let files = self.git_changed.get(&crate::states::pane_cwd(p)).map(Vec::len);
                                if row(ui, p, r, files, self.stage.as_deref() == Some(p.id.as_str()), now) {
                                    pick = Some(p.id.clone());
                                }
                            }
                        }
                        ui.add_space(8.0);
                        self.extras.plan.show(ui);
                    });
                });
            });
        });
        if let Some(a) = project {
            self.apply_project(a);
        }
        if let Some(id) = pick {
            self.stage = Some(id.clone());
            self.focus_pane = Some(id);
            self.board.open = false;
        }
    }

    /// Sessions, Flow or Review in the left panel, with the way back.
    fn side_mode(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("\u{2190} Queue").size(12.5).color(INK3)).frame(false)).clicked() {
                self.side_tab = SideTab::Queue;
            }
        });
        ui.add_space(4.0);
        match self.side_tab {
            SideTab::Review => self.review.show(ui, &mut self.panes),
            SideTab::Sessions => {
                if let Some(req) = self.sessions.show(ui) {
                    self.spawn_resume(req);
                }
                if let Some(req) = self.sessions.show_worktree(ui) {
                    if let Err(e) = self.spawn_worktree(req) {
                        self.sessions.wt_error = Some(e);
                    }
                }
            }
            SideTab::Flow => self.flow.show(ui),
            SideTab::Queue | SideTab::History => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_how_long_since() {
        assert_eq!(since(1_000, 30_000), "now");
        assert_eq!(since(0, 4 * 60_000), "4m");
        assert_eq!(since(0, 2 * 3_600_000), "2h");
        assert_eq!(since(0, 3 * 86_400_000), "3d");
        assert_eq!(fit("abcdef", 3), "abc\u{2026}");
    }
}
