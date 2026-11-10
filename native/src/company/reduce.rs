//! Everything that changes the company, as plain functions: state in, state and
//! effects out (reduce.ts). The engine runs the effects. The app re-checks every
//! rule here, because a request is only a line in a file. The words are the
//! oracle's, word for word: the agents and the owner read them.

use super::org::{apply_hires, can_message, from_draft, person_by_id, person_by_name, plan_cap, reuse_staff, validate_plan};
use super::types::{Company, CompanyRequest, FOUNDER_ID, Job, JobPlan, JobStatus, Person, Rank, RoleDef};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub company: Company,
    pub jobs: Vec<Job>,
    pub roles: Vec<RoleDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Deliver { person_id: String, text: String },
    /// Shown under Needs you: the owner is no terminal.
    Owner { key: String, text: String },
}

/// What a change gives back. A state equal to the one passed in means nothing
/// changed, so an idle company writes nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Reduced {
    pub state: State,
    pub effects: Vec<Effect>,
}

/// What an approval did, for the engine to carry out.
#[derive(Clone, Debug, PartialEq)]
pub struct Approved {
    pub state: State,
    pub hired: Vec<Person>,
    pub reused: Vec<Person>,
    pub rewired: Vec<Person>,
    pub saved: Vec<RoleDef>,
    pub effects: Vec<Effect>,
}

/// What a person's pane runs on. None is a field the pane no longer has.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    pub session: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
}

fn job_id(n: u64) -> String {
    format!("J-{n:04}")
}

fn job<'a>(s: &'a State, id: &str) -> Option<&'a Job> {
    s.jobs.iter().find(|j| j.id == id)
}

fn status_of(s: &State, id: &str) -> Option<JobStatus> {
    job(s, id).map(|j| j.status.clone())
}

fn patch(s: &State, id: &str, change: impl FnOnce(&mut Job)) -> State {
    let mut next = s.clone();
    if let Some(j) = next.jobs.iter_mut().find(|j| j.id == id) {
        change(j);
    }
    next
}

fn tell_founder(text: String) -> Effect {
    Effect::Deliver { person_id: FOUNDER_ID.into(), text }
}

/// A message ends up typed into a terminal: an escape or a carriage return in it
/// would be a keypress there, not text. Line breaks and tabs are still text.
pub fn typeable(text: &str) -> String {
    text.chars().filter(|c| !matches!(*c as u32, 0x00..=0x08 | 0x0b..=0x1f | 0x7f..=0x9f)).collect()
}

/// A new Job from the owner's ask, and the Founder woken to plan it.
pub fn new_job(s: &State, ask: &str, now: u64) -> (Reduced, Job) {
    let job = Job {
        id: job_id(s.company.next_job),
        title: ask.trim().split('\n').next().unwrap_or_default().trim().to_string(),
        ask: ask.to_string(),
        status: JobStatus::Planning,
        plan: None,
        token_cap: s.company.limits.tokens_per_job,
        tokens_used: 0,
        token_base: BTreeMap::new(),
        people: Vec::new(),
        summary: None,
        created_at: now,
        extra: Default::default(),
    };
    let mut state = s.clone();
    state.company.next_job += 1;
    state.jobs.push(job.clone());
    let effects = vec![tell_founder(format!("JOB {}: {ask}", job.id))];
    (Reduced { state, effects }, job)
}

/// The paused or stopped Job that keeps these two from talking. A pause ends, so it
/// holds everyone on it. A stop doesn't and a message names no Job, so someone on a
/// running Job as well stays reachable, or one stop would cut a Lead off for good.
fn holding_job<'a>(s: &'a State, from: &str, to: &str) -> Option<&'a Job> {
    let on_running = |id: &str| s.jobs.iter().any(|j| j.status == JobStatus::Running && j.people.iter().any(|p| p == id));
    s.jobs.iter().find(|j| {
        let on = j.people.iter().any(|p| p == from || p == to);
        on && (j.status == JobStatus::Paused || (j.status == JobStatus::Stopped && !on_running(from) && !on_running(to)))
    })
}

pub fn apply_request(s: &State, r: &CompanyRequest) -> Reduced {
    let none = || Reduced { state: s.clone(), effects: vec![] };
    let from_id = match r {
        CompanyRequest::Message { from, .. } | CompanyRequest::Plan { from, .. } | CompanyRequest::JobDone { from, .. } => from,
    };
    let Some(from) = person_by_id(&s.company, from_id) else { return none() };
    if let CompanyRequest::Message { to, message, .. } = r {
        let to = to.as_deref().filter(|t| !t.is_empty()).and_then(|t| person_by_name(&s.company, t));
        let Some(to) = to.filter(|to| can_message(&s.company, &from.id, &to.id)) else { return none() };
        // Nothing reaches or leaves the people of a paused or stopped Job: the message is dropped, and the sender is told so.
        let (person_id, text) = match holding_job(s, &from.id, &to.id) {
            Some(held) if held.status == JobStatus::Paused => (
                from.id.clone(),
                format!("Held: your job {} is paused, so your message to {} was not sent. Wait for it to be resumed.", held.id, to.name),
            ),
            Some(held) => (
                from.id.clone(),
                format!("Held: your job {} is stopped, so your message to {} was not sent. That job will not run again.", held.id, to.name),
            ),
            None => (to.id.clone(), format!("[From {}] {}", from.name, typeable(message))),
        };
        return Reduced { state: s.clone(), effects: vec![Effect::Deliver { person_id, text }] };
    }
    if from.rank != Rank::Founder {
        return none();
    }
    let refuse = |text: String| Reduced { state: s.clone(), effects: vec![tell_founder(text)] };
    match r {
        CompanyRequest::Plan { job: id, plan, .. } => {
            let Some(job) = job(s, id) else { return none() };
            if job.status != JobStatus::Planning {
                return refuse(format!("Plan for {id} refused: the job is {}, and a plan is only taken while it is planning.", job.status.as_str()));
            }
            // A role the floor already has is that person's work, not a new hire's. Who that is, is
            // the app's to say: a reuse list in the request is dropped.
            let plan = reuse_staff(&s.company, &JobPlan { reuse: None, ..plan.clone() });
            let errors = validate_plan(&s.company, &plan, &s.roles);
            if !errors.is_empty() {
                return refuse(format!("Plan for {id} refused: {}", errors.join(" ")));
            }
            let reuse = plan.reuse.as_deref().unwrap_or_default();
            let effects = match reuse.len() {
                0 => vec![],
                n => vec![tell_founder(format!(
                    "Plan for {id} taken without {n} of its hires: {} already there and {} that work.",
                    staff_on(s, reuse),
                    if n == 1 { "does" } else { "do" }
                ))],
            };
            Reduced { state: patch(s, id, |j| (j.status, j.plan) = (JobStatus::AwaitingPlan, Some(plan))), effects }
        }
        CompanyRequest::JobDone { job: id, summary, .. } => {
            let Some(job) = job(s, id) else { return none() };
            // Nothing was planned, so nobody worked and there is nothing to merge. A plan the owner is reading still has to be answered.
            if job.status == JobStatus::Planning {
                let state = patch(s, id, |j| (j.status, j.summary) = (JobStatus::Done, Some(summary.clone())));
                let effects = vec![Effect::Owner { key: format!("no-work-{id}"), text: format!("{id} needed no work: {summary}") }];
                return Reduced { state, effects };
            }
            if job.status != JobStatus::Running {
                return refuse(format!(
                    "{id} cannot be finished: it is {}, and only a running job, or a planning one that needs no work, can.",
                    job.status.as_str()
                ));
            }
            Reduced { state: patch(s, id, |j| (j.status, j.summary) = (JobStatus::AwaitingMerge, Some(summary.clone()))), effects: vec![] }
        }
        CompanyRequest::Message { .. } => none(),
    }
}

/// The people a plan reuses, by name, role and floor.
fn staff_on(s: &State, ids: &[String]) -> String {
    let held: Vec<&Person> = ids.iter().filter_map(|id| person_by_id(&s.company, id)).collect();
    let names: Vec<String> = held.iter().map(|p| format!("{} ({} on {})", p.name, p.role, p.floor.as_deref().unwrap_or_default())).collect();
    names.join(", ") + if held.len() == 1 { " is" } else { " are" }
}

/// The owner approves a waiting plan: drafted roles are saved, the hires made, and
/// the Job runs. `name` picks a name nobody has, `id` a new person id.
pub fn approve_plan(s: &State, id: &str, name: impl FnMut(&[String]) -> String, pid: impl FnMut() -> String) -> Approved {
    let unchanged = |state: State, effects: Vec<Effect>| Approved { state, hired: vec![], reused: vec![], rewired: vec![], saved: vec![], effects };
    let Some((job, plan)) = job(s, id).and_then(|j| Some((j, j.plan.as_ref()?))) else { return unchanged(s.clone(), vec![]) };
    if job.status != JobStatus::AwaitingPlan {
        return unchanged(s.clone(), vec![]);
    }
    // It passed on arrival, but the company may have changed while it waited.
    let errors = validate_plan(&s.company, plan, &s.roles);
    if !errors.is_empty() {
        let state = patch(s, id, |j| (j.status, j.plan) = (JobStatus::Planning, None));
        return unchanged(state, vec![tell_founder(format!("Plan for {id} no longer holds: {} Make a new plan with job_plan.", errors.join(" ")))]);
    }
    // The owner has read them now: from here a drafted role is a role.
    let saved: Vec<RoleDef> = plan.new_roles.iter().flatten().map(from_draft).collect();
    let roles: Vec<RoleDef> = s.roles.iter().chain(&saved).cloned().collect();
    let (company, hired) = apply_hires(&s.company, plan, &roles, name, pid);
    let names: Vec<String> = hired.iter().map(|p| format!("{} ({})", p.name, p.role)).collect();
    // Whoever took a hire's place works this Job, and starts with the hires if their floor is closed.
    let reused: Vec<Person> = plan.reuse.iter().flatten().filter_map(|p| person_by_id(&company, p).cloned()).collect();
    // Whoever a card names works this Job as much as a new hire does, and so does
    // the Lead of its floor, who hands it out and reviews it. A floor with no Lead
    // has nobody to hand it out: its people are briefed directly, and a stop on an
    // older Job would cut them off if this one left them out.
    let on_cards = plan.cards.iter().flat_map(|c| {
        let named = c.assignee.as_deref().and_then(|a| person_by_name(&company, a));
        let floor: Vec<&Person> = company.people.iter().filter(|p| p.floor.as_deref() == Some(c.floor.as_str())).collect();
        let briefed = match floor.iter().find(|p| p.rank == Rank::Lead) {
            Some(lead) => vec![*lead],
            None => floor,
        };
        named.into_iter().chain(briefed).map(|p| p.id.clone()).collect::<Vec<_>>()
    });
    let mut people: Vec<String> = Vec::new();
    for p in job.people.iter().cloned().chain(hired.iter().map(|p| p.id.clone())).chain(reused.iter().map(|p| p.id.clone())).chain(on_cards) {
        if !people.contains(&p) {
            people.push(p);
        }
    }
    let cap = plan_cap(plan, job.token_cap); // the owner's limit, never the Founder's
    let state = patch(&State { company: company.clone(), roles, ..s.clone() }, id, |j| {
        (j.status, j.token_cap, j.people) = (JobStatus::Running, cap, people);
    });
    // A new Lead takes over the floor. Whoever was there keeps a session that says
    // otherwise, and would refuse the Lead's orders until they hear it.
    let rewired: Vec<Person> = company.people.iter().filter(|p| person_by_id(&s.company, &p.id).is_some_and(|was| was.boss != p.boss)).cloned().collect();
    let mut effects: Vec<Effect> = rewired
        .iter()
        .map(|p| {
            // A rewired person's boss is the new Lead, so a boss not on the roster can't happen; it reads as nobody.
            let boss = p.boss.as_deref().and_then(|b| person_by_id(&company, b)).map_or("nobody", |b| b.name.as_str());
            Effect::Deliver {
                person_id: p.id.clone(),
                text: format!("[From Founder] You have a new boss: {boss}. Take your cards from them and report to them from now on."),
            }
        })
        .collect();
    let hired_words = if names.is_empty() { String::new() } else { format!(" Hired: {}.", names.join(", ")) };
    effects.push(tell_founder(format!(
        "Plan for {id} approved.{hired_words} The cards are on the boards. Brief the Lead of each floor, or the person working alone there."
    )));
    Approved { state, hired, reused, rewired, saved, effects }
}

/// Write down what a person's pane runs on. Gives back the same state when nothing moved.
pub fn note_person(s: &State, id: &str, seen: &Seen) -> State {
    let Some(p) = person_by_id(&s.company, id) else { return s.clone() };
    if p.session == seen.session && p.worktree == seen.worktree && p.branch == seen.branch {
        return s.clone();
    }
    let mut next = s.clone();
    if let Some(p) = next.company.people.iter_mut().find(|p| p.id == id) {
        (p.session, p.worktree, p.branch) = (seen.session.clone(), seen.worktree.clone(), seen.branch.clone());
    }
    next
}

/// Add someone who picked up a card of a running Job.
pub fn join_job(s: &State, id: &str, person_id: &str) -> State {
    match job(s, id) {
        Some(j) if j.status == JobStatus::Running && !j.people.iter().any(|p| p == person_id) => patch(s, id, |j| j.people.push(person_id.into())),
        _ => s.clone(),
    }
}

/// Stop a running Job, usually because it reached its token cap.
pub fn pause_job(s: &State, id: &str) -> State {
    if status_of(s, id) != Some(JobStatus::Running) {
        return s.clone();
    }
    patch(s, id, |j| j.status = JobStatus::Paused)
}

const GOING: [JobStatus; 4] = [JobStatus::Planning, JobStatus::AwaitingPlan, JobStatus::Running, JobStatus::Paused];

/// The owner ends a Job that is still going, planned or not. There is no way back: a new Job is.
pub fn halt_job(s: &State, id: &str) -> State {
    match status_of(s, id) {
        Some(status) if GOING.contains(&status) => patch(s, id, |j| j.status = JobStatus::Stopped),
        _ => s.clone(),
    }
}

/// Whose running turn a Stop ends: people on this Job with no other Job going. The
/// Founder works every Job, so it counts as long as no other is going.
pub fn stop_turns(s: &State, id: &str) -> Vec<String> {
    let Some(job) = job(s, id) else { return vec![] };
    let others: Vec<&Job> = s.jobs.iter().filter(|j| j.id != id && GOING.contains(&j.status)).collect();
    let busy = |p: &String| others.iter().any(|j| j.people.contains(p));
    let people: Vec<String> = job.people.iter().filter(|p| !busy(p)).cloned().collect();
    if !others.is_empty() {
        return people.into_iter().filter(|p| p != FOUNDER_ID).collect();
    }
    let mut all: Vec<String> = Vec::new();
    for p in people.into_iter().chain([FOUNDER_ID.to_string()]) {
        if !all.contains(&p) {
            all.push(p);
        }
    }
    all
}

pub fn stopped_note(id: &str) -> Effect {
    tell_founder(format!("{id} was stopped by the owner. It takes no more messages and cannot be finished. Leave it, and do not brief anyone on it."))
}

/// A pause is the owner's to lift, and the Founder would otherwise only see its effects.
pub fn paused_note(id: &str) -> Effect {
    tell_founder(format!(
        "{id} reached its token cap and is paused until the owner adds tokens. Its messages are held and it cannot be finished. Wait, you will be told."
    ))
}

/// What was held while a Job was paused is gone, and nobody knows it runs again.
/// Someone stopped is left out: a delivery would start them just to hear it.
pub fn resumed_notes(s: &State, id: &str, is_up: impl Fn(&str) -> bool) -> Vec<Effect> {
    let Some(job) = job(s, id) else { return vec![] };
    let mut notes = vec![tell_founder(format!("{id} is running again with more tokens. If its work is finished, call job_finish now."))];
    notes.extend(job.people.iter().filter(|p| is_up(p)).map(|p| Effect::Deliver {
        person_id: p.clone(),
        text: format!("{id} is running again. Anything you sent while it was paused was not delivered: send it again if it still matters."),
    }));
    notes
}

/// Carry on a paused Job, with `extra` more tokens on its cap.
pub fn resume_job(s: &State, id: &str, extra: u64) -> State {
    if status_of(s, id) != Some(JobStatus::Paused) {
        return s.clone();
    }
    patch(s, id, |j| (j.status, j.token_cap) = (JobStatus::Running, j.token_cap + extra))
}

/// The owner has reviewed and merged what a Job made: it is done. Only a Job
/// that is waiting for that can be closed.
pub fn close_job(s: &State, id: &str) -> State {
    if status_of(s, id) != Some(JobStatus::AwaitingMerge) {
        return s.clone();
    }
    patch(s, id, |j| j.status = JobStatus::Done)
}

/// Add what went wrong while carrying out a plan to what the Founder is told.
pub fn report_failures(effects: Vec<Effect>, failed: &[String]) -> Vec<Effect> {
    if failed.is_empty() {
        return effects;
    }
    let note = format!(" But this went wrong: {}", failed.join(" "));
    effects
        .into_iter()
        .map(|e| match e {
            Effect::Deliver { person_id, text } if person_id == FOUNDER_ID => Effect::Deliver { person_id, text: text + &note },
            e => e,
        })
        .collect()
}

pub fn reject_plan(s: &State, id: &str, note: &str) -> Reduced {
    if status_of(s, id) != Some(JobStatus::AwaitingPlan) {
        return Reduced { state: s.clone(), effects: vec![] };
    }
    let why = note.trim();
    if why.is_empty() {
        return Reduced { state: patch(s, id, |j| (j.status, j.plan) = (JobStatus::Rejected, None)), effects: vec![] };
    }
    Reduced {
        state: patch(s, id, |j| (j.status, j.plan) = (JobStatus::Planning, None)),
        effects: vec![tell_founder(format!("Plan for {id} sent back: {why} Make a new plan with job_plan."))],
    }
}

#[cfg(test)]
#[path = "reduce_tests.rs"]
mod tests;
