//! A whole company in memory, for looking at the page without an engine or a
//! fleet (office/fixtures.ts demoState and demoSource). A debug build with
//! BYTEHIVE_COMPANY_DEMO=1 shows it; it goes through office_model like the
//! real one, so the demo cannot drift from the model.

use std::collections::BTreeMap;

use crate::company::alerts::{Alert, Level};
use crate::company::types::{Company, FOUNDER_ID, Floor, Hire, Job, JobPlan, JobStatus, Look, Person, PlanCard, Rank, RoleDef};
use serde_json::Map;
use supervise::ask::{Ask, AskKind};
use supervise::task_state::{TaskState, TaskStatus};

use super::model::{OfficeModel, Pane, office_model};

/// The demo is asked for, in a debug build only.
pub(super) fn wanted() -> bool {
    cfg!(debug_assertions) && std::env::var("BYTEHIVE_COMPANY_DEMO").is_ok_and(|v| v == "1")
}

fn role(id: &str, title: &str, department: &str, shirt: &str, prop: &str, rank: Rank) -> RoleDef {
    RoleDef {
        id: id.into(),
        title: title.into(),
        department: department.into(),
        rank,
        cli: "claude".into(),
        deny_tools: vec![],
        look: Look { shirt: shirt.into(), hair: "#2a2e3a".into(), prop: prop.into(), ..Default::default() },
        prompt: String::new(),
        skills: vec![],
    }
}

fn roles() -> Vec<RoleDef> {
    vec![
        role("founder", "Founder", "Management", "#f5b83d", "none", Rank::Founder),
        role("lead", "Team Lead", "Management", "#5ec2f0", "clipboard", Rank::Lead),
        role("developer", "Developer", "Engineering", "#5fd08a", "laptop", Rank::Staff),
        role("designer", "Designer", "Design", "#ff6b6b", "pencil", Rank::Staff),
        role("qa", "QA", "Engineering", "#b18cf0", "magnifier", Rank::Staff),
        role("copywriter", "Copywriter", "Marketing", "#e6c84a", "pen", Rank::Staff),
        role("marketing", "Marketing", "Marketing", "#e0913a", "megaphone", Rank::Staff),
        role("researcher", "Researcher", "Research", "#3ad29f", "book", Rank::Staff),
    ]
}

const FLOORS: [&str; 5] = ["bytehive", "gcare", "vanta", "oldsite", "scratch"];
const CLOSED: [&str; 1] = ["oldsite"];

fn dir_of(floor: &str) -> String {
    format!("D:\\demo\\{floor}")
}

fn hire(name: &str, role_id: &str, floor: &str, roles: &[RoleDef]) -> Person {
    let r = roles.iter().find(|r| r.id == role_id).expect("a demo role");
    Person {
        id: name.to_lowercase(),
        name: name.into(),
        role: role_id.into(),
        rank: r.rank.clone(),
        floor: Some(floor.into()),
        boss: Some(FOUNDER_ID.into()),
        cli: "claude".into(),
        look: r.look.clone(),
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    }
}

fn pane(id: &str, state: TaskState, title: &str) -> Pane {
    let status = TaskStatus { state, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None };
    Pane { person_id: id.into(), status, doing: (!title.is_empty()).then(|| title.to_string()), nothing_to_merge: false, printing: state == TaskState::Working }
}

fn job(id: &str, title: &str, status: JobStatus, used: u64, people: &[&str]) -> Job {
    let mut j: Job = serde_json::from_str(&format!(r#"{{"id":"{id}","status":"running","people":[]}}"#)).expect("a demo job");
    j.title = title.into();
    j.ask = title.into();
    j.status = status;
    j.token_cap = 5_000_000;
    j.tokens_used = used;
    j.people = people.iter().map(|p| p.to_string()).collect();
    j.created_at = 1;
    j
}

fn plan() -> JobPlan {
    let card = |floor: &str, title: &str| PlanCard { floor: floor.into(), title: title.into(), desc: String::new(), checklist: vec![], assignee: None, extra: Map::new() };
    JobPlan {
        summary: "Split the work between the two teams.".into(),
        cards: vec![card("bytehive", "Add the price table"), card("bytehive", "Link each plan to sign-up"), card("gcare", "Write the pricing FAQ")],
        hires: vec![Hire { role: "designer".into(), floor: "gcare".into() }],
        token_cap: None,
        new_roles: None,
        reuse: None,
        extra: Map::new(),
    }
}

/// The demo company, as the page's actions change it.
pub(in crate::inbox) struct Demo {
    company: Company,
    jobs: Vec<Job>,
    alerts: Vec<Alert>,
    panes: Vec<Pane>,
    roles: Vec<RoleDef>,
    open: Vec<String>,
    cards: BTreeMap<String, (u32, u32)>,
}

impl Demo {
    pub(super) fn new() -> Self {
        let roles = roles();
        let staff = vec![
            hire("Hai", "lead", "bytehive", &roles),
            hire("Linh", "developer", "bytehive", &roles),
            hire("An", "designer", "bytehive", &roles),
            hire("Minh", "qa", "bytehive", &roles),
            hire("Mai", "lead", "gcare", &roles),
            hire("Tuan", "copywriter", "gcare", &roles),
            hire("Sol", "researcher", "vanta", &roles),
            hire("Bao", "marketing", "oldsite", &roles),
        ];
        let mut hai = pane("hai", TaskState::Needs, "Publish the package");
        hai.status.ask = Some(Ask { kind: AskKind::Run, prompt: String::new(), title: None, detail: Some("npm publish".into()), options: vec![], free_text_option: None });
        let panes = vec![
            pane(FOUNDER_ID, TaskState::Working, "Planning J-0142"),
            hai,
            pane("linh", TaskState::Working, "Checkout errors"),
            pane("an", TaskState::Working, "New icons"),
            pane("minh", TaskState::Review, "Test the refund path"),
            pane("mai", TaskState::Idle, ""),
            pane("tuan", TaskState::Working, "Launch post"),
            pane("sol", TaskState::Stopped, ""),
        ];
        let mut pricing = job("J-0142", "Add a pricing page", JobStatus::AwaitingPlan, 40_000, &[]);
        pricing.plan = Some(plan());
        let mut login = job("J-0138", "Fix the login page", JobStatus::AwaitingMerge, 1_200_000, &["linh"]);
        login.summary = Some("Login works on mobile again.".into());
        let jobs = vec![
            pricing,
            job("J-0141", "Ship the checkout", JobStatus::Running, 1_200_000, &["linh", "an"]),
            job("J-0139", "Rewrite the docs", JobStatus::Paused, 5_000_000, &["tuan"]),
            login,
        ];
        let alerts = vec![
            Alert { key: "stall-tuan".into(), job: Some("J-0141".into()), person_id: Some("tuan".into()), text: "Stuck: Tuan has not moved \u{201c}Launch post\u{201d}.".into(), level: Level::Warn, standing: false },
            Alert { key: "job-file-J-0137.json".into(), job: None, person_id: None, text: "The job file J-0137.json could not be read. It was left in place: mend it and restart ByteHive.".into(), level: Level::Warn, standing: true },
        ];
        let mut company = Company::empty();
        company.floors = FLOORS.iter().map(|id| Floor { id: id.to_string(), dir: dir_of(id), name: id.to_string(), extra: Map::new() }).collect();
        company.people.extend(staff);
        company.next_job = 143;
        let open = FLOORS.iter().filter(|f| !CLOSED.contains(f)).map(|f| dir_of(f)).collect();
        let cards = BTreeMap::from([("J-0141".to_string(), (2, 5)), ("J-0139".to_string(), (1, 3))]);
        Demo { company, jobs, alerts, panes, roles, open, cards }
    }

    pub(super) fn model(&self) -> OfficeModel {
        office_model(&self.company, &self.jobs, &self.alerts, &self.panes, &self.roles, &self.cards, &self.open)
    }

    fn update(&mut self, id: &str, f: impl FnOnce(&mut Job)) {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            f(j);
        }
    }

    pub(super) fn give_job(&mut self, ask: &str) {
        let id = format!("J-{:04}", self.company.next_job);
        self.company.next_job += 1;
        self.jobs.insert(0, job(&id, ask.trim(), JobStatus::Planning, 0, &[]));
    }

    pub(super) fn approve(&mut self, id: &str) {
        self.update(id, |j| j.status = JobStatus::Running);
    }

    pub(super) fn send_back(&mut self, id: &str, note: &str) {
        self.update(id, |j| {
            j.status = JobStatus::Rejected;
            j.summary = Some(note.into());
        });
    }

    pub(super) fn raise_cap(&mut self, id: &str, by: u64) {
        self.update(id, |j| {
            j.status = JobStatus::Running;
            j.token_cap += by;
        });
    }

    pub(super) fn finish(&mut self, id: &str) {
        self.update(id, |j| j.status = JobStatus::Done);
    }

    pub(super) fn stop(&mut self, id: &str) {
        self.update(id, |j| j.status = JobStatus::Stopped);
    }

    pub(super) fn dismiss(&mut self, key: &str) {
        self.alerts.retain(|a| a.key != key);
    }
}
