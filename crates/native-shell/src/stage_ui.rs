//! The stage of the approved design: one agent, large. A header with its
//! avatar, who and where it is, what it costs, its task as the title, its
//! state, Terminal or History, Changes and Kill; then its live terminal, and
//! when it asks you something, the decision card under it (the options as
//! buttons, or an answer in your own words).

use std::collections::{HashMap, HashSet};

use agent_core::lock;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Vec2};
use supervise::ask::Ask;
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::grid::GridOut;
use crate::inbox::{avatar, panel};
use crate::projects::{pane_home, project_name};
use crate::queue_ui::{answer_bytes, free_answer_bytes};
use crate::states::{pane_cwd, pane_status};
use crate::style::{hbtn, medium, primary, tint, GUTTER, RAISE};
use crate::term_pane::Pane;
use crate::theme::{state_dot, state_word, AMBER, DIM, INK3, TEXT};

#[derive(Default)]
pub struct StageState {
    pub history: bool,
    /// A typed answer per agent.
    answers: HashMap<String, String>,
    /// Agents whose question card you hid (type in the terminal instead).
    hidden_ask: HashSet<String>,
}

/// The state pill (.pill): the word on its state's tint.
fn pill(ui: &mut egui::Ui, state: TaskState) {
    let c = state_dot(state);
    let word = match state {
        TaskState::Needs => "Needs you",
        TaskState::Review => "Ready to review",
        s => state_word(s),
    };
    let word: String = word.chars().next().map(|f| f.to_uppercase().collect::<String>() + &word[1..]).unwrap_or_default();
    egui::Frame::new().fill(tint(c)).corner_radius(999.0).inner_margin(egui::Margin::symmetric(11, 5)).show(ui, |ui| {
        ui.label(RichText::new(word).size(12.5).color(c));
    });
}

impl ShellApp {
    /// The Queue view's stage: the agent picked in the queue, else the one
    /// that needs you most.
    pub(crate) fn stage_central(&mut self, ui: &mut egui::Ui, order: &[String]) -> GridOut {
        let mut out = GridOut::default();
        if self.stage.as_ref().is_none_or(|s| !order.contains(s)) {
            self.stage = order.first().cloned();
        }
        let frame = egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: GUTTER as i8, top: 0, bottom: 0 });
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            panel(ui, |ui| {
                let Some(i) = self.stage.as_ref().and_then(|id| self.panes.iter().position(|p| &p.id == id)) else {
                    ui.add_space(ui.available_height() * 0.4);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No agent selected").font(medium(16.0)).color(TEXT));
                        ui.label(RichText::new("Pick one from the queue, or press Ctrl+N to start a task.").size(13.0).color(INK3));
                    });
                    return;
                };
                self.stage_head(ui, i, &mut out);
                if self.extras.stage.history {
                    egui::Frame::NONE.inner_margin(egui::Margin { left: 74, right: 24, top: 6, bottom: 16 }).show(ui, |ui| {
                        let projects = &self.projects;
                        let home = |p: &Pane| {
                            let h = pane_home(p);
                            projects.owner(&h).map(str::to_string).unwrap_or(h)
                        };
                        if let Some(a) = self.history.show(ui, &self.panes, self.stage.as_deref(), &self.prefs.schedules, home) {
                            self.extras.pending_hist = Some(a);
                        }
                    });
                } else {
                    self.stage_body(ui, i);
                }
            });
        });
        if let Some(a) = self.extras.pending_hist.take() {
            self.history_action(a);
        }
        out
    }

    fn stage_head(&mut self, ui: &mut egui::Ui, i: usize, out: &mut GridOut) {
        let p = &self.panes[i];
        let changed = self.git_changed.get(&pane_cwd(p)).map(|f| f.len() as u32);
        let state = if p.merged { TaskState::Merged } else { pane_status(p, now_ms(), changed).state };
        let (id, exited, cli, name) = (p.id.clone(), p.exit_code.is_some(), p.cli, p.name.clone());
        let home = pane_home(p);
        let proj = self.projects.owner(&home).map(project_name).unwrap_or_else(|| project_name(&home));
        let place = match &p.worktree {
            Some(w) => format!("   {proj} / {}", w.branch),
            None => format!("   {proj}"),
        };
        let cost = p.usage.as_ref().and_then(|u| u.usd).map(supervise::pricing::money);
        let title = p.task.clone().unwrap_or_else(|| p.name.clone());
        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 20, top: 20, bottom: 6 }).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                avatar(ui, r.translate(Vec2::new(0.0, 2.0)), cli, &name, 10.0);
                ui.add_space(4.0);
                let right_w = 380.0_f32.min(ui.available_width() * 0.5);
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width() - right_w, 64.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    let mut meta = egui::text::LayoutJob::default();
                    let fmt = |c: Color32| egui::TextFormat::simple(FontId::proportional(13.0), c);
                    meta.append(&name, 0.0, egui::TextFormat::simple(medium(13.0), DIM));
                    meta.append(&format!("   {}", cli.map_or("Shell", |c| c.name())), 0.0, fmt(INK3));
                    meta.append(&place, 0.0, egui::TextFormat::simple(FontId::monospace(12.5), INK3));
                    if let Some(c) = &cost {
                        meta.append(&format!("   {c}"), 0.0, egui::TextFormat::simple(FontId::monospace(12.5), INK3));
                    }
                    ui.add(egui::Label::new(meta).truncate());
                    ui.add(egui::Label::new(RichText::new(title).font(medium(26.0)).color(TEXT)).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if hbtn(ui, if exited { "Restart" } else { "Kill" }, false).on_hover_text(if exited { "Start it again" } else { "Stop this agent" }).clicked() {
                        if exited {
                            out.restart = Some(i);
                        } else {
                            out.kill.push(id.clone());
                        }
                    }
                    if hbtn(ui, "Changes", self.changes.open).on_hover_text("Its diff, and Merge, Send back or Discard").clicked() {
                        if self.changes.open {
                            self.changes.open = false;
                        } else {
                            self.run_command("changes");
                        }
                    }
                    egui::Frame::new().fill(Color32::from_white_alpha(10)).corner_radius(999.0).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (label, hist) in [("History", true), ("Terminal", false)] {
                            if hbtn(ui, label, self.extras.stage.history == hist).clicked() {
                                self.extras.stage.history = hist;
                            }
                        }
                    });
                    pill(ui, state);
                });
            });
        });
    }

    fn stage_body(&mut self, ui: &mut egui::Ui, i: usize) {
        let ask = pane_status(&self.panes[i], now_ms(), None).ask.filter(|_| !self.extras.stage.hidden_ask.contains(&self.panes[i].id));
        let card_h = ask.as_ref().map_or(0.0, |a| 150.0 + 36.0 * a.options.len().min(4) as f32);
        let avail = ui.available_rect_before_wrap();
        let term = Rect::from_min_max(Pos2::new(avail.left() + 22.0, avail.top() + 8.0), Pos2::new(avail.right() - 18.0, avail.bottom() - 16.0 - card_h));
        ui.painter().rect_filled(term, 12.0, Color32::from_rgb(10, 10, 12));
        let cell = {
            let g = ui.painter().layout_no_wrap("MMMMMMMMMM".to_string(), self.font.clone(), TEXT);
            (g.rect.width() / 10.0, g.rect.height())
        };
        let font = self.font.clone();
        let reg = std::sync::Arc::clone(&self.registry);
        self.panes[i].paint(ui, term.shrink(6.0), &mut lock(&reg), &font, cell);
        if !self.focused_once || self.focus_pane.as_deref() == Some(self.panes[i].id.as_str()) {
            self.focused_once = true;
            ui.memory_mut(|m| m.request_focus(ui.make_persistent_id(("term", &self.panes[i].id))));
            self.focus_pane = None;
        }
        if let Some(a) = ask {
            let card = Rect::from_min_max(Pos2::new(term.left(), term.bottom() + 12.0), Pos2::new(term.right(), avail.bottom() - 12.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| self.decision(ui, i, &a));
        }
        ui.allocate_rect(avail, egui::Sense::hover());
    }

    /// The question an agent waits on: its options, or your own words.
    fn decision(&mut self, ui: &mut egui::Ui, i: usize, a: &Ask) {
        let id = self.panes[i].id.clone();
        let name = self.panes[i].name.clone();
        let mut send: Option<Vec<u8>> = None;
        egui::Frame::new().fill(RAISE).corner_radius(16.0).stroke(egui::Stroke::new(1.0, tint(AMBER).gamma_multiply(2.2))).inner_margin(egui::Margin { left: 18, right: 18, top: 14, bottom: 14 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("\u{25C6}  {name} asks you")).size(12.5).color(AMBER));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if hbtn(ui, "Hide \u{00B7} type in terminal", false).clicked() {
                        self.extras.stage.hidden_ask.insert(id.clone());
                    }
                });
            });
            ui.label(RichText::new(&a.prompt).font(medium(16.0)).color(TEXT));
            if let Some(d) = &a.detail {
                egui::Frame::new().fill(Color32::from_rgb(10, 10, 12)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(14, 9)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(d).font(FontId::monospace(13.0)).color(TEXT));
                });
            }
            ui.horizontal_wrapped(|ui| {
                for o in &a.options {
                    let label = if o.always { format!("{}  (always)", o.label) } else { o.label.clone() };
                    let r = if o.n == 1 { primary(ui, &label, Some(&o.n.to_string()), 34.0) } else { hbtn(ui, &format!("{label}  {}", o.n), false) };
                    if r.clicked() {
                        send = answer_bytes(a, o.n);
                    }
                }
            });
            let text = self.extras.stage.answers.entry(id.clone()).or_default();
            let r = ui.add(egui::TextEdit::singleline(text).hint_text(format!("Or tell {name} what to do instead\u{2026}")).desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !text.trim().is_empty() {
                send = Some(free_answer_bytes(Some(a), text.trim()));
                text.clear();
            }
        });
        if let Some(bytes) = send {
            let _ = lock(&self.registry).write_input(&id, &bytes);
        }
    }
}
