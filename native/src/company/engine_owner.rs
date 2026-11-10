//! What the owner does: give a job, approve or send back a plan, stop a job,
//! raise its cap, close it (engine.ts giveJob, approve, reject, stopJob, raiseCap,
//! finishJob), and the floors and boards those touch.

use super::super::host::{BoardChange, BoardStamp, BoardWrite, Host, Op};
use super::super::org::{floor_id, person_by_name};
use super::super::reduce::{
    approve_plan, close_job, halt_job, new_job, reject_plan, report_failures, resume_job, resumed_notes, stop_turns, stopped_note,
};
use super::super::types::{Company, Floor, Job, JobStatus, PlanCard};
use super::{Engine, NO_PROJECT, same_folder};
use crate::board::{Board, ChecklistItem, List, default_board, mk_card, uid};
use serde_json::Map;

/// Tries at a board an agent keeps writing to.
const BOARD_TRIES: usize = 3;

/// Persona names for hires, in the order they are given out (crew.ts).
const PERSONA_NAMES: [&str; 20] = ["Ana", "Bob", "Cid", "Dot", "Eve", "Fin", "Gio", "Hux", "Ivy", "Jax", "Kim", "Lux", "Mei", "Nix", "Oz", "Pia", "Rue", "Sol", "Tao", "Uma"];

/// The next persona not taken, else "<cli> N" (nameForNewPane).
pub(crate) fn name_for_new(cli: &str, taken: &[String]) -> String {
    PERSONA_NAMES.iter().find(|n| !taken.iter().any(|t| t == *n)).map_or_else(|| format!("{cli} {}", taken.len() + 1), |n| n.to_string())
}

/// Put a Job's cards for one floor at the end of its To do list.
pub(crate) fn place_on_board(board: &mut Board, company: &Company, job: &Job, cards: &[&PlanCard]) {
    let at = match board.lists.iter().position(|l| l.title.trim().to_lowercase() == "to do") {
        Some(at) => at,
        None => {
            board.lists.insert(0, List { id: uid("l"), title: "To do".into(), cards: Vec::new() });
            0
        }
    };
    for c in cards {
        let mut card = mk_card(&c.title);
        card.desc = c.desc.clone();
        card.job = Some(job.id.clone());
        card.checklist = c.checklist.iter().enumerate().map(|(i, text)| ChecklistItem { id: format!("{}-{i}", job.id), text: text.clone(), done: false }).collect();
        if let Some(p) = c.assignee.as_deref().and_then(|a| person_by_name(company, a)) {
            card.assignee = Some(p.name.clone());
            card.assignee_id = Some(p.id.clone());
        }
        board.lists[at].cards.push(card);
    }
}

/// Mark a stopped Job's open cards. Gives back how many changed.
pub(crate) fn mark_job_stopped(board: &mut Board, job: &str) -> usize {
    let mut n = 0;
    for l in board.lists.iter_mut().filter(|l| l.title.trim().to_lowercase() != "done") {
        for c in l.cards.iter_mut().filter(|c| c.job.as_deref() == Some(job) && !c.stopped) {
            c.stopped = true;
            n += 1;
        }
    }
    n
}

/// Make a change to a board. Gives back false when there is nothing to write.
pub(crate) fn apply_change(board: &mut Board, change: &BoardChange) -> bool {
    match change {
        BoardChange::Place { job, cards, company } => {
            place_on_board(board, company, job, &cards.iter().collect::<Vec<_>>());
            true
        }
        BoardChange::MarkStopped { job } => mark_job_stopped(board, job) > 0,
    }
}

/// Change a floor's board (changeBoard). An agent writing it at the same moment
/// is a Conflict: read its copy and try again, three times at most, no waiting.
pub(crate) fn change_board_with(
    read: &mut dyn FnMut() -> Result<Option<(Board, BoardStamp)>, String>,
    write: &mut dyn FnMut(&Board, BoardStamp) -> Result<(), BoardWrite>,
    change: &BoardChange,
) -> Result<(), String> {
    for tries in 1..=BOARD_TRIES {
        let (mut board, seen) = read()?.unwrap_or_else(|| (default_board(), None));
        if !apply_change(&mut board, change) {
            return Ok(());
        }
        match write(&board, seen) {
            Ok(()) => return Ok(()),
            Err(BoardWrite::Conflict) if tries < BOARD_TRIES => continue,
            Err(BoardWrite::Conflict) => return Err("the board kept changing while it was written".into()),
            Err(BoardWrite::Failed(e)) => return Err(e),
        }
    }
    Ok(())
}

impl Engine {
    /// Keep a project's .bytehive/ out of its git status, once per folder per run.
    /// Off the UI thread: a git that fails or hangs never holds up a job, and
    /// one that fails is tried again with the next.
    pub(crate) fn keep_out_of_git(&mut self, host: &mut dyn Host, dir: &str) {
        if !self.excluded.insert(dir.to_lowercase()) {
            return;
        }
        let label = format!("Keeping .bytehive/ out of git in {dir}");
        self.begin(host, Op::Exclude { dir: dir.to_string() }, super::ops::exclude_key(dir), label);
    }

    /// Every open project with a folder is a floor.
    pub(crate) fn sync_floors(&mut self, host: &dyn Host) {
        let Some(s) = &mut self.state else { return };
        for p in host.projects() {
            let Some(dir) = p.dir else { continue };
            if s.company.floors.iter().any(|f| same_folder(&f.dir, &dir)) {
                continue;
            }
            let taken: Vec<String> = s.company.floors.iter().map(|f| f.id.clone()).collect();
            s.company.floors.push(Floor { id: floor_id(&p.name, &taken), dir, name: p.name, extra: Map::new() });
        }
    }

    /// The floors a Job can touch. Never for a launch alone: someone who has not
    /// used the Company has their repo left alone.
    pub(crate) fn exclude_floors(&mut self, host: &mut dyn Host) {
        let dirs: Vec<String> = self.state.as_ref().map(|s| s.company.floors.iter().map(|f| f.dir.clone()).collect()).unwrap_or_default();
        for d in dirs {
            self.keep_out_of_git(host, &d);
        }
    }

    /// The owner gives the Founder a job.
    pub fn give_job(&mut self, host: &mut dyn Host, ask: &str) -> Result<(), String> {
        self.running()?;
        if ask.trim().is_empty() {
            return Ok(());
        }
        if host.seat(super::people::FOUNDER).is_none() && self.founder_dir(host).is_none() {
            return Err(NO_PROJECT.into());
        }
        // Every member's MCP server runs on Node: without it the Founder has no job_plan.
        // Asked off the UI thread; a refusal asks again, as Node may be installed since.
        if let Some(Err(e)) = self.node.clone() {
            self.node = None;
            self.begin(host, Op::NodeCheck, "node".into(), "Checking Node.js".into());
            return Err(e);
        }
        self.sync_floors(host);
        self.exclude_floors(host);
        let Some(s) = &self.state else { return Ok(()) };
        let (r, job) = new_job(s, ask, host.now_ms().max(0) as u64);
        self.state = Some(r.state);
        self.try_persist(host, &[job]);
        self.run(host, r.effects);
        Ok(())
    }

    /// Put a Job's cards on the boards, off the UI thread; what could not be
    /// placed reaches the Founder when it is known.
    fn place_cards(&mut self, host: &mut dyn Host, job: &Job) {
        let Some(s) = &self.state else { return };
        let Some(plan) = &job.plan else { return };
        let company = s.company.clone();
        for floor in &company.floors {
            let cards: Vec<PlanCard> = plan.cards.iter().filter(|c| c.floor == floor.id).cloned().collect();
            if cards.is_empty() {
                continue;
            }
            let change = BoardChange::Place { job: job.clone(), cards, company: company.clone() };
            let label = format!("Placing {}'s cards on {}", job.id, floor.id);
            self.begin(host, Op::ChangeBoard { floor: floor.id.clone(), dir: floor.dir.clone(), change }, format!("board-{}-{}", job.id, floor.id), label);
        }
    }

    /// Start the new hires, and anyone a plan reuses whose floor was closed. Gives
    /// back who did not start: only a seat started here can have failed to.
    fn start_hires(&mut self, host: &mut dyn Host, hired: &[super::super::types::Person]) -> Vec<String> {
        let mut failed = Vec::new();
        for p in hired {
            if host.seat(&p.id).is_some() {
                continue;
            }
            match self.ensure_person(host, p) {
                Ok(()) => {
                    if let Some(e) = host.seat(&p.id).and_then(|s| s.error) {
                        failed.push(format!("{} did not start: {e}.", p.name));
                    }
                }
                Err(e) => failed.push(format!("{} did not start: {e}.", p.name)),
            }
        }
        failed
    }

    /// The owner approves a plan: hire, place the cards, start people. Whatever
    /// fails along the way, the Job runs, and the Founder hears what failed.
    pub fn approve(&mut self, host: &mut dyn Host, id: &str) -> Result<(), String> {
        self.running()?;
        let Some(s) = &self.state else { return Ok(()) };
        if s.jobs.iter().find(|j| j.id == id).map(|j| &j.status) != Some(&JobStatus::AwaitingPlan) {
            return Ok(());
        }
        let seats = host.seat_names();
        let r = approve_plan(s, id, |taken: &[String]| name_for_new("agent", &[taken, &seats[..]].concat()), || format!("p{}", uid("")));
        self.state = Some(r.state);
        let Some(job) = self.state.as_ref().and_then(|s| s.jobs.iter().find(|j| j.id == id)).cloned() else { return Ok(()) };
        let mut failed = Vec::new();
        if let Err(e) = self.persist(std::slice::from_ref(&job)) {
            failed.push(format!("{} could not be saved: {e}.", job.id));
        }
        if !r.saved.is_empty() {
            let op = Op::SaveRoles { root: self.root.clone(), roles: r.saved.clone() };
            self.begin(host, op, format!("roles-{id}"), "Saving the new roles".into());
        }
        self.refresh_files(host, &r.rewired, true);
        self.place_cards(host, &job);
        let starting: Vec<_> = r.hired.iter().chain(r.reused.iter()).cloned().collect();
        failed.extend(self.start_hires(host, &starting));
        self.run(host, report_failures(r.effects, &failed));
        self.take_done(host);
        Ok(())
    }

    /// The owner sends a plan back to the Founder with a note.
    pub fn send_back(&mut self, host: &mut dyn Host, id: &str, note: &str) -> Result<(), String> {
        self.running()?;
        let Some(s) = &self.state else { return Ok(()) };
        let r = reject_plan(s, id, note);
        if &r.state == s {
            return Ok(());
        }
        self.state = Some(r.state);
        let jobs = self.jobs_of(id);
        self.try_persist(host, &jobs);
        self.run(host, r.effects);
        Ok(())
    }

    fn jobs_of(&self, id: &str) -> Vec<Job> {
        self.state.as_ref().map(|s| s.jobs.iter().filter(|j| j.id == id).cloned().collect()).unwrap_or_default()
    }

    /// Floors with people on them: only those can hold a Job's card.
    pub(crate) fn floors_with_people(&self) -> Vec<Floor> {
        let Some(s) = &self.state else { return Vec::new() };
        s.company.floors.iter().filter(|f| s.company.people.iter().any(|p| p.floor.as_deref() == Some(&f.id))).cloned().collect()
    }

    /// The owner ends a Job that is still going, for good.
    pub fn stop_job(&mut self, host: &mut dyn Host, id: &str) -> Result<(), String> {
        self.running()?;
        let Some(s) = &self.state else { return Ok(()) };
        let next = halt_job(s, id);
        if &next == s {
            return Ok(());
        }
        self.state = Some(next);
        self.alerts.retain(|a| a.job.as_deref() != Some(id)); // the watch would only drop them on its next round
        let jobs = self.jobs_of(id);
        self.try_persist(host, &jobs);
        let turns = self.state.as_ref().map(|s| stop_turns(s, id)).unwrap_or_default();
        for p in turns {
            self.interrupt(host, &p);
        }
        for f in self.floors_with_people() {
            let change = BoardChange::MarkStopped { job: id.to_string() };
            let label = format!("Marking {id}'s cards on {}", f.id);
            self.begin(host, Op::ChangeBoard { floor: f.id.clone(), dir: f.dir.clone(), change }, format!("board-{id}-{}", f.id), label);
        }
        self.run(host, vec![stopped_note(id)]);
        self.take_done(host);
        Ok(())
    }

    /// Give a paused Job more tokens and let it carry on.
    pub fn raise_cap(&mut self, host: &mut dyn Host, id: &str, extra: u64) -> Result<(), String> {
        self.running()?;
        let Some(s) = &self.state else { return Ok(()) };
        if extra == 0 {
            return Ok(());
        }
        let next = resume_job(s, id, extra);
        if &next == s {
            return Ok(());
        }
        self.state = Some(next);
        let jobs = self.jobs_of(id);
        self.try_persist(host, &jobs);
        let notes = self.state.as_ref().map(|s| resumed_notes(s, id, |p| host.seat(p).is_some_and(|x| x.running))).unwrap_or_default();
        self.run(host, notes);
        Ok(())
    }

    /// The owner has merged what a Job made: close it.
    pub fn finish_job(&mut self, host: &mut dyn Host, id: &str) -> Result<(), String> {
        self.running()?;
        let Some(s) = &self.state else { return Ok(()) };
        let next = close_job(s, id);
        if &next == s {
            return Ok(());
        }
        self.state = Some(next);
        let jobs = self.jobs_of(id);
        self.try_persist(host, &jobs);
        Ok(())
    }
}
