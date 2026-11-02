//! The stage: one agent, large. A header with its face, its CLI and where it
//! works, its name (or what it is on), its state, More, Rename and Stop, and
//! the tabs Commands, Model, Changes, History; then its live terminal, where
//! its questions are answered.


use agent_core::lock;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Vec2};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::grid::GridOut;
use crate::icons::Icon;
use crate::inbox::panel_ring;
use crate::projects::{pane_home, project_name};
use crate::states::pane_status;
use crate::sessions_ui::Cli;
use crate::style::{icon_btn, medium, primary, tint, GUTTER};
use crate::term_pane::Pane;
use crate::theme::{ADD, AMBER, DIM, FOCUS, INK3, TEXT};

#[derive(Default)]
pub struct StageState {
    pub history: bool,
    /// The title being typed after the pencil, while it is.
    pub renaming: Option<String>,
    /// The plan meter's height at the foot of the left panel, last frame.
    pub plan_h: f32,
}

/// The state pill (.spill): a dot and the word on its state's tint; a
/// needs-you dot is a small diamond.
fn pill(ui: &mut egui::Ui, state: TaskState, stopped: bool) {
    let (c, word) = match state {
        _ if stopped => (DIM, "Stopped"),
        TaskState::Needs => (AMBER, "Needs you"),
        TaskState::Blocked => (AMBER, "Blocked"),
        TaskState::Review => (FOCUS, "Ready to review"),
        TaskState::Working => (ADD, "Working"),
        TaskState::Merged => (DIM, "Merged"),
        _ => (DIM, "Idle"),
    };
    let fill = if matches!(state, TaskState::Working | TaskState::Review | TaskState::Needs | TaskState::Blocked) && !stopped { tint(c) } else { Color32::from_white_alpha(15) };
    let g = ui.painter().layout_no_wrap(word.into(), crate::style::w500(12.5), c);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0 + 8.0 + 7.0 + g.size().x + 12.0, 28.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 14.0, fill);
    let dot = Pos2::new(rect.left() + 16.0, rect.center().y);
    if matches!(state, TaskState::Needs | TaskState::Blocked) && !stopped {
        let d = 4.2;
        p.add(egui::Shape::convex_polygon(vec![dot + Vec2::new(0.0, -d), dot + Vec2::new(d, 0.0), dot + Vec2::new(0.0, d), dot + Vec2::new(-d, 0.0)], c, egui::Stroke::NONE));
    } else {
        p.circle_filled(dot, 4.0, c);
    }
    p.galley(Pos2::new(dot.x + 4.0 + 7.0, rect.center().y - g.size().y / 2.0), g, c);
}

/// The tab group (.stabs): quiet pills on a white 5% track; true per click.
fn tabs(ui: &mut egui::Ui, items: &[(&str, bool)]) -> Option<usize> {
    let mut hit = None;
    egui::Frame::new().fill(Color32::from_white_alpha(13)).corner_radius(999.0).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, (label, on)) in items.iter().enumerate() {
            let g = ui.painter().layout_no_wrap(label.to_string(), crate::style::w500(12.5), DIM);
            let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 26.0), egui::Sense::click());
            let fill = if *on { Color32::from_white_alpha(31) } else if r.hovered() { Color32::from_white_alpha(15) } else { Color32::TRANSPARENT };
            ui.painter().rect_filled(rect, 13.0, fill);
            let ink = if *on || r.hovered() { TEXT } else { DIM };
            ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
            if r.clicked() {
                hit = Some(i);
            }
        }
    });
    hit
}

/// What the Model tab types into a CLI, when it has one. Ctrl+U first: a
/// half-typed line (the "/" Commands left) would turn it into "//model".
fn model_command(cli: Option<Cli>) -> Option<&'static str> {
    match cli? {
        Cli::Claude | Cli::Codex => Some("\x15/model\r"),
        Cli::Opencode => Some("\x15/models\r"),
        Cli::Muse => None,
    }
}

impl ShellApp {
    /// The Queue view's stage: the agent picked in the queue, else the one
    /// that needs you most.
    pub(crate) fn stage_central(&mut self, ui: &mut egui::Ui, order: &[String]) -> GridOut {
        let mut out = GridOut::default();
        if self.stage.as_ref().is_none_or(|s| !order.contains(s)) {
            self.stage = order.first().cloned();
        }
        let ring = match self.stage.as_ref().and_then(|id| self.panes.iter().find(|p| &p.id == id)) {
            Some(p) if p.exit_code.is_none() => match pane_status(p, now_ms(), None).state {
                TaskState::Working => ADD.gamma_multiply(0.35),
                TaskState::Needs | TaskState::Blocked => AMBER.gamma_multiply(0.45),
                _ => Color32::from_white_alpha(18),
            },
            _ => Color32::from_white_alpha(18),
        };
        let frame = egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: GUTTER as i8, top: 0, bottom: 0 });
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            panel_ring(ui, ring, |ui| {
                let Some(i) = self.stage.as_ref().and_then(|id| self.panes.iter().position(|p| &p.id == id)) else {
                    ui.add_space(ui.available_height() * 0.4);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No agent selected").font(medium(16.0)).color(TEXT));
                        ui.label(RichText::new("Pick one on the left, or press Ctrl+N to start an agent.").size(13.0).color(INK3));
                    });
                    return;
                };
                self.stage_head(ui, i, &mut out);
                if self.extras.stage.history {
                    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 24, top: 6, bottom: 16 }).show(ui, |ui| {
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
        let changed = self.changed_count(p);
        let state = if p.merged { TaskState::Merged } else { pane_status(p, now_ms(), changed).state };
        let (id, exited, cli, name) = (p.id.clone(), p.exit_code.is_some(), p.cli, p.name.clone());
        let home = pane_home(p);
        let proj = self.projects.owner(&home).map(project_name).unwrap_or_else(|| project_name(&home));
        let place = match &p.worktree {
            Some(w) => format!("{proj} / {}", w.branch),
            None => proj,
        };
        let cost = p.usage.as_ref().and_then(|u| u.usd).map(supervise::pricing::money);
        let task = p.task.clone();
        let custom = p.custom.clone();
        let cli_word = match (cli, &custom) {
            (Some(c), _) => c.program().to_string(),
            (None, Some(cmd)) => crate::custom::program_name(cmd),
            _ => "shell".to_string(),
        };
        let cli_word = cli_word.as_str();
        let mut act: Option<&str> = None;
        egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 20, top: 20, bottom: 8 }).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // The face, ringed in the state's color while it works or asks.
                let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
                let face = Rect::from_min_size(r.min + Vec2::new(0.0, 2.0), Vec2::splat(36.0));
                let ring = match state {
                    _ if exited => None,
                    TaskState::Working => Some(ADD.gamma_multiply(0.65)),
                    TaskState::Needs | TaskState::Blocked => Some(AMBER.gamma_multiply(0.78)),
                    _ => None,
                };
                if let Some(c) = ring {
                    ui.painter().rect_stroke(face.expand(3.0), 12.0, egui::Stroke::new(2.0, c), egui::StrokeKind::Middle);
                }
                crate::face::avatar(ui.painter(), face, cli, custom.as_deref(), &name, 10.0, exited);
                ui.add_space(4.0);
                let right_w = 330.0_f32.min(ui.available_width() * 0.45);
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width() - right_w, 76.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let mut meta = egui::text::LayoutJob::default();
                    let fmt = |f: FontId, c: Color32| egui::TextFormat::simple(f, c);
                    if task.is_some() {
                        meta.append(&name, 0.0, fmt(medium(13.0), DIM));
                        meta.append(&format!(" \u{00B7} {cli_word}"), 0.0, fmt(FontId::proportional(12.5), DIM));
                    } else {
                        meta.append(cli_word, 0.0, fmt(FontId::proportional(12.5), DIM));
                    }
                    meta.append(&format!("   {place}"), 0.0, fmt(FontId::monospace(12.5), INK3));
                    if let Some(c) = &cost {
                        meta.append(&format!("   {c}"), 0.0, fmt(FontId::monospace(12.5), INK3));
                    }
                    ui.add(egui::Label::new(meta).truncate());
                    let title = task.clone().unwrap_or_else(|| name.clone());
                    match self.extras.stage.renaming.as_mut() {
                        Some(text) => {
                            let r = ui.add(egui::TextEdit::singleline(text).font(medium(20.0)).desired_width(ui.available_width()).hint_text("What this agent is on"));
                            r.request_focus();
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                act = Some("rename-save");
                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                act = Some("rename-cancel");
                            }
                        }
                        None => {
                            ui.add(egui::Label::new(RichText::new(title).font(medium(22.0)).extra_letter_spacing(-0.44).color(TEXT)).truncate());
                        }
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if exited && primary(ui, "Resume", None, 32.0).on_hover_text("Start it again in its own session").clicked() {
                            act = Some("restart");
                        }
                        let mut items: Vec<(&str, bool)> = Vec::new();
                        if !exited {
                            items.push(("Commands", false));
                            if model_command(cli).is_some() {
                                items.push(("Model", false));
                            }
                        } else if cli.is_some() {
                            items.push(("Hand over", false));
                        }
                        items.push(("Changes", self.changes.open));
                        items.push(("History", self.extras.stage.history));
                        if let Some(k) = tabs(ui, &items) {
                            act = Some(match items[k].0 {
                                "Commands" => "commands",
                                "Model" => "model",
                                "Hand over" => "handoff",
                                "Changes" => "changes",
                                _ => "history",
                            });
                        }
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if icon_btn(ui, Icon::Close, 30.0, false, true).on_hover_text(if exited { "Remove" } else { "Stop this agent" }).clicked() {
                        act = Some("kill");
                    }
                    if icon_btn(ui, Icon::Pencil, 30.0, self.extras.stage.renaming.is_some(), false).on_hover_text("Rename what it is on").clicked() {
                        act = Some("rename");
                    }
                    let more = icon_btn(ui, Icon::Dots, 30.0, false, false).on_hover_text("More");
                    egui::Popup::menu(&more).show(|ui| {
                        for (key, label) in [("handoff", "Hand over to another CLI"), ("record", "Record (start or stop)"), ("files", "Files on this branch"), ("memory", "Project memory"), ("restart", "Restart")] {
                            // A plain shell has no task to hand over.
                            if key == "handoff" && cli.is_none() {
                                continue;
                            }
                            if ui.button(label).clicked() {
                                act = Some(key);
                            }
                        }
                    });
                    ui.add_space(8.0);
                    pill(ui, state, exited);
                });
            });
        });
        match act {
            // A running agent stops and stays (Resume); a stopped one goes.
            Some("kill") if exited => out.kill.push(id.clone()),
            Some("kill") => out.stop.push(id.clone()),
            Some("restart") => out.restart = Some(i),
            // One tab at a time: the terminal, the history, or the changes.
            Some("commands") => {
                let _ = lock(&self.registry).write_input(&id, b"/");
                self.extras.stage.history = false;
                self.changes.open = false;
                self.focus_pane = Some(id);
            }
            Some("model") => {
                if let Some(m) = model_command(cli) {
                    let _ = lock(&self.registry).write_input(&id, m.as_bytes());
                }
                self.extras.stage.history = false;
                self.changes.open = false;
                self.focus_pane = Some(id);
            }
            Some("history") => {
                self.extras.stage.history = !self.extras.stage.history;
                self.changes.open = false;
            }
            Some("changes") => {
                self.extras.stage.history = false;
                if self.changes.open {
                    self.changes.open = false;
                } else {
                    self.run_command("changes");
                }
            }
            Some("rename") => {
                let now = self.panes[i].task.clone().unwrap_or_default();
                self.extras.stage.renaming = if self.extras.stage.renaming.is_some() { None } else { Some(now) };
            }
            Some("rename-save") => {
                let t = self.extras.stage.renaming.take().unwrap_or_default();
                self.panes[i].task = Some(t.trim().to_string()).filter(|t| !t.is_empty());
            }
            Some("rename-cancel") => self.extras.stage.renaming = None,
            Some(key) => self.run_command(key),
            None => {}
        }
    }

    fn stage_body(&mut self, ui: &mut egui::Ui, i: usize) {
        // The terminal takes the whole stage: a question is answered in it, as
        // the CLI asks it, with no form of ByteHive's own on top.
        let avail = ui.available_rect_before_wrap();
        let term = Rect::from_min_max(Pos2::new(avail.left() + 24.0, avail.top() + 4.0), Pos2::new(avail.right() - 24.0, avail.bottom() - 12.0));
        let cell = {
            crate::term_pane::cell_metrics(ui.painter(), &self.font)
        };
        let font = self.font.clone();
        let reg = std::sync::Arc::clone(&self.registry);
        self.panes[i].paint(ui, term, &mut lock(&reg), &font, cell);
        if !self.focused_once || self.focus_pane.as_deref() == Some(self.panes[i].id.as_str()) {
            self.focused_once = true;
            ui.memory_mut(|m| m.request_focus(ui.make_persistent_id(("term", &self.panes[i].id))));
            self.focus_pane = None;
        }
        self.panes[i].claim_focus(ui);
        ui.allocate_rect(avail, egui::Sense::hover());
    }
}
