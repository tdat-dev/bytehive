//! The conductor (conductor.ts): an in-app auto-dispatch scheduler, not
//! another LLM. When on it keeps running agents fed with approved board
//! cards: each FREE agent (running, not already on a Doing card) gets the
//! next unassigned "To do" card. In "auto" it also tops up To do from
//! Proposed so agents don't stall waiting for you to approve.
//!
//! Pure planner: given the mode, board and agents it returns the moves to
//! make; the Inbox carries them out (inbox/board/conduct.rs).

use std::collections::HashSet;

use crate::board::{Board, List};

/// "pipeline" is planned by pipeline.rs, not plan_conductor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Semi,
    Auto,
    Pipeline,
}

impl Mode {
    /// As kanban.ts saves it.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Semi => "semi",
            Mode::Auto => "auto",
            Mode::Pipeline => "pipeline",
        }
    }

    pub fn from_key(key: &str) -> Option<Mode> {
        [Mode::Off, Mode::Semi, Mode::Auto, Mode::Pipeline].into_iter().find(|m| m.key() == key)
    }

    /// The header button cycles Off → Semi → Auto → Pipeline → Off.
    pub fn next(self) -> Mode {
        match self {
            Mode::Off => Mode::Semi,
            Mode::Semi => Mode::Auto,
            Mode::Auto => Mode::Pipeline,
            Mode::Pipeline => Mode::Off,
        }
    }
}

/// An agent of the board's project (ConductorAgent, PipelineAgent).
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub running: bool,
}

/// Card → agent: type it in, and pull the card into Doing.
#[derive(Clone, Debug, PartialEq)]
pub struct Dispatch {
    pub card_id: String,
    pub agent_id: String,
    pub agent_name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Card ids to approve (Proposed → To do); auto only.
    pub approvals: Vec<String>,
    pub dispatches: Vec<Dispatch>,
}

pub(crate) fn by_title<'a>(board: &'a Board, title: &str) -> Option<&'a List> {
    board.lists.iter().find(|l| l.title.trim().to_lowercase() == title)
}

/// This tick's moves. Deterministic; the caller applies them.
pub fn plan_conductor(mode: Mode, board: &Board, agents: &[Agent]) -> Plan {
    // Only semi and auto run here; off does nothing and pipeline is planned elsewhere.
    if mode != Mode::Semi && mode != Mode::Auto {
        return Plan::default();
    }
    let cards = |title: &str| by_title(board, title).map(|l| l.cards.as_slice()).unwrap_or_default();
    let busy: HashSet<&str> = cards("doing").iter().filter_map(|c| c.assignee.as_deref()).collect();
    // A free agent is running and not already tied to a card in Doing.
    let free: Vec<&Agent> = agents.iter().filter(|a| a.running && !busy.contains(a.name.as_str())).collect();
    if free.is_empty() {
        return Plan::default();
    }
    let mut queue: Vec<String> = cards("to do").iter().filter(|c| c.assignee.is_none()).map(|c| c.id.clone()).collect();
    let mut approvals = Vec::new();
    if mode == Mode::Auto && queue.len() < free.len() {
        let need = free.len() - queue.len();
        for c in cards("proposed").iter().take(need) {
            approvals.push(c.id.clone());
            // Approved cards are dispatchable this same tick.
            queue.push(c.id.clone());
        }
    }
    let dispatches = queue.into_iter().zip(free).map(|(card_id, a)| Dispatch { card_id, agent_id: a.id.clone(), agent_name: a.name.clone() }).collect();
    Plan { approvals, dispatches }
}

#[cfg(test)]
pub(crate) mod tests {
    //! Ported from src/conductor.test.ts.

    use super::*;
    use crate::board::{Card, mk_card};

    pub fn card(id: &str, assignee: Option<&str>) -> Card {
        let mut c = mk_card(id);
        c.id = id.to_string();
        c.assignee = assignee.map(str::to_string);
        c
    }

    pub fn board(lists: Vec<(&str, Vec<Card>)>) -> Board {
        Board { lists: lists.into_iter().enumerate().map(|(i, (title, cards))| List { id: format!("l{i}"), title: title.to_string(), cards }).collect() }
    }

    pub fn agent(name: &str, running: bool) -> Agent {
        Agent { id: format!("a-{name}"), name: name.to_string(), running }
    }

    fn sent(card: &str, name: &str) -> Dispatch {
        Dispatch { card_id: card.to_string(), agent_id: format!("a-{name}"), agent_name: name.to_string() }
    }

    #[test]
    fn off_mode_does_nothing() {
        let b = board(vec![("To do", vec![card("c1", None)])]);
        assert_eq!(plan_conductor(Mode::Off, &b, &[agent("A", true)]), Plan::default());
    }

    #[test]
    fn semi_pairs_free_agents_with_unassigned_to_do_cards() {
        let b = board(vec![("To do", vec![card("c1", None), card("c2", None)]), ("Doing", vec![])]);
        let plan = plan_conductor(Mode::Semi, &b, &[agent("A", true), agent("B", true)]);
        assert!(plan.approvals.is_empty());
        assert_eq!(plan.dispatches, vec![sent("c1", "A"), sent("c2", "B")]);
    }

    #[test]
    fn skips_agents_already_on_a_doing_card_and_cards_already_assigned() {
        let b = board(vec![("To do", vec![card("c1", Some("A")), card("c2", None)]), ("Doing", vec![card("d1", Some("A"))])]);
        // A is busy (Doing d1); only B is free; c1 is already assigned so c2 is next.
        let plan = plan_conductor(Mode::Semi, &b, &[agent("A", true), agent("B", true)]);
        assert_eq!(plan.dispatches, vec![sent("c2", "B")]);
    }

    #[test]
    fn semi_never_touches_proposed_even_when_to_do_is_empty() {
        let b = board(vec![("Proposed", vec![card("p1", None)]), ("To do", vec![])]);
        assert_eq!(plan_conductor(Mode::Semi, &b, &[agent("A", true)]), Plan::default());
    }

    #[test]
    fn auto_approves_proposed_cards_to_feed_idle_agents_then_dispatches_them() {
        let b = board(vec![("Proposed", vec![card("p1", None), card("p2", None), card("p3", None)]), ("To do", vec![]), ("Doing", vec![])]);
        let plan = plan_conductor(Mode::Auto, &b, &[agent("A", true), agent("B", true)]);
        // Only as many as there are free agents.
        assert_eq!(plan.approvals, vec!["p1", "p2"]);
        assert_eq!(plan.dispatches, vec![sent("p1", "A"), sent("p2", "B")]);
    }

    #[test]
    fn ignores_stopped_agents() {
        let b = board(vec![("To do", vec![card("c1", None)])]);
        assert!(plan_conductor(Mode::Semi, &b, &[agent("A", false)]).dispatches.is_empty());
    }

    #[test]
    fn the_button_cycles_and_keys_round_trip() {
        let order = [Mode::Off, Mode::Semi, Mode::Auto, Mode::Pipeline];
        for (i, m) in order.iter().enumerate() {
            assert_eq!(m.next(), order[(i + 1) % 4]);
            assert_eq!(Mode::from_key(m.key()), Some(*m));
        }
        assert_eq!(Mode::from_key("turbo"), None);
    }
}
