//! The left panel: every agent, grouped by project. A project's header folds
//! it and carries its count, how many need you, what it cost, and its
//! memory; the rows and headers are drawn in agent_row.rs, an agent's menu
//! is row_menu.rs. Picking a row puts the agent on the stage. The plan meter sits at the
//! foot. Sessions, Flow and Review open in the same panel, with a way back.

use std::collections::HashSet;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, Vec2};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::commands::SideTab;
use crate::agent_row::{project_head, row};
use crate::projects::{pane_home, project_name};
use crate::queue_ui::{is_attention, QueueRow};
use crate::style::{GUTTER, PANEL};
use crate::term_pane::Pane;
use crate::theme::{ADD, AMBER, DEL, FOCUS, INK3};

pub const QUEUE_W: f32 = 320.0;

/// "35s", "4m", "2h", "3d" since the agent last moved.
pub fn since(ms: u64, now: u64) -> String {
    let s = now.saturating_sub(ms) / 1000;
    match s {
        0..60 => format!("{s}s"),
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
        TaskState::Working => ADD,
        _ => INK3,
    }
}

/// The state as a row says it.
pub(crate) fn state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Needs => "Needs you",
        TaskState::Blocked => "Blocked",
        TaskState::Review => "Ready to review",
        TaskState::Working => "Working",
        TaskState::Idle => "Idle",
        TaskState::Stopped => "Stopped",
        TaskState::Merged => "Merged",
    }
}

/// A row's line: the state, then what the agent is on when that says more
/// ("Idle · Done"), just the state when it would repeat it ("Working").
pub fn row_line(name: &str, state: TaskState, line: &str) -> (String, String) {
    let label = state_label(state).to_string();
    let rest = line.strip_prefix(name).unwrap_or(line).trim_start();
    let rest = rest.strip_prefix("is ").unwrap_or(rest).trim_end_matches('.');
    // "Working · overlaps Cy" after the label "Working": say it once.
    let head = format!("{label} \u{00B7} ");
    let rest = match rest.get(..head.len()) {
        Some(h) if h.eq_ignore_ascii_case(&head) => &rest[head.len()..],
        _ => rest,
    };
    let rest: String = rest.chars().next().map(|c| c.to_uppercase().collect::<String>() + &rest[c.len_utf8()..]).unwrap_or_default();
    if rest.is_empty() || rest.eq_ignore_ascii_case(&label) {
        (label, String::new())
    } else {
        (label, rest)
    }
}

/// A floating panel filling `ui`: the design's --panel at --r-xl.
pub fn panel<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    panel_ring(ui, Color32::from_white_alpha(18), add)
}

/// A floating panel with its edge in `ring`.
pub fn panel_ring<R>(ui: &mut egui::Ui, ring: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(PANEL)
        .corner_radius(18.0)
        .stroke(Stroke::new(1.0, ring))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            add(ui)
        })
        .inner
}

/// What an agent works on, in a line: its CLI's title for the conversation,
/// else the first line of the job it was given.
pub fn headline(p: &Pane, brief: Option<&crate::brief::Brief>) -> Option<String> {
    brief.and_then(|b| b.title.clone()).or_else(|| p.task.clone()).filter(|t| !t.trim().is_empty())
}

impl ShellApp {
    /// The left panel; a picked agent goes on the stage.
    pub(crate) fn inbox_panel(&mut self, ui: &mut egui::Ui, rows: &[QueueRow], hidden: &HashSet<String>) {
        // Folded: the faces alone (Sessions and the rest still open full).
        if self.extras.side_mini && self.side_tab == SideTab::Queue {
            self.inbox_rail(ui, rows, hidden);
            return;
        }
        // The project tab on show decides what is listed: All groups every
        // project, a project's tab only its own agents.
        let mut act = None;
        let mut memory = None;
        let mut close = None;
        let mut clear = Vec::new();
        let now = now_ms();
        let frame = egui::Frame::NONE.inner_margin(egui::Margin { left: GUTTER as i8, right: 0, top: 0, bottom: 0 });
        egui::Panel::left("inbox").exact_size(QUEUE_W + GUTTER).resizable(false).show_separator_line(false).frame(frame).show(ui, |ui| {
            panel(ui, |ui| {
                if self.side_tab == SideTab::History {
                    // History lives on the stage now.
                    self.extras.stage.history = true;
                    self.side_tab = SideTab::Queue;
                }
                if self.side_tab != SideTab::Queue {
                    egui::Frame::NONE.inner_margin(egui::Margin::same(12)).show(ui, |ui| self.side_mode(ui));
                    return;
                }
                let full = ui.available_rect_before_wrap();
                let plan_h = self.extras.stage.plan_h;
                let list = Rect::from_min_max(full.min, Pos2::new(full.right(), full.bottom() - plan_h));
                ui.scope_builder(egui::UiBuilder::new().max_rect(list.shrink2(Vec2::new(8.0, 0.0))), |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.add_space(2.0);
                        self.flow.show_recent(ui, now);
                        let shown: Vec<&QueueRow> = rows.iter().filter(|r| !hidden.contains(&r.pane_id)).collect();
                        // The tabs name the projects: a project's tab lists its
                        // agents with no header, All heads only the groups
                        // that have agents.
                        let one = self.projects.filter.is_some();
                        let mut order = Vec::new();
                        // Stopped or merged: what a folded project puts away.
                        let away = |id: &str| self.panes.iter().any(|p| p.id == id && (p.exit_code.is_some() || p.merged));
                        for (key, name) in self.groups(&shown) {
                            let mine: Vec<&&QueueRow> = shown.iter().filter(|r| self.group_of(&r.pane_id).as_deref() == Some(key.as_str())).collect();
                            if one {
                                for r in mine {
                                    let Some(p) = self.panes.iter().find(|p| p.id == r.pane_id) else { continue };
                                    order.push(p.id.clone());
                                    let on = self.stage.as_deref() == Some(r.pane_id.as_str());
                                    if let Some(a) = row(ui, p, r, on, now, self.extras.briefs.get(&p.id)) {
                                        act = Some(a);
                                    }
                                }
                                continue;
                            }
                            if mine.is_empty() {
                                continue;
                            }
                            let open = !self.extras.folded.contains(&key);
                            let needs = mine.iter().filter(|r| is_attention(r.status.state)).count();
                            let cost: f64 = mine.iter().filter_map(|r| self.panes.iter().find(|p| p.id == r.pane_id)).filter_map(|p| p.usage.as_ref().and_then(|u| u.usd)).sum();
                            // Folded, a project keeps its agents still going in
                            // view and puts the stopped ones away.
                            let put_away = mine.iter().filter(|r| away(&r.pane_id)).count();
                            let (head, mem) = project_head(ui, &name, open, mine.len(), needs, (cost > 0.0).then_some(cost));
                            let tip = match (open, put_away) {
                                (true, _) => "Fold the stopped agents away".to_string(),
                                (false, 0) => "Unfold".to_string(),
                                (false, n) => format!("Show {n} stopped"),
                            };
                            let head = head.on_hover_text(tip);
                            if mem {
                                memory = Some(key.clone());
                            }
                            head.context_menu(|ui| {
                                crate::row_menu::tight(ui);
                                if ui.button("Project memory").clicked() {
                                    memory = Some(key.clone());
                                }
                                if ui.button("Close project").on_hover_text("Its agents stop and leave the list").clicked() {
                                    close = Some(key.clone());
                                }
                            });
                            if head.clicked() {
                                if open {
                                    self.extras.folded.insert(key.clone());
                                } else {
                                    self.extras.folded.remove(&key);
                                }
                            }
                            ui.add_space(2.0);
                            for r in mine.into_iter().filter(|r| open || !away(&r.pane_id)) {
                                let Some(p) = self.panes.iter().find(|p| p.id == r.pane_id) else { continue };
                                order.push(p.id.clone());
                                let on = self.stage.as_deref() == Some(r.pane_id.as_str());
                                if let Some(a) = row(ui, p, r, on, now, self.extras.briefs.get(&p.id)) {
                                    act = Some(a);
                                }
                            }
                        }
                        if shown.is_empty() {
                            ui.add_space(14.0);
                            ui.label(RichText::new("No agents here yet. New agent starts one.").size(12.5).color(INK3));
                        }
                        // Merged agents are done: one button takes them all off.
                        let merged: Vec<String> = shown.iter().filter(|r| self.panes.iter().any(|p| p.id == r.pane_id && p.merged)).map(|r| r.pane_id.clone()).collect();
                        if !merged.is_empty() {
                            ui.add_space(8.0);
                            let b = egui::Button::new(RichText::new(format!("Clear {} merged", merged.len())).size(12.5).color(INK3)).frame(false);
                            if ui.add(b).on_hover_text("Their work is in and their worktrees are gone").clicked() {
                                clear = merged;
                            }
                        }
                        self.extras.list_order = order;
                        ui.add_space(8.0);
                    });
                });
                // The plan meter, kept at the foot, with a line over it.
                let foot = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - plan_h), full.max);
                // A child that takes no room in the panel: measured, then kept
                // at the foot next frame.
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(foot.min, Pos2::new(foot.right(), full.bottom() + 400.0))));
                if plan_h > 1.0 {
                    child.painter().hline(foot.x_range(), foot.top(), Stroke::new(1.0, Color32::from_white_alpha(18)));
                }
                egui::Frame::NONE.inner_margin(egui::Margin { left: 14, right: 14, top: 4, bottom: 14 }).show(&mut child, |ui| self.extras.plan.show(ui));
                let h = child.min_rect().height();
                let h = if h < 24.0 { 0.0 } else { h };
                if (h - plan_h).abs() > 0.5 {
                    self.extras.stage.plan_h = h;
                    ui.ctx().request_repaint();
                }
            });
        });
        if let Some(dir) = memory {
            self.extras.memory.open_on(&dir);
        }
        if let Some(dir) = close {
            self.apply_project(crate::projects_ui::ProjectAction::Close(dir));
            ui.ctx().request_repaint();
        }
        if !clear.is_empty() {
            self.remove_panes(&clear);
            ui.ctx().request_repaint();
        }
        if let Some(a) = act {
            self.row_act(a, ui.ctx());
        }
    }

    /// The project a pane is grouped under: its open project, else its home.
    pub(crate) fn group_of(&self, pane_id: &str) -> Option<String> {
        let p = self.panes.iter().find(|p| p.id == pane_id)?;
        let home = pane_home(p);
        Some(self.projects.owner(&home).map(str::to_string).unwrap_or(home))
    }

    /// The groups in order: every open project, then any other home an agent
    /// runs in. (key, name).
    fn groups(&self, rows: &[&QueueRow]) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self.projects.dirs.iter().map(|d| (d.clone(), project_name(d))).collect();
        for r in rows {
            if let Some(k) = self.group_of(&r.pane_id) {
                if !v.iter().any(|(x, _)| *x == k) {
                    let n = project_name(&k);
                    v.push((k, n));
                }
            }
        }
        // A project's tab lists that project alone.
        if let Some(f) = &self.projects.filter {
            v.retain(|(k, _)| k.eq_ignore_ascii_case(f));
        }
        v
    }

    /// Sessions, Flow or Review in the left panel, with the way back.
    fn side_mode(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("\u{2190} Agents").size(12.5).color(INK3)).frame(false)).clicked() {
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
        assert_eq!(since(1_000, 30_000), "29s");
        assert_eq!(since(0, 4 * 60_000), "4m");
        assert_eq!(since(0, 2 * 3_600_000), "2h");
        assert_eq!(since(0, 3 * 86_400_000), "3d");
    }

    #[test]
    fn a_row_line_says_the_state_then_what_more_there_is() {
        assert_eq!(row_line("Ana", TaskState::Working, "Ana is working"), ("Working".into(), String::new()));
        assert_eq!(row_line("Bob", TaskState::Idle, "Bob done."), ("Idle".into(), "Done".into()));
        assert_eq!(row_line("Cy", TaskState::Working, "Cy is working \u{00B7} overlaps Ana in a.ts"), ("Working".into(), "Overlaps Ana in a.ts".into()));
    }
}
