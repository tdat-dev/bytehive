//! One round of the board's background timer on a project's files
//! (kanban.ts: pollPlan + importProposed, conductorTick, pipelineTick):
//! agent-proposed tasks in .bytehive/plan.json land in Proposed, then the
//! conductor approves and picks what to type into which agent. Blocking on
//! the disk: run it off the UI thread. Typing happens on the UI thread, and
//! only what was typed is claimed afterwards (`claim`).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::SystemTime;

use super::{Board, ChecklistItem, assign_card, change, default_board, dispatch_prompt, find_card, find_or_create_list, mk_card, read, uid};
use crate::conductor::{Agent, Mode, plan_conductor};
use crate::pipeline::{DEFAULT_PIPELINE, ENTRY, pipeline_prompt, plan_pipeline};
use crate::planparse::{PlanTask, parse_plan};

const PROPOSED_TITLE: &str = "Proposed";

/// What a round starts from.
pub struct Ask {
    pub dir: String,
    pub mode: Mode,
    /// The project's agents.
    pub agents: Vec<Agent>,
    /// Task titles already imported, lower-cased (they stay out once deleted).
    pub seen: HashSet<String>,
    /// plan.json's time as last imported.
    pub plan_mtime: Option<SystemTime>,
    /// Pipeline: card id → the stage it was dispatched into.
    pub assigned: HashMap<String, String>,
    /// Pipeline just turned on: take `assigned` from cards already assigned,
    /// so a hand-off isn't misread after a restart.
    pub seed: bool,
}

/// A card to type into an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    pub card: String,
    pub agent: String,
    pub name: String,
    pub prompt: String,
    /// Pipeline: the stage the card works in; None pulls it into Doing.
    pub stage: Option<String>,
}

pub struct Round {
    pub plan_mtime: Option<SystemTime>,
    /// Titles imported this round, lower-cased.
    pub imported: Vec<String>,
    pub assigned: HashMap<String, String>,
    /// `seed` was used up.
    pub seeded: bool,
    pub sends: Vec<Send>,
    /// The board as this round left it, the fallback for `claim`.
    pub board: Board,
}

pub fn plan_path(dir: &str) -> std::path::PathBuf {
    Path::new(dir).join(".bytehive").join("plan.json")
}

/// Agent-proposed tasks as cards at the end of Proposed (importProposed):
/// a blue label unless they name one, steps as the checklist.
pub fn import_proposed(board: &mut Board, tasks: &[PlanTask]) {
    let list = find_or_create_list(board, PROPOSED_TITLE);
    for t in tasks {
        let mut card = mk_card(&t.title);
        if let Some(desc) = &t.desc {
            card.desc = desc.clone();
        }
        card.labels = vec![t.label.clone().unwrap_or_else(|| "blue".to_string())];
        if let Some(steps) = &t.subtasks {
            card.checklist = steps.iter().map(|text| ChecklistItem { id: uid("i"), text: text.clone(), done: false }).collect();
        }
        board.lists[list].cards.push(card);
    }
}

/// The tasks not imported before and not on the board already.
pub fn fresh_tasks(board: &Board, tasks: Vec<PlanTask>, seen: &HashSet<String>) -> Vec<PlanTask> {
    let on_board = |title: &str| board.lists.iter().any(|l| l.cards.iter().any(|c| c.title.to_lowercase() == title.to_lowercase()));
    tasks.into_iter().filter(|t| !seen.contains(&t.title.to_lowercase()) && !on_board(&t.title)).collect()
}

/// Apply a change to the file; the board on hand when it can't be saved.
fn write(dir: &str, board: &mut Board, f: impl Fn(&mut Board)) -> bool {
    match change(dir, board, &f) {
        Ok((b, _)) => {
            *board = b;
            true
        }
        Err(_) => false,
    }
}

pub fn run(ask: Ask) -> Round {
    let Ask { dir, mode, agents, seen, mut plan_mtime, mut assigned, seed } = ask;
    // No file yet is a fresh board; a corrupt or unreadable one is left alone.
    let mut board = match read(&dir) {
        Ok(Some((b, _))) => b,
        Ok(None) => default_board(),
        Err(_) => return Round { plan_mtime, imported: Vec::new(), assigned, seeded: false, sends: Vec::new(), board: Board::default() },
    };

    // The agent (re)wrote plan.json: import what is new.
    let mut imported = Vec::new();
    let now = std::fs::metadata(plan_path(&dir)).and_then(|m| m.modified()).ok();
    if now.is_some() && now != plan_mtime {
        if let Ok(text) = std::fs::read_to_string(plan_path(&dir)) {
            let fresh = fresh_tasks(&board, parse_plan(&text), &seen);
            // Not saved: tried again next round.
            if fresh.is_empty() || write(&dir, &mut board, |b| import_proposed(b, &fresh)) {
                plan_mtime = now;
                imported = fresh.iter().map(|t| t.title.to_lowercase()).collect();
            }
        }
    }

    let mut sends = Vec::new();
    let mut seeded = false;
    match mode {
        Mode::Off => {}
        Mode::Semi | Mode::Auto => {
            let plan = plan_conductor(mode, &board, &agents);
            // Auto-approve: the planned Proposed cards into To do in one write.
            if !plan.approvals.is_empty() {
                write(&dir, &mut board, |b| {
                    let todo = find_or_create_list(b, "To do");
                    for id in &plan.approvals {
                        if let Some((l, c)) = find_card(b, id).filter(|(l, _)| *l != todo) {
                            let card = b.lists[l].cards.remove(c);
                            b.lists[todo].cards.push(card);
                        }
                    }
                });
            }
            for d in plan.dispatches {
                let Some((l, c)) = find_card(&board, &d.card_id) else { continue };
                let prompt = dispatch_prompt(&board.lists[l].cards[c]);
                sends.push(Send { card: d.card_id, agent: d.agent_id, name: d.agent_name, prompt, stage: None });
            }
        }
        Mode::Pipeline => {
            if seed {
                seeded = true;
                assigned = board.lists.iter().flat_map(|l| l.cards.iter().filter(|c| c.assignee.is_some()).map(|c| (c.id.clone(), l.title.clone()))).collect();
            }
            let plan = plan_pipeline(&board, &agents, &DEFAULT_PIPELINE, &assigned);
            if !plan.unassign.is_empty() {
                for id in &plan.unassign {
                    assigned.remove(id);
                }
                write(&dir, &mut board, |b| {
                    for id in &plan.unassign {
                        if let Some(c) = super::card_mut(b, id) {
                            c.assignee = None;
                        }
                    }
                });
            }
            for d in plan.dispatches {
                let Some((l, c)) = find_card(&board, &d.card_id) else { continue };
                // A To-do card enters the first work stage; a work-stage card stays put.
                let stage = if d.from_title.trim().eq_ignore_ascii_case(ENTRY) { DEFAULT_PIPELINE[0].title.to_string() } else { d.from_title };
                let prompt = pipeline_prompt(&dispatch_prompt(&board.lists[l].cards[c]), &DEFAULT_PIPELINE, &stage);
                sends.push(Send { card: d.card_id, agent: d.agent_id, name: d.agent_name, prompt, stage: Some(stage) });
            }
        }
    }
    Round { plan_mtime, imported, assigned, seeded, sends, board }
}

/// What was typed, claimed: the agent's name on the card, and the card in
/// Doing, or in its pipeline stage.
pub fn claim(dir: &str, fallback: &Board, sent: &[Send]) {
    let _ = change(dir, fallback, &|b: &mut Board| {
        for s in sent {
            let Some(stage) = &s.stage else {
                assign_card(b, &s.card, Some(&s.name), true);
                continue;
            };
            let Some((l, c)) = find_card(b, &s.card) else { continue };
            b.lists[l].cards[c].assignee = Some(s.name.clone());
            if b.lists[l].title.trim().to_lowercase() != stage.trim().to_lowercase() {
                let card = b.lists[l].cards.remove(c);
                let dest = find_or_create_list(b, stage);
                b.lists[dest].cards.push(card);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conductor::tests::agent;

    fn ask(dir: &str, mode: Mode, agents: Vec<Agent>) -> Ask {
        Ask { dir: dir.to_string(), mode, agents, seen: HashSet::new(), plan_mtime: None, assigned: HashMap::new(), seed: false }
    }

    fn titles(board: &Board, list: &str) -> Vec<String> {
        board.lists.iter().find(|l| l.title == list).map(|l| l.cards.iter().map(|c| c.title.clone()).collect()).unwrap_or_default()
    }

    fn write_plan(dir: &str, text: &str) {
        std::fs::create_dir_all(Path::new(dir).join(".bytehive")).unwrap();
        std::fs::write(plan_path(dir), text).unwrap();
    }

    #[test]
    fn imports_a_plan_into_proposed_once() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().to_string_lossy().into_owned();
        write_plan(&dir, r#"[{"title":"Add auth","label":"red","subtasks":["jwt"]},"Tests"]"#);
        let r = run(ask(&dir, Mode::Off, vec![]));
        assert_eq!(r.imported, vec!["add auth", "tests"]);
        let board = read(&dir).unwrap().unwrap().0;
        // Proposed sits first; a task without a label is blue.
        assert_eq!(board.lists[0].title, "Proposed");
        assert_eq!(titles(&board, "Proposed"), ["Add auth", "Tests"]);
        let first = &board.lists[0].cards[0];
        assert_eq!((first.labels.clone(), first.checklist[0].text.as_str()), (vec!["red".to_string()], "jwt"));
        assert_eq!(board.lists[0].cards[1].labels, vec!["blue"]);
        // The same file again: nothing; a deleted card stays out once seen.
        let again = run(Ask { plan_mtime: r.plan_mtime, ..ask(&dir, Mode::Off, vec![]) });
        assert!(again.imported.is_empty());
        let seen: HashSet<String> = r.imported.into_iter().collect();
        let tasks = parse_plan(r#"["Tests","New one"]"#);
        assert_eq!(fresh_tasks(&default_board(), tasks, &seen), vec![PlanTask { title: "New one".into(), ..PlanTask::default() }]);
    }

    #[test]
    fn auto_approves_then_claims_what_was_typed() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().to_string_lossy().into_owned();
        write_plan(&dir, "- one\n- two");
        let r = run(ask(&dir, Mode::Auto, vec![agent("A", true)]));
        assert_eq!(titles(&r.board, "To do"), ["one"]);
        assert_eq!(r.sends.len(), 1);
        assert!(r.sends[0].prompt.starts_with("Task: one") && r.sends[0].stage.is_none());
        claim(&dir, &r.board, &r.sends);
        let board = read(&dir).unwrap().unwrap().0;
        assert_eq!(titles(&board, "Doing"), ["one"]);
        assert_eq!(board.lists.iter().find(|l| l.title == "Doing").unwrap().cards[0].assignee.as_deref(), Some("A"));
        // A is busy now: the next round sends nothing.
        assert!(run(Ask { plan_mtime: r.plan_mtime, ..ask(&dir, Mode::Auto, vec![agent("A", true)]) }).sends.is_empty());
    }

    #[test]
    fn pipeline_moves_a_to_do_card_into_build_and_frees_it_on_hand_off() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().to_string_lossy().into_owned();
        let mut b = default_board();
        let todo = b.lists[0].id.clone();
        super::super::add_card(&mut b, &todo, "task");
        super::super::save(&dir, &b).unwrap();
        let r = run(ask(&dir, Mode::Pipeline, vec![agent("A", true)]));
        assert_eq!(r.sends[0].stage.as_deref(), Some("Build"));
        assert!(r.sends[0].prompt.contains("Pipeline stage (Build)"));
        claim(&dir, &r.board, &r.sends);
        let board = read(&dir).unwrap().unwrap().0;
        assert_eq!(titles(&board, "Build"), ["task"]);
        // The agent moved it on to Test: the assignee is cleared and it goes out again.
        let assigned: HashMap<String, String> = [(r.sends[0].card.clone(), "Build".to_string())].into();
        let mut moved = board.clone();
        let build = moved.lists.iter().position(|l| l.title == "Build").unwrap();
        let card = moved.lists[build].cards.remove(0);
        moved.lists.push(crate::board::List { id: "lt".into(), title: "Test".into(), cards: vec![card] });
        super::super::save(&dir, &moved).unwrap();
        let next = run(Ask { assigned, ..ask(&dir, Mode::Pipeline, vec![agent("A", true)]) });
        assert!(next.assigned.is_empty());
        assert_eq!(next.sends[0].stage.as_deref(), Some("Test"));
        assert!(next.sends[0].prompt.contains("move this card to \"Review\""));
    }
}
