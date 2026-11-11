//! The office as plain data (model.ts officeModel): the rail, the jobs and the
//! floors read this and nothing else, so what they show cannot drift from the
//! company, and all of it can be tested. No 3D: no seats, papers or flights.

use crate::company::alerts::Alert;
use crate::company::needs::{NeedKind, NeedRow, Names, needs_you};
use crate::company::types::{Company, Job, JobStatus, Person, Rank, RoleDef};
use std::collections::BTreeMap;
use supervise::task_state::{TaskState, TaskStatus};

use super::words::{RESUME_DETAIL, clip, going, resume_title, status_line};

/// The one place a person's state comes from. Stopped covers no seat and a
/// merged agent: the chair is empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfficeState {
    Needs,
    Working,
    Review,
    Idle,
    Stopped,
}

pub(super) fn office_state(status: Option<&TaskStatus>) -> OfficeState {
    match status.map(|s| s.state) {
        Some(TaskState::Needs) => OfficeState::Needs,
        Some(TaskState::Working) => OfficeState::Working,
        Some(TaskState::Review) => OfficeState::Review,
        Some(TaskState::Idle) => OfficeState::Idle,
        _ => OfficeState::Stopped,
    }
}

/// A person's seat in the fleet, as the app sees it now.
#[derive(Clone, Debug)]
pub(super) struct Pane {
    pub person_id: String,
    pub status: TaskStatus,
    /// What their seat is on, its task's title.
    pub doing: Option<String>,
    /// Their branch holds nothing a merge would bring in: no commit ahead of the
    /// base and nothing uncommitted (a Lead who only hands out cards, a QA who reports).
    pub nothing_to_merge: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OfficePerson {
    pub id: String,
    pub name: String,
    pub rank: Rank,
    pub title: String,
    pub state: OfficeState,
    pub doing: Option<String>,
    pub floor_id: Option<String>,
    /// Their seat is in the fleet: their desk can be opened.
    pub seated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OfficeFloor {
    pub id: String,
    pub name: String,
    pub dir: String,
    /// Its project is open in the app.
    pub open: bool,
    /// Open, with someone at a seat.
    pub lit: bool,
    pub people: Vec<OfficePerson>,
    /// People here someone is waiting on.
    pub needs: usize,
}

/// `cards` is how far along it is, from the boards: None until its cards are on one.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct OfficeJob {
    pub id: String,
    pub title: String,
    pub status: JobStatus,
    pub label: &'static str,
    pub tokens_used: u64,
    pub token_cap: u64,
    pub cards: Option<(u32, u32)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Counts {
    pub people: u64,
    pub working: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OfficeModel {
    pub founder: Option<OfficePerson>,
    pub floors: Vec<OfficeFloor>,
    pub needs: Vec<NeedRow>,
    pub jobs: Vec<OfficeJob>,
    pub counts: Counts,
}

impl OfficeModel {
    pub(super) fn everyone(&self) -> impl Iterator<Item = &OfficePerson> {
        self.founder.iter().chain(self.floors.iter().flat_map(|f| f.people.iter()))
    }

    pub(super) fn person(&self, id: &str) -> Option<&OfficePerson> {
        self.everyone().find(|p| p.id == id)
    }

    /// A person's role, and their floor when they have one: "Developer, gcare".
    pub(super) fn role_of(&self, p: &OfficePerson) -> String {
        match p.floor_id.as_ref().and_then(|id| self.floors.iter().find(|f| &f.id == id)) {
            Some(f) => format!("{}, {}", p.title, f.name),
            None => p.title.clone(),
        }
    }
}

/// "data-analyst" as "Data Analyst", for a role no file names.
fn title_case(id: &str) -> String {
    id.split('-').filter(|w| !w.is_empty()).map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ")
}

/// The Job a person is on. The Founder is on the one they plan, else the newest running;
/// anyone else on a running Job that counts them in.
fn job_for<'a>(jobs: &'a [Job], p: &Person) -> Option<&'a Job> {
    if p.rank == Rank::Founder {
        let planning = jobs.iter().find(|j| j.status == JobStatus::Planning);
        return planning.or_else(|| jobs.iter().rev().find(|j| j.status == JobStatus::Running));
    }
    jobs.iter().rev().find(|j| j.status == JobStatus::Running && j.people.contains(&p.id))
}

fn to_person(p: &Person, role: Option<&RoleDef>, pane: Option<&Pane>, jobs: &[Job]) -> OfficePerson {
    let title = role.map_or_else(|| title_case(&p.role), |r| r.title.clone());
    // A seat is named after its role, which the line already says: name the Job instead.
    let seat = pane.and_then(|x| x.doing.clone()).filter(|d| *d != title);
    OfficePerson {
        id: p.id.clone(),
        name: p.name.clone(),
        rank: p.rank.clone(),
        state: office_state(pane.map(|x| &x.status)),
        doing: job_for(jobs, p).map(|j| clip(&j.title)).or(seat),
        title,
        floor_id: p.floor.clone(),
        seated: pane.is_some(),
    }
}

/// What a person is asking shows in Needs you: they run with skip permissions, so it is rare, and worth a line.
fn ask_row(p: &OfficePerson, status: &TaskStatus) -> NeedRow {
    NeedRow {
        key: format!("ask-{}", p.id),
        kind: NeedKind::Alert,
        job_id: None,
        person_id: Some(p.id.clone()),
        title: status_line(&p.name, status),
        detail: String::new(),
        standing: false,
        plan: None,
    }
}

/// `open_dirs`: the projects open in the app; a floor with nobody on it shows only while open.
pub(super) fn office_model(
    company: &Company,
    jobs: &[Job],
    alerts: &[Alert],
    panes: &[Pane],
    roles: &[RoleDef],
    job_cards: &BTreeMap<String, (u32, u32)>,
    open_dirs: &[String],
) -> OfficeModel {
    let role_of = |id: &str| roles.iter().find(|r| r.id == id);
    let pane_of = |id: &str| panes.iter().find(|p| p.person_id == id);
    let open = |dir: &str| open_dirs.iter().any(|d| crate::presets::same_folder(d, dir));

    let founder = company.people.iter().find(|p| p.rank == Rank::Founder).map(|p| to_person(p, role_of(&p.role), pane_of(&p.id), jobs));
    let names = Names { floors: &company.floors, roles };
    // Someone with nothing to merge is as done as someone merged: the walk never
    // sends the owner to a Review that says there is nothing to review.
    let merged = |id: &str| pane_of(id).is_some_and(|p| p.status.state == TaskState::Merged || p.nothing_to_merge);
    let rows = needs_you(jobs, alerts, &names, merged);

    let floors: Vec<OfficeFloor> = company
        .floors
        .iter()
        .map(|f| {
            let here: Vec<&Person> = company.people.iter().filter(|p| p.floor.as_deref() == Some(f.id.as_str())).collect();
            // The Lead sits first.
            let seated = here.iter().filter(|p| p.rank == Rank::Lead).chain(here.iter().filter(|p| p.rank != Rank::Lead));
            let people: Vec<OfficePerson> = seated.map(|p| to_person(p, role_of(&p.role), pane_of(&p.id), jobs)).collect();
            let mut wanted: Vec<&str> = people.iter().filter(|p| p.state == OfficeState::Needs).map(|p| p.id.as_str()).collect();
            for r in &rows {
                if let Some(id) = r.person_id.as_deref().filter(|id| people.iter().any(|p| p.id == *id)) {
                    wanted.push(id);
                }
            }
            wanted.sort_unstable();
            wanted.dedup();
            let open = open(&f.dir);
            let lit = open && people.iter().any(|p| p.state != OfficeState::Stopped);
            OfficeFloor { id: f.id.clone(), name: f.name.clone(), dir: f.dir.clone(), open, lit, needs: wanted.len(), people }
        })
        .filter(|f| !f.people.is_empty() || f.open)
        .collect();

    let everyone: Vec<&OfficePerson> = founder.iter().chain(floors.iter().flat_map(|f| f.people.iter())).collect();
    let asks: Vec<NeedRow> = everyone
        .iter()
        .filter(|p| p.state == OfficeState::Needs)
        .filter_map(|p| pane_of(&p.id).map(|pane| ask_row(p, &pane.status)))
        .collect();
    // A Job still running with all its people stopped (ByteHive closed under them)
    // goes nowhere until they start again: say so, with the one button that does it.
    let stopped = |id: &String| pane_of(id).is_none_or(|p| office_state(Some(&p.status)) == OfficeState::Stopped);
    let resumes = jobs.iter().filter(|j| j.status == JobStatus::Running && !j.people.is_empty() && j.people.iter().all(stopped)).map(|j| NeedRow {
        key: format!("resume-{}", j.id),
        kind: NeedKind::Alert,
        job_id: Some(j.id.clone()),
        person_id: None,
        title: resume_title(&j.title),
        detail: RESUME_DETAIL.into(),
        standing: false,
        plan: None,
    });
    let asks: Vec<NeedRow> = asks.into_iter().chain(resumes).collect();
    // A person's question goes before the merges: it is waiting on an answer now.
    let at = rows.iter().position(|r| r.kind == NeedKind::Merge).unwrap_or(rows.len());
    let mut needs = rows[..at].to_vec();
    needs.extend(asks);
    needs.extend_from_slice(&rows[at..]);

    let counts = Counts { people: everyone.len() as u64, working: everyone.iter().filter(|p| p.state == OfficeState::Working).count() as u64 };
    let jobs = jobs
        .iter()
        .filter_map(|j| {
            let label = going(&j.status)?;
            Some(OfficeJob {
                id: j.id.clone(),
                title: j.title.clone(),
                status: j.status.clone(),
                label,
                tokens_used: j.tokens_used,
                token_cap: j.token_cap,
                cards: job_cards.get(&j.id).copied(),
            })
        })
        .collect();
    OfficeModel { founder, floors, needs, jobs, counts }
}
