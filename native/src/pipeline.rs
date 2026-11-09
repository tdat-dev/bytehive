//! Pipelines (pipeline.ts): cards flow through work stages (Build → Test →
//! Review) between the "To do" entry and "Done". In pipeline mode the
//! conductor hands each card in a work stage to a free agent with a
//! STAGE-SPECIFIC prompt, and the agent moves the card on when finished.
//! The conductor sees that hand-off (the card changed stage) and clears the
//! assignee so the next stage can be picked up, maybe by another agent.
//!
//! Pure: plan_pipeline and its helpers are unit-tested; the Inbox applies.

use std::collections::{HashMap, HashSet};

use crate::board::{Board, Card, List};
use crate::conductor::Agent;

pub struct Stage {
    pub title: &'static str,
    pub instruction: &'static str,
}

pub const ENTRY: &str = "To do";
pub const TERMINAL: &str = "Done";

pub const DEFAULT_PIPELINE: [Stage; 3] = [
    Stage { title: "Build", instruction: "Implement this task end to end." },
    Stage { title: "Test", instruction: "Write and run tests for this task; fix any failures." },
    Stage { title: "Review", instruction: "Review the change for correctness, bugs, and clarity; fix issues you find." },
];

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Ordered titles a card can be dispatched from: entry + every work stage.
pub fn dispatch_stages(pipeline: &[Stage]) -> Vec<&str> {
    std::iter::once(ENTRY).chain(pipeline.iter().map(|s| s.title)).collect()
}

/// The stage a card goes to when its current one is finished. A To-do card
/// enters the first work stage; the last work stage goes to Done.
pub fn next_title<'a>(pipeline: &'a [Stage], current: &str) -> &'a str {
    let c = norm(current);
    if c == norm(ENTRY) {
        return pipeline.first().map_or(TERMINAL, |s| s.title);
    }
    let Some(i) = pipeline.iter().position(|s| norm(s.title) == c) else { return TERMINAL };
    pipeline.get(i + 1).map_or(TERMINAL, |s| s.title)
}

/// The work stage with this list title, if it is one.
pub fn stage_for<'a>(pipeline: &'a [Stage], title: &str) -> Option<&'a Stage> {
    pipeline.iter().find(|s| norm(s.title) == norm(title))
}

/// The text typed for a card entering `current`: `base` (dispatch_prompt)
/// plus the stage's instruction and where to move the card when done. For
/// the To-do entry the stage is the first work stage.
pub fn pipeline_prompt(base: &str, pipeline: &[Stage], current: &str) -> String {
    let effective = if norm(current) == norm(ENTRY) { pipeline.first() } else { stage_for(pipeline, current) };
    let Some(stage) = effective else { return base.to_string() };
    let next = next_title(pipeline, stage.title);
    let to = if next == TERMINAL { "call card_done with a one-line summary".to_string() } else { format!("use the bytehive card_move tool to move this card to \"{next}\"") };
    format!("{base}\n\nPipeline stage ({}): {} When finished, {to}.", stage.title, stage.instruction)
}

fn by_title<'a>(board: &'a Board, title: &str) -> Option<&'a List> {
    board.lists.iter().find(|l| norm(&l.title) == norm(title))
}

/// Cards whose assignee is to be cleared: they moved to another stage than
/// the one they were dispatched into (a hand-off completed), or are gone.
pub fn stale_assignments(board: &Board, assigned: &HashMap<String, String>) -> Vec<String> {
    let mut out: Vec<String> = assigned
        .iter()
        .filter(|(id, at)| {
            let cur = board.lists.iter().rev().find(|l| l.cards.iter().any(|c| &c.id == *id)).map(|l| l.title.as_str());
            cur.is_none_or(|cur| norm(cur) != norm(at))
        })
        .map(|(id, _)| id.clone())
        .collect();
    // A Map keeps insertion order; a HashMap has none, so keep it steady.
    out.sort();
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dispatch {
    pub card_id: String,
    pub agent_id: String,
    pub agent_name: String,
    /// The list the card is in now.
    pub from_title: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Cards to clear (stale hand-offs).
    pub unassign: Vec<String>,
    pub dispatches: Vec<Dispatch>,
}

/// One pipeline tick. `assigned` maps an in-flight card id to the stage it
/// was dispatched into, kept by the conductor between ticks.
pub fn plan_pipeline(board: &Board, agents: &[Agent], pipeline: &[Stage], assigned: &HashMap<String, String>) -> Plan {
    let unassign = stale_assignments(board, assigned);
    let stale: HashSet<&str> = unassign.iter().map(String::as_str).collect();
    // Assigned only when its assignee is set AND it isn't stale.
    let assignee = |c: &'_ Card| if stale.contains(c.id.as_str()) { None } else { c.assignee.clone() };
    let stages = dispatch_stages(pipeline);
    let cards = || stages.iter().flat_map(|t| by_title(board, t).into_iter().flat_map(move |l| l.cards.iter().map(move |c| (c, *t))));

    // Busy agents own a non-stale card in a dispatchable stage.
    let busy: HashSet<String> = cards().filter_map(|(c, _)| assignee(c)).collect();
    let free: Vec<&Agent> = agents.iter().filter(|a| a.running && !busy.contains(&a.name)).collect();
    if free.is_empty() {
        return Plan { unassign, dispatches: Vec::new() };
    }
    // Unassigned cards across dispatchable stages, earliest stage first so
    // nothing is starved at the front of the line.
    let queue = cards().filter(|(c, _)| assignee(c).is_none());
    let dispatches = queue.zip(free).map(|((c, t), a)| Dispatch { card_id: c.id.clone(), agent_id: a.id.clone(), agent_name: a.name.clone(), from_title: t.to_string() }).collect();
    Plan { unassign, dispatches }
}

#[cfg(test)]
mod tests {
    //! Ported from src/pipeline.test.ts.

    use super::*;
    use crate::conductor::tests::{agent, board, card};

    fn at(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn to_do_goes_to_the_first_work_stage_then_along_ending_at_done() {
        assert_eq!(next_title(&DEFAULT_PIPELINE, "To do"), "Build");
        assert_eq!(next_title(&DEFAULT_PIPELINE, "Build"), "Test");
        assert_eq!(next_title(&DEFAULT_PIPELINE, "Test"), "Review");
        assert_eq!(next_title(&DEFAULT_PIPELINE, "Review"), "Done");
    }

    #[test]
    fn prompt_adds_the_stage_instruction_and_the_next_stage_move() {
        let p = pipeline_prompt("BASE", &DEFAULT_PIPELINE, "Build");
        assert!(p.contains("BASE"));
        assert!(p.contains("Pipeline stage (Build)"));
        assert!(p.contains("move this card to \"Test\""));
    }

    #[test]
    fn the_last_stage_instructs_card_done_instead_of_a_move() {
        assert!(pipeline_prompt("BASE", &DEFAULT_PIPELINE, "Review").contains("card_done"));
    }

    #[test]
    fn from_to_do_the_effective_stage_is_build() {
        assert!(pipeline_prompt("BASE", &DEFAULT_PIPELINE, "To do").contains("Pipeline stage (Build)"));
        assert_eq!(pipeline_prompt("BASE", &DEFAULT_PIPELINE, "Backlog"), "BASE");
    }

    #[test]
    fn flags_a_card_that_moved_to_a_different_stage_than_it_was_assigned_in() {
        let b = board(vec![("Build", vec![card("c1", Some("A"))]), ("Test", vec![card("c2", Some("B"))])]);
        // c1 was assigned in Build (still there, fresh); c2 was assigned in
        // Build but is now in Test, stale.
        assert_eq!(stale_assignments(&b, &at(&[("c1", "Build"), ("c2", "Build")])), vec!["c2"]);
    }

    #[test]
    fn flags_a_card_that_vanished() {
        let b = board(vec![("Build", vec![])]);
        assert_eq!(stale_assignments(&b, &at(&[("gone", "Build")])), vec!["gone"]);
    }

    #[test]
    fn dispatches_unassigned_work_stage_cards_to_free_agents() {
        let b = board(vec![("To do", vec![card("c1", None)]), ("Build", vec![card("c2", None)])]);
        let plan = plan_pipeline(&b, &[agent("A", true), agent("B", true)], &DEFAULT_PIPELINE, &HashMap::new());
        let mut ids: Vec<&str> = plan.dispatches.iter().map(|d| d.card_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["c1", "c2"]);
    }

    #[test]
    fn treats_a_stale_card_as_free_again_and_unassigns_it() {
        // c1 moved on from Build.
        let b = board(vec![("Build", vec![]), ("Test", vec![card("c1", Some("A"))])]);
        let plan = plan_pipeline(&b, &[agent("A", true)], &DEFAULT_PIPELINE, &at(&[("c1", "Build")]));
        assert_eq!(plan.unassign, vec!["c1"]);
        // A is free now (its old card is stale) and picks c1 back up for Test.
        assert_eq!(plan.dispatches, vec![Dispatch { card_id: "c1".into(), agent_id: "a-A".into(), agent_name: "A".into(), from_title: "Test".into() }]);
    }

    #[test]
    fn keeps_an_agent_busy_while_its_card_sits_fresh_in_its_assigned_stage() {
        let b = board(vec![("Build", vec![card("c1", Some("A"))])]);
        assert!(plan_pipeline(&b, &[agent("A", true)], &DEFAULT_PIPELINE, &at(&[("c1", "Build")])).dispatches.is_empty());
    }

    #[test]
    fn ignores_done_cards_and_stopped_agents() {
        let b = board(vec![("Build", vec![card("c1", None)]), ("Done", vec![card("d1", None)])]);
        assert!(plan_pipeline(&b, &[agent("A", false)], &DEFAULT_PIPELINE, &HashMap::new()).dispatches.is_empty());
    }
}
