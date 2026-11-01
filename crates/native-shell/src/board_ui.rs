//! The Board view, in place of the terminals (top bar "Board"). Two halves:
//! Agents, the approved design's derived board, where columns follow what
//! each agent is actually doing so there is nothing to drag and a click opens
//! the agent; and Plan, the project's `.bytehive/board.json`, where agents
//! propose cards and you approve them into "To do", move them on, and tick
//! their checklists. The plan file is re-read every two seconds, since
//! agents write to it while you look.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};
use supervise::task_state::TaskState;

use crate::plan_board::{default_board, doing_list, edit, load, move_card, save, set_check, todo_list, view, ListView};
use crate::plan_edit::Change;
use crate::theme::{state_dot, DEL, DIM, INK3, PRIMARY, PRIMARY_INK, R3, SURFACE, TEXT};

const RELOAD: Duration = Duration::from_secs(2);
const COL_W: f32 = 250.0;

/// One agent as a Board card.
pub struct AgentCard {
    pub pane_id: String,
    pub name: String,
    pub cli: String,
    pub title: String,
    pub where_: String,
    pub state: TaskState,
    /// "$0.42", or empty when not known.
    pub cost: String,
}

/// The design's five columns, in order.
const COLUMNS: [(&str, &[TaskState]); 5] = [
    ("Free", &[TaskState::Idle, TaskState::Stopped]),
    ("Working", &[TaskState::Working]),
    ("Needs you", &[TaskState::Needs, TaskState::Blocked]),
    ("Ready to review", &[TaskState::Review]),
    ("Merged", &[TaskState::Merged]),
];

#[derive(PartialEq, Clone, Copy, Default)]
enum Half {
    #[default]
    Agents,
    Plan,
}

#[derive(Default)]
pub struct BoardView {
    pub open: bool,
    half: Half,
    plan: Option<(PathBuf, serde_json::Value)>,
    loaded: Option<Instant>,
    expanded: Option<String>,
    error: Option<String>,
    edit: crate::plan_edit::PlanEdit,
    /// A card given to an agent: (pane id, task); the app types it in.
    pub give: Option<(String, String)>,
}

impl BoardView {
    /// Open on the plan half (Ctrl+K "Plan board").
    pub fn open_plan(&mut self) {
        self.open = true;
        self.half = Half::Plan;
    }

    fn reload(&mut self, path: &Path) {
        let stale = self.loaded.is_none_or(|t| t.elapsed() >= RELOAD) || self.plan.as_ref().is_none_or(|(p, _)| p != path);
        if stale {
            self.plan = load(path).map(|v| (path.to_path_buf(), v));
            self.loaded = Some(Instant::now());
        }
    }

    /// Paint the board; a clicked agent card hands back its pane id.
    pub fn show(&mut self, ui: &mut egui::Ui, agents: &[AgentCard], plan_path: Option<&Path>, project: &str) -> Option<String> {
        let mut open = None;
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            for (h, label) in [(Half::Agents, "Agents"), (Half::Plan, "Plan")] {
                let on = self.half == h;
                let b = egui::Button::new(RichText::new(label).size(13.0).color(if on { TEXT } else { INK3 }))
                    .fill(if on { Color32::from_white_alpha(20) } else { Color32::TRANSPARENT })
                    .corner_radius(999.0);
                if ui.add(b).clicked() {
                    self.half = h;
                }
            }
            ui.add_space(12.0);
            let note = match self.half {
                Half::Agents => "Columns follow what each agent is actually doing, so there is nothing to drag. Click a card to open it.".to_string(),
                Half::Plan => format!("{project}: cards agents proposed and the plan they work from. Approve a card to put it in To do."),
            };
            ui.label(RichText::new(note).size(13.0).color(INK3));
        });
        ui.add_space(10.0);
        match self.half {
            Half::Agents => open = agent_columns(ui, agents),
            Half::Plan => {
                ui.ctx().request_repaint_after(RELOAD);
                match plan_path {
                    Some(p) => {
                        self.reload(p);
                        self.plan_columns(ui, agents, p);
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(RichText::new("Open a project to see its plan board.").color(INK3));
                        });
                    }
                }
            }
        }
        open
    }

    fn plan_columns(&mut self, ui: &mut egui::Ui, agents: &[AgentCard], at: &Path) {
        if let Some(e) = &self.error {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new(e).size(12.5).color(DEL));
            });
        }
        let Some((path, board)) = self.plan.clone() else {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new("No plan board here yet. Agents start one with card_add over MCP, or start it here.").color(INK3));
                if ui.button("Start a plan board").clicked() {
                    let made = at.parent().map(std::fs::create_dir_all).transpose().map_err(|e| e.to_string()).and_then(|_| save(at, &default_board()));
                    self.error = made.err();
                    self.loaded = None;
                }
            });
            return;
        };
        let lists = view(&board);
        let todo = todo_list(&lists).map(|l| l.id.clone());
        let doing = doing_list(&lists).map(|l| l.id.clone());
        let mut change: Option<Change> = None;
        egui::ScrollArea::horizontal().id_salt("plan-cols").show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(16.0);
                for list in &lists {
                    column(ui, &list.title, list.cards.len(), |ui| {
                        for card in &list.cards {
                            if let Some(c) = self.plan_card(ui, card, list, &lists, (todo.as_deref(), doing.as_deref()), agents) {
                                change = Some(c);
                            }
                        }
                        if let Some(c) = self.edit.add_box(ui, &list.id) {
                            change = Some(c);
                        }
                    });
                }
            });
        });
        if let Some(c) = change {
            self.error = edit(&path, c).err();
            self.loaded = None;
        }
    }

    /// One plan card; an action hands back the change to write.
    fn plan_card(&mut self, ui: &mut egui::Ui, card: &crate::plan_board::CardView, list: &ListView, lists: &[ListView], (todo, doing): (Option<&str>, Option<&str>), agents: &[AgentCard]) -> Option<Change> {
        let mut change: Option<Change> = None;
        let done = card.checks.iter().filter(|c| c.2).count();
        let expanded = self.expanded.as_deref() == Some(card.id.as_str());
        let r = card_frame(ui, |ui| {
            ui.label(RichText::new(&card.title).color(TEXT).family(egui::FontFamily::Name("semibold".into())));
            if !card.desc.is_empty() {
                let desc: String = if expanded { card.desc.clone() } else { card.desc.chars().take(110).collect() };
                ui.label(RichText::new(desc).size(12.5).color(INK3));
            }
            if !card.checks.is_empty() {
                ui.label(RichText::new(format!("{done} / {} steps", card.checks.len())).size(12.0).color(DIM));
            }
            if !expanded {
                return;
            }
            for (item, text, on) in &card.checks {
                let mut v = *on;
                if ui.checkbox(&mut v, RichText::new(text).size(12.5).color(DIM)).changed() {
                    let (cid, iid) = (card.id.clone(), item.clone());
                    change = Some(Box::new(move |b| set_check(b, &cid, &iid, v)));
                }
            }
            ui.horizontal(|ui| {
                if let Some(t) = todo.filter(|t| *t != list.id && list.title.eq_ignore_ascii_case("proposed")) {
                    let go = egui::Button::new(RichText::new("Approve").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0);
                    if ui.add(go).on_hover_text("Move to To do").clicked() {
                        let (cid, to) = (card.id.clone(), t.to_string());
                        change = Some(Box::new(move |b| move_card(b, &cid, &to)));
                    }
                }
                egui::ComboBox::from_id_salt(("move", &card.id)).selected_text(RichText::new("Move to").size(12.5)).show_ui(ui, |ui| {
                    for l in lists.iter().filter(|l| l.id != list.id) {
                        if ui.selectable_label(false, &l.title).clicked() {
                            let (cid, to) = (card.id.clone(), l.id.clone());
                            change = Some(Box::new(move |b| move_card(b, &cid, &to)));
                        }
                    }
                });
            });
            let (c, give) = self.edit.card_tools(ui, card, agents, doing);
            if c.is_some() {
                change = c;
            }
            if give.is_some() {
                self.give = give;
            }
        });
        if r.clicked() {
            self.expanded = if expanded { None } else { Some(card.id.clone()) };
        }
        change
    }
}

fn agent_columns(ui: &mut egui::Ui, agents: &[AgentCard]) -> Option<String> {
    let mut open = None;
    egui::ScrollArea::horizontal().id_salt("agent-cols").show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.add_space(16.0);
            for (title, states) in COLUMNS {
                let mine: Vec<&AgentCard> = agents.iter().filter(|a| states.contains(&a.state)).collect();
                column(ui, title, mine.len(), |ui| {
                    if mine.is_empty() {
                        ui.label(RichText::new("None").size(12.5).color(INK3));
                    }
                    for a in mine {
                        let r = card_frame(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot.center(), 4.0, state_dot(a.state));
                                ui.label(RichText::new(&a.name).color(TEXT).family(egui::FontFamily::Name("semibold".into())));
                                ui.label(RichText::new(&a.cli).size(12.0).color(INK3));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(&a.cost).size(12.0).monospace().color(INK3));
                                });
                            });
                            ui.label(RichText::new(&a.title).size(12.5).color(DIM));
                            ui.label(RichText::new(&a.where_).size(12.0).monospace().color(INK3));
                        });
                        if r.clicked() {
                            open = Some(a.pane_id.clone());
                        }
                    }
                });
            }
        });
    });
    open
}

/// A board column: title and count, then its cards.
fn column(ui: &mut egui::Ui, title: &str, n: usize, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(egui::vec2(COL_W, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(COL_W);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(12.5).color(INK3));
            ui.label(RichText::new(n.to_string()).size(12.5).monospace().color(INK3));
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt(("col", title)).auto_shrink([false, false]).show(ui, add);
    });
    ui.add_space(12.0);
}

/// A raised card; the whole card is the click target.
fn card_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.contains_pointer());
    let fill = if hovered { crate::theme::SURFACE_HIGH } else { SURFACE };
    let r = egui::Frame::new()
        .fill(fill)
        .corner_radius(R3)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        })
        .response;
    ui.add_space(8.0);
    ui.interact(r.rect, id, egui::Sense::click())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_paints_both_halves() {
        let agents = vec![AgentCard { pane_id: "p".into(), name: "Ana".into(), cli: "Claude".into(), title: "Fix login".into(), where_: "bytehive / ana/fix".into(), state: TaskState::Needs, cost: "$0.42".into() }];
        let mut b = BoardView::default();
        let ctx = egui::Context::default();
        for half in [Half::Agents, Half::Plan] {
            b.half = half;
            ctx.run_ui(egui::RawInput::default(), |ui| {
                assert!(b.show(ui, &agents, Some(Path::new("Z:\\none\\board.json")), "bytehive").is_none());
            })
            .drop_without_applying_deltas();
        }
    }
}
