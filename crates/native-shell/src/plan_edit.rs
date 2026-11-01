//! Making and changing plan cards by hand on the Board's Plan half: add a
//! card to any list (title, note, one step per line), edit a card's title
//! and note, delete it (asks first), and give it to an agent, which types it
//! in as a task, marks the agent on the card and moves it to Doing (the web
//! app did that by dragging the card onto a terminal).

use eframe::egui::{self, RichText};
use serde_json::Value;

use crate::board_ui::AgentCard;
use crate::plan_board::{add_card, assign, card_task, delete_card, move_card, update_card, CardView};
use crate::theme::{DEL, INK3};

pub type Change = Box<dyn FnOnce(&mut Value) -> bool>;

#[derive(Default)]
pub struct PlanEdit {
    /// The list a card is being added to: (list id, title, note, steps).
    adding: Option<(String, String, String, String)>,
    /// The card being edited: (id, title, note).
    editing: Option<(String, String, String)>,
    deleting: Option<String>,
}

impl PlanEdit {
    /// "+ Add a card" at the foot of a list, and its form when open.
    pub fn add_box(&mut self, ui: &mut egui::Ui, list_id: &str) -> Option<Change> {
        let open = self.adding.as_ref().is_some_and(|a| a.0 == list_id);
        if !open {
            if ui.add(egui::Button::new(RichText::new("+ Add a card").size(12.5).color(INK3)).frame(false)).clicked() {
                self.adding = Some((list_id.to_string(), String::new(), String::new(), String::new()));
            }
            return None;
        }
        let (mut add, mut cancel) = (false, false);
        if let Some((_, title, desc, steps)) = self.adding.as_mut() {
            ui.add(egui::TextEdit::singleline(title).hint_text("Title").desired_width(f32::INFINITY));
            ui.add(egui::TextEdit::multiline(desc).hint_text("Note (optional)").desired_rows(2).desired_width(f32::INFINITY));
            ui.add(egui::TextEdit::multiline(steps).hint_text("Steps, one per line (optional)").desired_rows(2).desired_width(f32::INFINITY));
            let ready = !title.trim().is_empty();
            ui.horizontal(|ui| {
                add = ui.add_enabled(ready, egui::Button::new("Add")).clicked();
                cancel = ui.add(egui::Button::new(RichText::new("Cancel").color(INK3)).frame(false)).clicked();
            });
        }
        if cancel {
            self.adding = None;
        }
        if !add {
            return None;
        }
        let (list, title, desc, steps) = self.adding.take()?;
        let steps: Vec<String> = steps.lines().map(str::to_string).collect();
        Some(Box::new(move |b| add_card(b, &list, &title, &desc, &steps)))
    }

    /// Edit, Delete and Give to under an open card. The change to write, and
    /// a task to type into an agent (pane id, text).
    pub fn card_tools(&mut self, ui: &mut egui::Ui, card: &CardView, agents: &[AgentCard], doing: Option<&str>) -> (Option<Change>, Option<(String, String)>) {
        let mut change: Option<Change> = None;
        let mut give = None;
        if self.editing.as_ref().is_some_and(|e| e.0 == card.id) {
            let (mut save, mut cancel) = (false, false);
            if let Some((_, title, desc)) = self.editing.as_mut() {
                ui.add(egui::TextEdit::singleline(title).desired_width(f32::INFINITY));
                ui.add(egui::TextEdit::multiline(desc).desired_rows(3).desired_width(f32::INFINITY));
                let ready = !title.trim().is_empty();
                ui.horizontal(|ui| {
                    save = ui.add_enabled(ready, egui::Button::new("Save")).clicked();
                    cancel = ui.add(egui::Button::new(RichText::new("Cancel").color(INK3)).frame(false)).clicked();
                });
            }
            if save {
                if let Some((id, title, desc)) = self.editing.take() {
                    change = Some(Box::new(move |b| update_card(b, &id, &title, &desc)));
                }
            } else if cancel {
                self.editing = None;
            }
            return (change, None);
        }
        if self.deleting.as_deref() == Some(card.id.as_str()) {
            ui.label(RichText::new("Delete this card?").size(12.5).color(DEL));
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Delete").color(DEL))).clicked() {
                    let id = card.id.clone();
                    change = Some(Box::new(move |b| delete_card(b, &id)));
                    self.deleting = None;
                } else if ui.add(egui::Button::new(RichText::new("Keep it").color(INK3)).frame(false)).clicked() {
                    self.deleting = None;
                }
            });
            return (change, None);
        }
        ui.horizontal_wrapped(|ui| {
            let live: Vec<&AgentCard> = agents.iter().filter(|a| a.cli != "Shell").collect();
            if !live.is_empty() {
                egui::ComboBox::from_id_salt(("give", &card.id)).selected_text(RichText::new("Give to").size(12.5)).show_ui(ui, |ui| {
                    for a in live {
                        if ui.selectable_label(false, format!("{} ({})", a.name, a.cli)).clicked() {
                            give = Some((a.pane_id.clone(), card_task(card)));
                            let (id, who, to) = (card.id.clone(), a.name.clone(), doing.map(str::to_string));
                            change = Some(Box::new(move |b| assign(b, &id, &who) && to.is_none_or(|t| move_card(b, &id, &t))));
                        }
                    }
                });
            }
            if ui.add(egui::Button::new(RichText::new("Edit").size(12.5)).frame(false)).clicked() {
                self.editing = Some((card.id.clone(), card.title.clone(), card.desc.clone()));
            }
            if ui.add(egui::Button::new(RichText::new("Delete").size(12.5).color(DEL)).frame(false)).clicked() {
                self.deleting = Some(card.id.clone());
            }
        });
        (change, give)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supervise::task_state::TaskState;

    #[test]
    fn the_tools_paint_and_add_needs_a_title() {
        let mut e = PlanEdit::default();
        let card = CardView { id: "c1".into(), title: "Fix".into(), desc: String::new(), checks: vec![] };
        let agents = [AgentCard { pane_id: "p1".into(), name: "Ana".into(), cli: "Claude".into(), title: String::new(), where_: String::new(), state: TaskState::Idle, cost: String::new() }];
        e.adding = Some(("l1".into(), " ".into(), String::new(), String::new()));
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(e.add_box(ui, "l1").is_none(), "no title, nothing to add");
            let (c, g) = e.card_tools(ui, &card, &agents, Some("l2"));
            assert!(c.is_none() && g.is_none());
        })
        .drop_without_applying_deltas();
    }
}
