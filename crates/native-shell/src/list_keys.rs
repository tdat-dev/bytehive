//! The old app's Alt keys for the agent list, when no terminal has the keys
//! (there they are the program's): Alt+Up and Alt+Down step through the agents as the list shows
//! them, Alt+S switches Focus and Grid, Alt+R opens or closes Changes,
//! Alt+H History, and Alt+O Sessions.

use eframe::egui::{self, Key, Modifiers};

use crate::app::ShellApp;
use crate::commands::SideTab;
use crate::dock::View;

/// The agent `delta` places from `cur` in `order`, held at the ends; the
/// first when none is picked.
pub fn step(order: &[String], cur: Option<&str>, delta: i32) -> Option<String> {
    if order.is_empty() {
        return None;
    }
    let i = cur.and_then(|c| order.iter().position(|x| x == c));
    let next = match i {
        None => 0,
        Some(i) => (i as i32 + delta).clamp(0, order.len() as i32 - 1) as usize,
    };
    order.get(next).cloned()
}

impl ShellApp {
    pub(crate) fn list_keys(&mut self, ctx: &egui::Context) {
        let take = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::ALT, k));
        let delta = if take(Key::ArrowDown) {
            Some(1)
        } else if take(Key::ArrowUp) {
            Some(-1)
        } else {
            None
        };
        if let Some(delta) = delta {
            if let Some(id) = step(&self.extras.list_order, self.stage.as_deref(), delta) {
                self.put_on_stage(&id);
            }
        } else if take(Key::S) {
            self.extras.view = if self.extras.view == View::Split { View::Queue } else { View::Split };
            self.board.open = false;
            self.extras.home = false;
        } else if take(Key::R) {
            self.extras.stage.history = false;
            if self.changes.open {
                self.changes.open = false;
            } else {
                self.run_command("changes");
            }
        } else if take(Key::H) {
            self.extras.stage.history = !self.extras.stage.history;
            self.changes.open = false;
        } else if take(Key::O) {
            self.side_tab = if self.side_tab == SideTab::Sessions { SideTab::Queue } else { SideTab::Sessions };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::step;

    #[test]
    fn steps_through_the_list_and_stops_at_its_ends() {
        let order: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        assert_eq!(step(&order, Some("a"), 1).as_deref(), Some("b"));
        assert_eq!(step(&order, Some("c"), 1).as_deref(), Some("c"));
        assert_eq!(step(&order, Some("a"), -1).as_deref(), Some("a"));
        assert_eq!(step(&order, None, 1).as_deref(), Some("a"), "none picked: the first");
        assert_eq!(step(&order, Some("gone"), -1).as_deref(), Some("a"));
        assert_eq!(step(&[], None, 1), None);
    }
}
