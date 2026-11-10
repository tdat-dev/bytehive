//! The company's rules, with no UI and no IO (org.ts). The bundled MCP server
//! holds the agents to the same rules in TS, so the words here are the oracle's,
//! word for word: the Founder reads them.

use super::types::{Company, FOUNDER_ID, Hire, JobPlan, Look, NewRole, Person, Rank, RoleDef};
use std::collections::{HashMap, HashSet, VecDeque};

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// A role id becomes a folder name, so the app and the MCP tool hold it to this.
/// Windows keeps some names for devices, and a folder can't have them.
pub fn valid_role_id(id: &str) -> bool {
    let b = id.as_bytes();
    let shape = (2..=31).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-');
    let device = matches!(id, "con" | "prn" | "aux" | "nul")
        || (b.len() == 4 && (id.starts_with("com") || id.starts_with("lpt")) && (b'1'..=b'9').contains(&b[3]));
    shape && !device
}

/// A drafted role as the app will run it: staff on Claude with nothing denied, whatever
/// the plan's JSON says. Only the four fields of a draft are the Founder's to set.
pub fn from_draft(d: &NewRole) -> RoleDef {
    RoleDef {
        id: d.id.clone(),
        title: d.title.clone(),
        department: d.department.clone(),
        prompt: d.prompt.clone(),
        rank: Rank::Staff,
        cli: "claude".into(),
        deny_tools: Vec::new(),
        look: Look { shirt: "#7d8b99".into(), hair: "#2a2e3a".into(), prop: "none".into(), edited: None, ..Default::default() },
        skills: Vec::new(),
    }
}

/// The cap a plan runs under: its own, inside the owner's limit, never above it.
pub fn plan_cap(plan: &JobPlan, limit: u64) -> u64 {
    plan.token_cap.unwrap_or(limit).min(limit)
}

/// A floor's id, from its project's name: two projects can share a name.
pub fn floor_id(name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            slug.push(ch);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = match slug.trim_matches('-') {
        "" => "floor".to_string(),
        s => s.to_string(),
    };
    let mut id = slug.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{slug}-{n}");
        n += 1;
    }
    id
}

pub fn person_by_id<'a>(c: &'a Company, id: &str) -> Option<&'a Person> {
    c.people.iter().find(|p| p.id == id)
}

pub fn person_by_name<'a>(c: &'a Company, name: &str) -> Option<&'a Person> {
    c.people.iter().find(|p| same(&p.name, name))
}

pub fn reports_of<'a>(c: &'a Company, id: &str) -> Vec<&'a Person> {
    c.people.iter().filter(|p| p.boss.as_deref() == Some(id)).collect()
}

fn boss_of<'a>(c: &'a Company, p: &Person) -> Option<&'a Person> {
    p.boss.as_deref().and_then(|b| person_by_id(c, b))
}

/// A person talks to their boss and to their own reports, nobody else.
pub fn can_message(c: &Company, from_id: &str, to_id: &str) -> bool {
    let (Some(from), Some(to)) = (person_by_id(c, from_id), person_by_id(c, to_id)) else {
        return false;
    };
    if from.id == to.id {
        return false;
    }
    from.boss.as_deref() == Some(&to.id) || to.boss.as_deref() == Some(&from.id)
}

/// Who a message goes to: `to` by name, or with no name the boss, else the reports.
pub fn resolve_targets(c: &Company, from_id: &str, to: Option<&str>) -> Result<Vec<Person>, String> {
    let Some(from) = person_by_id(c, from_id) else {
        return Err(format!("You are not on the company roster ({from_id})."));
    };
    let Some(to) = to else {
        let people: Vec<Person> = match boss_of(c, from) {
            Some(boss) => vec![boss.clone()],
            None => reports_of(c, &from.id).into_iter().cloned().collect(),
        };
        if people.is_empty() {
            return Err("You have nobody to message yet: no boss and no reports.".into());
        }
        return Ok(people);
    };
    let Some(target) = person_by_name(c, to) else {
        return Err(format!("Nobody named {to} works here. See company_status."));
    };
    if can_message(c, &from.id, &target.id) {
        return Ok(vec![target.clone()]);
    }
    let via = boss_of(c, from).map_or("your reports", |b| b.name.as_str());
    Err(format!("You cannot message {} directly. Send it to {via} and they will pass it on.", target.name))
}

fn floor_people<'a>(c: &'a Company, floor: &str) -> Vec<&'a Person> {
    c.people.iter().filter(|p| p.floor.as_deref() == Some(floor)).collect()
}

/// Who speaks for a floor: its Lead, else the one person there.
pub fn floor_contact<'a>(c: &'a Company, floor: &str) -> Option<&'a Person> {
    let here = floor_people(c, floor);
    here.iter().find(|p| p.rank == Rank::Lead).or(here.first()).copied()
}

/// What is wrong with the roles a plan drafts. They run with skip permissions, so
/// nothing here can be left to the Founder.
fn draft_errors(drafts: &[NewRole], roles: &[RoleDef]) -> Vec<String> {
    let mut errors = Vec::new();
    for (i, d) in drafts.iter().enumerate() {
        let id = &d.id;
        if !valid_role_id(id) {
            errors.push(format!("Role {id} refused: an id is 2 to 31 lowercase letters, digits or dashes, starting with a letter, and not a Windows device name."));
        } else if id == FOUNDER_ID {
            errors.push("Role founder refused: the company has one.".into());
        } else if roles.iter().any(|r| &r.id == id) {
            errors.push(format!("Role {id} refused: that role exists already. Use it, or pick another id."));
        } else if drafts.iter().position(|x| &x.id == id) != Some(i) {
            errors.push(format!("Role {id} is in the plan twice."));
        }
        for (field, value) in [("title", &d.title), ("department", &d.department), ("prompt", &d.prompt)] {
            if value.trim().is_empty() {
                errors.push(format!("Role {id} has no {field}."));
            }
        }
    }
    errors
}

/// A plan that hires a role a floor already has: the person there takes the work, and only
/// the hires beyond them stay. Each one on a floor covers one hire of their role, and is named
/// in `reuse`, so they start with the hires. A drafted role nobody is hired into any more goes.
pub fn reuse_staff(c: &Company, plan: &JobPlan) -> JobPlan {
    let mut free: HashMap<(&str, &str), VecDeque<&str>> = HashMap::new();
    for p in &c.people {
        if let Some(floor) = p.floor.as_deref().filter(|f| !f.is_empty()) {
            free.entry((floor, p.role.as_str())).or_default().push_back(&p.id);
        }
    }
    let mut reuse: Vec<String> = Vec::new();
    let hires: Vec<Hire> = plan
        .hires
        .iter()
        .filter(|h| match free.get_mut(&(h.floor.as_str(), h.role.as_str())).and_then(VecDeque::pop_front) {
            Some(who) => {
                reuse.push(who.to_string());
                false
            }
            None => true,
        })
        .cloned()
        .collect();
    if reuse.is_empty() {
        return plan.clone();
    }
    let hired: HashSet<&str> = hires.iter().map(|h| h.role.as_str()).collect();
    let dropped: HashSet<&str> = plan.hires.iter().map(|h| h.role.as_str()).filter(|r| !hired.contains(r)).collect();
    let new_roles = plan.new_roles.as_ref().map(|list| list.iter().filter(|r| !dropped.contains(r.id.as_str())).cloned().collect());
    JobPlan { hires, reuse: Some(reuse), new_roles, ..plan.clone() }
}

/// Everything wrong with a plan, in the words the Founder reads; empty when it holds.
pub fn validate_plan(c: &Company, plan: &JobPlan, roles: &[RoleDef]) -> Vec<String> {
    let drafts = plan.new_roles.as_deref().unwrap_or_default();
    let mut errors = draft_errors(drafts, roles);
    let mut known: Vec<&str> = Vec::new();
    for f in &c.floors {
        if !known.contains(&f.id.as_str()) {
            known.push(&f.id);
        }
    }
    let all: Vec<RoleDef> = roles.iter().cloned().chain(drafts.iter().map(from_draft)).collect();
    let role_of = |id: &str| all.iter().find(|r| r.id == id);
    let is_lead = |id: &str| role_of(id).is_some_and(|r| r.rank == Rank::Lead);
    let limits = &c.limits;
    if plan.hires.len() > limits.people_per_job as usize {
        errors.push(format!("A plan may hire at most {} people.", limits.people_per_job));
    }
    if c.people.len() + plan.hires.len() > limits.people_total as usize {
        errors.push(format!("The company is limited to {} people.", limits.people_total));
    }
    for h in &plan.hires {
        if !known.contains(&h.floor.as_str()) {
            errors.push(format!("Unknown floor {}.", h.floor));
        }
        if h.role == FOUNDER_ID || role_of(&h.role).is_some_and(|r| r.rank == Rank::Founder) {
            errors.push("A plan cannot hire a Founder: the company has one.".into());
        } else if role_of(&h.role).is_none() {
            errors.push(format!("Unknown role {}. See role_list, or draft it in newRoles in this plan.", h.role));
        }
    }
    for card in &plan.cards {
        if !known.contains(&card.floor.as_str()) {
            errors.push(format!("Card \"{}\" is on unknown floor {}.", card.title, card.floor));
        } else if floor_people(c, &card.floor).is_empty() && !plan.hires.iter().any(|h| h.floor == card.floor) {
            errors.push(format!("Card \"{}\" is on floor {}, where nobody works: hire someone there.", card.title, card.floor));
        }
    }
    for floor in known {
        let hires: Vec<&Hire> = plan.hires.iter().filter(|h| h.floor == floor).collect();
        if hires.is_empty() {
            continue;
        }
        let now = floor_people(c, floor);
        let leads = now.iter().filter(|p| p.rank == Rank::Lead).count() + hires.iter().filter(|h| is_lead(&h.role)).count();
        let total = now.len() + hires.len();
        if total >= 2 && leads == 0 {
            errors.push(format!("Floor {floor} will have {total} people and needs a Lead: add a hire with role lead."));
        }
        if leads > 1 {
            errors.push(format!("Floor {floor} can only have one Lead."));
        }
        if total - leads > limits.people_per_lead as usize {
            errors.push(format!("A Lead can have at most {} people.", limits.people_per_lead));
        }
    }
    errors
}

/// The company with a plan's hires in it, and who they are. Leads come first and take
/// their floor; staff go under their floor's Lead, else the Founder. `name` picks a name
/// nobody has, `id` a new person id.
pub fn apply_hires(
    c: &Company,
    plan: &JobPlan,
    roles: &[RoleDef],
    mut name: impl FnMut(&[String]) -> String,
    mut id: impl FnMut() -> String,
) -> (Company, Vec<Person>) {
    let mut people = c.people.clone();
    // Indexes into people, so a hire rewired after it was added is given back rewired.
    let mut hired: Vec<usize> = Vec::new();
    let role_of = |rid: &str| roles.iter().find(|r| r.id == rid);
    let is_lead = |rid: &str| role_of(rid).is_some_and(|r| r.rank == Rank::Lead);
    let mut add = |people: &mut Vec<Person>, h: &Hire, boss: String| -> Option<String> {
        // validate_plan has checked the role; one that went missing is no hire.
        let role = role_of(&h.role)?;
        let pid = id();
        let taken: Vec<String> = people.iter().map(|p| p.name.clone()).collect();
        let person = Person {
            id: pid.clone(),
            name: name(&taken),
            role: role.id.clone(),
            rank: if role.rank == Rank::Lead { Rank::Lead } else { Rank::Staff },
            floor: Some(h.floor.clone()),
            boss: Some(boss),
            cli: role.cli.clone(),
            look: role.look.clone(),
            session: None,
            worktree: None,
            branch: None,
            extra: Default::default(),
        };
        people.push(person);
        hired.push(people.len() - 1);
        Some(pid)
    };
    for h in plan.hires.iter().filter(|h| is_lead(&h.role)) {
        let Some(lead) = add(&mut people, h, FOUNDER_ID.into()) else { continue };
        for p in people.iter_mut() {
            if p.floor.as_deref() == Some(h.floor.as_str()) && p.id != lead {
                p.boss = Some(lead.clone());
            }
        }
    }
    for h in plan.hires.iter().filter(|h| !is_lead(&h.role)) {
        let lead = people.iter().find(|p| p.floor.as_deref() == Some(h.floor.as_str()) && p.rank == Rank::Lead);
        let boss = lead.map_or_else(|| FOUNDER_ID.to_string(), |p| p.id.clone());
        add(&mut people, h, boss);
    }
    let hired = hired.iter().map(|&i| people[i].clone()).collect();
    (Company { people, ..c.clone() }, hired)
}

#[cfg(test)]
#[path = "org_tests.rs"]
mod tests;
