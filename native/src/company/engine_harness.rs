//! The engine under test: a temp company folder, the fake host, and the TS
//! test helpers (waitingJob, staffFloors, request, said...).

pub use super::super::host::{BoardWrite, Host, Project, Restore, SeatView};
pub use super::super::types::{Hire, Job, JobPlan, JobStatus, Look, Person, PlanCard, Rank};
pub use super::Engine;
pub use super::fake::{A, B, Fake, project};
pub use crate::board::Card;
use serde_json::{Map, Value, json};
use std::path::Path;

pub const STEP: i64 = 1500;

pub struct T {
    pub dir: tempfile::TempDir,
    pub host: Fake,
    pub e: Engine,
}

pub fn plan_for(floor: &str, hires: usize) -> JobPlan {
    JobPlan {
        summary: "s".into(),
        cards: vec![PlanCard { floor: floor.into(), title: "Do it".into(), desc: "d".into(), checklist: Vec::new(), assignee: None, extra: Map::new() }],
        hires: (0..hires).map(|_| Hire { role: "developer".into(), floor: floor.into() }).collect(),
        token_cap: None,
        new_roles: None,
        reuse: None,
        extra: Map::new(),
    }
}

pub fn hiring(role: &str, floor: &str) -> JobPlan {
    JobPlan { cards: Vec::new(), hires: vec![Hire { role: role.into(), floor: floor.into() }], ..plan_for(floor, 0) }
}

impl T {
    pub fn new() -> T {
        T::with(Fake::new())
    }

    pub fn with(mut host: Fake) -> T {
        let dir = tempfile::tempdir().unwrap();
        let e = Engine::start(dir.path(), &mut host);
        let mut t = T { dir, host, e };
        t.advance(STEP); // the first tick notes that requests.jsonl isn't there yet
        t
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Time passes, a tick every 1.5 s.
    pub fn advance(&mut self, ms: i64) {
        let mut left = ms;
        while left >= STEP {
            self.host.now += STEP;
            left -= STEP;
            self.e.tick(&mut self.host);
        }
        self.host.now += left;
    }

    /// The app starts again on the same files, with no seats open.
    pub fn relaunch(&mut self) {
        self.host.seats.clear();
        self.e = Engine::start(self.dir.path(), &mut self.host);
        // The load runs off the UI thread: the next tick takes it in.
        self.e.tick(&mut self.host);
    }

    /// The MCP server appends a request, and a tick reads it.
    pub fn request(&mut self, line: Value) {
        let at = self.root().join("requests.jsonl");
        let old = std::fs::read_to_string(&at).unwrap_or_default();
        std::fs::write(&at, format!("{old}{line}\n")).unwrap();
        self.advance(STEP);
    }

    pub fn plan(&mut self, job: &str, plan: &JobPlan) {
        self.request(json!({ "type": "plan", "ts": 1, "from": "founder", "job": job, "plan": plan }));
    }

    pub fn message(&mut self, from: &str, to: &str, text: &str) {
        self.request(json!({ "type": "message", "ts": 1, "from": from, "to": to, "message": text }));
    }

    pub fn give(&mut self, ask: &str) -> Result<(), String> {
        self.e.give_job(&mut self.host, ask)
    }

    pub fn approve(&mut self, id: &str) {
        let _ = self.e.approve(&mut self.host, id);
    }

    pub fn job(&self, n: usize) -> Job {
        self.e.state().unwrap().jobs[n].clone()
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.e.state().map(|s| s.jobs.clone()).unwrap_or_default()
    }

    pub fn people(&self) -> Vec<Person> {
        self.e.state().unwrap().company.people.clone()
    }

    pub fn person(&self, f: impl Fn(&Person) -> bool) -> Person {
        self.people().into_iter().find(|p| f(p)).expect("a person")
    }

    /// Everything handed to someone, typed at once or waiting.
    pub fn said(&self) -> Vec<String> {
        self.e.said.iter().map(|(_, t, _)| t.clone()).collect()
    }

    pub fn said_all(&self) -> String {
        self.said().join(" ")
    }

    pub fn clear_said(&mut self) {
        self.e.said.clear();
        self.host.typed.clear();
    }

    /// A card needs someone on its floor: whoever the plan doesn't hire already works there.
    pub fn staff_floors(&mut self, plan: &JobPlan) {
        let c = &mut self.e.state_mut().company;
        let mut floors: Vec<&str> = plan.cards.iter().map(|x| x.floor.as_str()).collect();
        floors.dedup();
        for floor in floors {
            if plan.hires.iter().any(|h| h.floor == floor) || c.people.iter().any(|p| p.id == format!("r-{floor}")) {
                continue;
            }
            c.people.push(Person {
                id: format!("r-{floor}"),
                name: format!("Resident {floor}"),
                role: "developer".into(),
                rank: Rank::Staff,
                floor: Some(floor.into()),
                boss: Some("founder".into()),
                cli: "claude".into(),
                look: Look::default(),
                session: None,
                worktree: None,
                branch: None,
                extra: Map::new(),
            });
        }
    }

    /// Floors for D:\a and D:\b, one Job given and its plan in.
    pub fn waiting_job(&mut self, plan: JobPlan) {
        self.give("do it").unwrap();
        self.staff_floors(&plan);
        let id = self.job(0).id;
        self.plan(&id, &plan);
        assert_eq!(self.job(0).status, JobStatus::AwaitingPlan);
        self.clear_said();
    }

    pub fn cards(&self, dir: &str) -> Vec<Card> {
        self.host.boards.get(dir).map(|b| b.lists.iter().flat_map(|l| l.cards.clone()).collect()).unwrap_or_default()
    }

    pub fn list_mut(&mut self, dir: &str, title: &str) -> &mut Vec<Card> {
        &mut self.host.boards.get_mut(dir).unwrap().lists.iter_mut().find(|l| l.title == title).unwrap().cards
    }

    /// The first person with a seat who is not the Founder.
    pub fn member(&self) -> String {
        let mut ids: Vec<&String> = self.host.seats.keys().filter(|k| *k != "founder").collect();
        ids.sort();
        ids[0].clone()
    }

    /// A job's file as it is on disk.
    pub fn stored(&self, id: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.root().join("jobs").join(format!("{id}.json"))).unwrap()).unwrap()
    }

    pub fn on_disk(&self, person: &str) -> Value {
        let c: Value = serde_json::from_str(&std::fs::read_to_string(self.root().join("company.json")).unwrap()).unwrap();
        c["people"].as_array().unwrap().iter().find(|p| p["id"] == person).cloned().unwrap()
    }

    pub fn alert_keys(&self) -> Vec<String> {
        self.e.alerts().into_iter().map(|a| a.key).collect()
    }

    /// The Founder already up at its prompt in `dir`.
    pub fn founder_up(&mut self, dir: &str) {
        let now = self.host.now;
        self.host.add_seat("founder", "Founder", dir, SeatView { running: true, at_prompt: true, spawned_at: Some(now - 60_000), ..SeatView::default() });
    }
}
