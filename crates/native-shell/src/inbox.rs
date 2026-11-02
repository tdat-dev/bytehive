//! The left panel: every agent, grouped by project. A project's header folds
//! it and carries its count, how many need you, what it cost, and its
//! memory. An agent's row is its pixel face with a state dot, its name, how
//! long since it moved, and one line (its state, what it is on, its cost).
//! Picking a row puts the agent on the stage. The plan meter sits at the
//! foot. Sessions, Flow and Review open in the same panel, with a way back.

use std::collections::HashSet;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::commands::SideTab;
use crate::icons::{paint_in, Icon};
use crate::projects::{pane_home, project_name};
use crate::queue_ui::{is_attention, QueueRow};
use crate::style::{medium, GUTTER, PANEL, RAISE};
use crate::term_pane::Pane;
use crate::theme::{ADD, AMBER, DEL, DIM, FOCUS, INK3, TEXT};

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

/// What a row asks of the app.
enum RowAct {
    Pick(String),
    Restart(usize),
    Remove(String),
}

/// One agent row (.row); what it was asked to do.
fn row(ui: &mut egui::Ui, i: usize, p: &Pane, r: &QueueRow, on: bool, now: u64) -> Option<RowAct> {
    let state = r.status.state;
    let stopped = p.exit_code.is_some();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 56.0), Sense::click());
    let needs = is_attention(state);
    let painter = ui.painter().clone();
    let hot = resp.hovered() || resp.contains_pointer();
    let fill = match (needs, on, hot) {
        (true, true, _) => Some(AMBER.gamma_multiply(0.20)),
        (true, _, true) => Some(AMBER.gamma_multiply(0.17)),
        (true, ..) => Some(AMBER.gamma_multiply(0.12)),
        (_, true, _) => Some(RAISE),
        (_, _, true) => Some(Color32::from_white_alpha(8)),
        _ => None,
    };
    if let Some(f) = fill {
        painter.rect_filled(rect, 12.0, f);
    }
    if needs {
        painter.rect_stroke(rect, 12.0, Stroke::new(1.0, AMBER.gamma_multiply(0.3)), egui::StrokeKind::Inside);
    } else if on {
        painter.rect_stroke(rect, 12.0, Stroke::new(1.0, Color32::from_white_alpha(26)), egui::StrokeKind::Inside);
    }
    let inner = rect.shrink(10.0);
    let face = Rect::from_min_size(inner.min + Vec2::new(0.0, 2.0), Vec2::splat(24.0));
    crate::face::avatar(&painter, face, p.cli, &p.name, 7.0, stopped);
    // The state dot at the face's corner, ringed in the row's ground.
    let dot = face.right_bottom() + Vec2::new(-1.5, -1.5);
    let ground = if on { RAISE } else { PANEL };
    painter.circle_filled(dot, 6.5, ground);
    if stopped {
        painter.circle_stroke(dot, 3.75, Stroke::new(1.5, INK3));
    } else {
        let c = match state {
            TaskState::Needs | TaskState::Blocked => AMBER,
            TaskState::Review => FOCUS,
            TaskState::Working => ADD,
            _ => INK3,
        };
        painter.circle_filled(dot, if state == TaskState::Merged { 2.5 } else { 4.5 }, c);
    }
    let left = inner.left() + 34.0;
    let mut act = None;
    // Time at the right, or the ⋯ menu while the pointer is on the row.
    let more = Rect::from_min_size(Pos2::new(inner.right() - 24.0, inner.top() - 4.0), Vec2::splat(24.0));
    let more_r = ui.interact(more, ui.make_persistent_id(("row-more", &p.id)), Sense::click());
    if hot || more_r.hovered() || egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&more_r)) {
        if more_r.hovered() {
            painter.rect_filled(more, 12.0, Color32::from_white_alpha(15));
        }
        paint_in(&painter, more, 15.0, Icon::Dots, if more_r.hovered() { TEXT } else { INK3 }, 1.6);
    } else {
        painter.text(Pos2::new(inner.right(), inner.top() + 1.0), egui::Align2::RIGHT_TOP, since(p.last_output_ms, now), FontId::monospace(12.0), INK3);
    }
    egui::Popup::menu(&more_r).show(|ui| {
        if ui.button(if stopped { "Resume" } else { "Restart" }).clicked() {
            act = Some(RowAct::Restart(i));
        }
        if ui.button("Remove").clicked() {
            act = Some(RowAct::Remove(p.id.clone()));
        }
    });
    let name_c = if stopped { DIM } else { TEXT };
    let name_f = if stopped { FontId::proportional(13.5) } else { crate::style::w500(13.5) };
    let name = painter.layout(p.name.clone(), name_f, name_c, inner.right() - 34.0 - left);
    painter.galley(Pos2::new(left, inner.top()), name, name_c);
    let (label, rest) = row_line(&p.name, state, &r.line);
    let fmt = |c: Color32| egui::TextFormat::simple(FontId::proportional(12.5), c);
    let mut sub = egui::text::LayoutJob::default();
    sub.append(&label, 0.0, fmt(if stopped { INK3 } else { line_color(state) }));
    if !rest.is_empty() {
        sub.append(&format!(" \u{00B7} {rest}"), 0.0, fmt(INK3));
    }
    if let Some(usd) = p.usage.as_ref().and_then(|u| u.usd) {
        sub.append(&format!("  \u{00B7}  {}", supervise::pricing::money(usd)), 0.0, egui::TextFormat::simple(FontId::monospace(11.5), INK3));
    }
    let chip_w = if stopped { 76.0 } else { 0.0 };
    sub.wrap = egui::text::TextWrapping::truncate_at_width(inner.right() - left - chip_w);
    painter.galley(Pos2::new(left, inner.top() + 20.0), painter.layout_job(sub), INK3);
    if stopped {
        let chip = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - 68.0, rect.bottom() - 8.0 - 24.0), Vec2::new(68.0, 24.0));
        let cr = ui.interact(chip, ui.make_persistent_id(("row-resume", &p.id)), Sense::click());
        let (bg, ink) = if cr.hovered() { (TEXT, Color32::from_rgb(5, 5, 6)) } else { (Color32::from_white_alpha(26), TEXT) };
        painter.rect_filled(chip, 12.0, bg);
        painter.text(chip.center(), egui::Align2::CENTER_CENTER, "Resume", medium(12.0), ink);
        if cr.clicked() {
            act = Some(RowAct::Restart(i));
        }
    }
    if act.is_none() && resp.clicked() {
        act = Some(RowAct::Pick(p.id.clone()));
    }
    if let Some(t) = &p.task {
        resp.on_hover_text(t);
    }
    act
}

/// A project's header (.proj); (clicked to fold, memory clicked).
fn project_head(ui: &mut egui::Ui, name: &str, open: bool, count: usize, needs: usize, cost: Option<f64>) -> (egui::Response, bool) {
    ui.add_space(6.0);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w - 34.0, 32.0), Sense::click());
    let p = ui.painter().clone();
    if resp.hovered() {
        p.rect_filled(rect, 9.0, Color32::from_white_alpha(10));
    }
    let ink = if resp.hovered() { TEXT } else { DIM };
    let chev = Rect::from_center_size(Pos2::new(rect.left() + 12.0, rect.center().y), Vec2::splat(11.0));
    crate::icons::paint(&p, chev, Icon::Chevron { open }, INK3, 2.4);
    let mut x = rect.right() - 6.0;
    if let Some(usd) = cost {
        let g = p.layout_no_wrap(format!("\u{00B7} {}", supervise::pricing::money(usd)), FontId::monospace(11.5), INK3);
        x -= g.size().x;
        p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, INK3);
        x -= 6.0;
    }
    let g = p.layout_no_wrap(count.to_string(), FontId::monospace(12.0), INK3);
    x -= g.size().x;
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, INK3);
    x -= 8.0;
    if needs > 0 {
        let g = p.layout_no_wrap(needs.to_string(), FontId::new(11.0, egui::FontFamily::Name("monomedium".into())), Color32::from_rgb(26, 18, 4));
        let badge = Rect::from_min_size(Pos2::new(x - (g.size().x + 10.0).max(18.0), rect.center().y - 9.0), Vec2::new((g.size().x + 10.0).max(18.0), 18.0));
        p.rect_filled(badge, 9.0, AMBER);
        p.galley(badge.center() - g.size() / 2.0, g, Color32::BLACK);
        x = badge.left() - 8.0;
    }
    let title = p.layout(name.to_string(), medium(12.5), ink, (x - rect.left() - 26.0).max(20.0));
    let title = if title.rows.len() > 1 { p.layout_no_wrap(format!("{}\u{2026}", name.chars().take(24).collect::<String>()), medium(12.5), ink) } else { title };
    p.galley(Pos2::new(rect.left() + 24.0, rect.center().y - title.size().y / 2.0), title, ink);
    // The memory book at the far right, faint until hovered.
    let book = Rect::from_min_size(Pos2::new(rect.right() + 2.0, rect.top() + 1.0), Vec2::splat(30.0));
    let m = ui.interact(book, ui.make_persistent_id(("proj-mem", name)), Sense::click()).on_hover_text("Project memory: what every agent here knows");
    if m.hovered() {
        p.rect_filled(book, 9.0, Color32::from_white_alpha(15));
    }
    paint_in(&p, book, 15.0, Icon::Book, if m.hovered() { TEXT } else { INK3.gamma_multiply(0.75) }, 1.4);
    (resp, m.clicked())
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
                        for (key, name) in self.groups(&shown) {
                            let mine: Vec<&&QueueRow> = shown.iter().filter(|r| self.group_of(&r.pane_id).as_deref() == Some(key.as_str())).collect();
                            if one {
                                for r in mine {
                                    let Some(i) = self.panes.iter().position(|p| p.id == r.pane_id) else { continue };
                                    let on = self.stage.as_deref() == Some(r.pane_id.as_str());
                                    if let Some(a) = row(ui, i, &self.panes[i], r, on, now) {
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
                            let (head, mem) = project_head(ui, &name, open, mine.len(), needs, (cost > 0.0).then_some(cost));
                            if mem {
                                memory = Some(key.clone());
                            }
                            head.context_menu(|ui| {
                                if ui.button("Project memory").clicked() {
                                    memory = Some(key.clone());
                                }
                                if ui.button("Close project").clicked() {
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
                            if !open {
                                continue;
                            }
                            ui.add_space(2.0);
                            for r in mine {
                                let Some(i) = self.panes.iter().position(|p| p.id == r.pane_id) else { continue };
                                let on = self.stage.as_deref() == Some(r.pane_id.as_str());
                                if let Some(a) = row(ui, i, &self.panes[i], r, on, now) {
                                    act = Some(a);
                                }
                            }
                        }
                        if shown.is_empty() {
                            ui.add_space(14.0);
                            ui.label(RichText::new("No agents here yet. New agent starts one.").size(12.5).color(INK3));
                        }
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
        }
        match act {
            Some(RowAct::Pick(id)) => {
                self.stage = Some(id.clone());
                self.focus_pane = Some(id);
                self.board.open = false;
                self.extras.home = false;
            }
            Some(RowAct::Restart(i)) => self.restart_pane(i),
            Some(RowAct::Remove(id)) => {
                let gone = agent_core::lock(&self.registry).take(&id);
                drop(gone);
                self.panes.retain(|p| p.id != id);
            }
            None => {}
        }
    }

    /// The project a pane is grouped under: its open project, else its home.
    fn group_of(&self, pane_id: &str) -> Option<String> {
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
