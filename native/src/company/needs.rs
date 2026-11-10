//! What the owner has to act on, in the order worth acting on it (needs.ts).

use super::alerts::{Alert, Level};
use super::org::plan_cap;
use super::types::{Floor, Job, JobPlan, JobStatus, NewRole, RoleDef};

/// What approving a plan comes to: hired people run with skip permissions, so the
/// owner reads all of it here, new roles word for word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanDetail {
    pub hires: Vec<String>,
    pub floors: Vec<FloorCards>,
    pub token_cap: u64,
    pub new_roles: Vec<NewRole>,
}

/// A floor's name and the titles of the plan's cards on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorCards {
    pub floor: String,
    pub cards: Vec<String>,
}

/// In the order the rows are listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NeedKind {
    Plan,
    Paused,
    Alert,
    Merge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedRow {
    pub key: String,
    pub kind: NeedKind,
    pub job_id: Option<String>,
    pub person_id: Option<String>,
    pub title: String,
    pub detail: String,
    /// An alert that stays until dismissed. False on every other row.
    pub standing: bool,
    pub plan: Option<PlanDetail>,
}

/// What turns ids into the names the owner knows.
pub struct Names<'a> {
    pub floors: &'a [Floor],
    pub roles: &'a [RoleDef],
}

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

fn plan_detail(plan: &JobPlan, limit: u64, names: &Names) -> PlanDetail {
    let floor_name = |id: &str| names.floors.iter().find(|f| f.id == id).map_or_else(|| id.to_string(), |f| f.name.clone());
    let new_roles = plan.new_roles.clone().unwrap_or_default();
    let role_title = |id: &str| {
        let known = names.roles.iter().map(|r| (&r.id, &r.title));
        let drafted = new_roles.iter().map(|r| (&r.id, &r.title));
        known.chain(drafted).find(|(rid, _)| *rid == id).map_or_else(|| id.to_string(), |(_, t)| t.clone())
    };
    let mut floors: Vec<&str> = Vec::new();
    for c in &plan.cards {
        if !floors.contains(&c.floor.as_str()) {
            floors.push(&c.floor);
        }
    }
    PlanDetail {
        hires: plan.hires.iter().map(|h| format!("{} on {}", role_title(&h.role), floor_name(&h.floor))).collect(),
        floors: floors
            .into_iter()
            .map(|f| FloorCards { floor: floor_name(f), cards: plan.cards.iter().filter(|c| c.floor == f).map(|c| c.title.clone()).collect() })
            .collect(),
        token_cap: plan_cap(plan, limit),
        new_roles,
    }
}

/// A Job with several people has a branch each: its one row takes the owner to
/// the next one still to merge, and says how far along the merging is.
fn merge_row(j: &Job, merged: &impl Fn(&str) -> bool) -> NeedRow {
    let summary = j.summary.as_deref().unwrap_or("");
    let n = j.people.len();
    let done = j.people.iter().filter(|p| merged(p)).count();
    let next = if n > 1 { j.people.iter().find(|p| !merged(p)) } else { j.people.first() };
    let left = if done == 0 {
        format!("{n} people to merge.")
    } else if done == n {
        format!("All {n} merged.")
    } else {
        format!("{done} of {n} merged.")
    };
    let detail = if n > 1 { [summary, left.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ") } else { summary.to_string() };
    NeedRow {
        key: format!("merge-{}", j.id),
        kind: NeedKind::Merge,
        job_id: Some(j.id.clone()),
        person_id: next.cloned(),
        title: format!("Review and merge: {}", j.title),
        detail,
        standing: false,
        plan: None,
    }
}

/// `merged` says whether a person's branch is merged already.
pub fn needs_you(jobs: &[Job], alerts: &[Alert], names: &Names, merged: impl Fn(&str) -> bool) -> Vec<NeedRow> {
    let mut rows = Vec::new();
    let row = |key: String, kind, j: &Job, title: String, detail: String| NeedRow {
        key,
        kind,
        job_id: Some(j.id.clone()),
        person_id: None,
        title,
        detail,
        standing: false,
        plan: None,
    };
    for j in jobs {
        if let (JobStatus::AwaitingPlan, Some(plan)) = (&j.status, &j.plan) {
            let detail = format!("{} {}, {}.", plan.summary, plural(plan.cards.len(), "card"), plural(plan.hires.len(), "hire"));
            let title = format!("Approve plan: {}", j.title);
            rows.push(NeedRow { plan: Some(plan_detail(plan, j.token_cap, names)), ..row(format!("plan-{}", j.id), NeedKind::Plan, j, title, detail) });
        }
        if j.status == JobStatus::Paused {
            rows.push(row(format!("paused-{}", j.id), NeedKind::Paused, j, format!("Paused: {}", j.title), "It reached its token cap.".into()));
        }
        if j.status == JobStatus::AwaitingMerge {
            rows.push(merge_row(j, &merged));
        }
    }
    for a in alerts {
        if a.level == Level::Stop {
            continue; // a paused Job already has its row
        }
        rows.push(NeedRow {
            key: a.key.clone(),
            kind: NeedKind::Alert,
            job_id: a.job.clone(),
            person_id: a.person_id.clone(),
            title: a.text.clone(),
            detail: String::new(),
            standing: a.standing,
            plan: None,
        });
    }
    rows.sort_by_key(|r| r.kind); // stable, as Array.prototype.sort
    rows
}

#[cfg(test)]
#[path = "needs_tests.rs"]
mod tests;
