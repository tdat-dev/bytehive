//! Acting on an agent's state changing (onStateChanges in inbox.ts), Settings
//! > Inbox: bring an agent that starts waiting on you onto the stage when the
//! one there is not busy (Show the agent that needs me), and open Changes when
//! the agent on the stage finishes with changes (Open Changes when an agent
//! finishes). The tick notes what moved; the next render, which has the
//! window, acts, never while you are typing, and never while the Grid is on:
//! it already shows every agent.

use super::*;

#[derive(Default)]
pub(super) struct Moves {
    /// An agent to bring onto the stage.
    jump: Option<String>,
    /// The agent on the stage whose Changes should open.
    review: Option<String>,
}

impl Inbox {
    /// From the tick: `moved` is (seat index, state before, state now).
    pub(super) fn note_moves(&mut self, moved: &[(usize, State, State)], statuses: &[TaskStatus]) {
        let stage = if self.home { None } else { self.fleet.focused.filter(|i| *i < self.fleet.seats.len()) };
        let stage_busy = stage.and_then(|i| statuses.get(i)).is_some_and(|s| matches!(s.state, State::Needs | State::Working));
        for &(index, before, now) in moved {
            let id = &self.fleet.seats[index].spec.id;
            if now == State::Needs && self.prefs.jump_to_needs && !self.grid.on && Some(index) != stage && !stage_busy {
                self.moves.jump = Some(id.clone());
                return;
            }
            if now == State::Review && before == State::Working && Some(index) == stage && self.prefs.review_on_done && self.changes.is_none() && !self.grid.on {
                self.moves.review = Some(id.clone());
            }
        }
    }

    /// You are typing somewhere or looking at a panel: leave the stage be.
    pub(super) fn busy_elsewhere(&self, window: &Window, cx: &App) -> bool {
        let open = self.sheet.is_some()
            || self.palette.is_some()
            || self.settings.is_some()
            || self.sessions.is_some()
            || self.memory.is_some()
            || self.usage_view.is_some()
            || self.schedules.is_some()
            || self.board.is_some()
            || self.changes.is_some()
            || self.history.open.is_some()
            || self.menu.is_some()
            || self.popup.is_some()
            || self.removing.is_some();
        open || (self.home && self.compose.as_ref().is_some_and(|c| c.job.focus_handle(cx).contains_focused(window, cx)))
    }

    /// From render: do what the tick noted.
    pub(super) fn act_on_moves(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (jump, review) = (self.moves.jump.take(), self.moves.review.take());
        if jump.is_none() && review.is_none() {
            return;
        }
        if self.busy_elsewhere(window, cx) {
            return;
        }
        let at = |id: &String| self.fleet.seats.iter().position(|s| &s.spec.id == id);
        if let Some(index) = jump.as_ref().and_then(at) {
            self.home = false;
            self.fleet.focus(index, window, cx);
            cx.notify();
        } else if let Some(index) = review.as_ref().and_then(at).filter(|i| Some(*i) == self.fleet.focused && !self.home) {
            self.open_changes(index, window, cx);
        }
    }
}
