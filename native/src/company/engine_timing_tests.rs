//! How long the engine's own file work holds the UI thread, at the size of a
//! real company: 3 floors, 8 people, two running Jobs with full boards. The
//! file parts of the host are real (roles, person files, board.json); seats are
//! the fake's. Run with `cargo test timing -- --ignored --nocapture`.

use super::harness::*;
use crate::company::fleet_host::write_board_file;
use crate::company::host::{Done, Op};
use crate::company::types::{Company, Person, RoleDef};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The fake host, with real files where the engine touches them. Slow work
/// waits in `pending` until `finish` runs it, outside what is timed.
struct Real {
    fake: Fake,
    mcp: PathBuf,
    pending: Vec<Op>,
    ready: Vec<Done>,
}

impl Real {
    /// Run the slow work started so far, as the background threads would.
    fn finish(&mut self) {
        for op in std::mem::take(&mut self.pending) {
            let done = match op {
                Op::NodeCheck => Done::Node(Ok(())),
                Op::Exclude { dir } => Done::Excluded { dir, result: Ok(()) },
                Op::Files { root, people, roles, company, report } => {
                    Done::Files { failed: crate::company::launch::refresh_files(&root, &people, &roles, &company, &self.mcp), report }
                }
                op => crate::company::drive::run_op(op),
            };
            self.ready.push(done);
        }
        self.ready.extend(self.fake.done());
    }
}

impl Host for Real {
    fn now_ms(&self) -> i64 {
        self.fake.now_ms()
    }
    fn pid(&self) -> u32 {
        self.fake.pid()
    }
    fn process_alive(&self, pid: u64) -> bool {
        self.fake.process_alive(pid)
    }
    fn projects(&self) -> Vec<Project> {
        self.fake.projects()
    }
    fn active_project(&self) -> Option<Project> {
        self.fake.active_project()
    }
    fn seat(&self, person: &str) -> Option<SeatView> {
        self.fake.seat(person)
    }
    fn seat_names(&self) -> Vec<String> {
        self.fake.seat_names()
    }
    fn write_person_files(&mut self, root: &Path, person: &Person, role: &RoleDef, company: &Company) -> Vec<String> {
        crate::company::launch::refresh_files(root, std::slice::from_ref(person), std::slice::from_ref(role), company, &self.mcp)
    }
    fn spawn_person(&mut self, root: &Path, person: &Person, role: &RoleDef, dir: &str, restore: &Restore) -> Result<(), String> {
        self.fake.spawn_person(root, person, role, dir, restore)
    }
    fn restart(&mut self, person: &str) {
        self.fake.restart(person)
    }
    fn type_message(&mut self, person: &str, text: &str) {
        self.fake.type_message(person, text)
    }
    fn interrupt(&mut self, person: &str) {
        self.fake.interrupt(person)
    }
    fn folder_exists(&self, dir: &str) -> bool {
        Path::new(dir).is_dir()
    }
    fn session_exists(&self, dir: &str, session: &str) -> bool {
        self.fake.session_exists(dir, session)
    }
    fn begin(&mut self, op: Op) {
        self.pending.push(op);
    }
    fn done(&mut self) -> Vec<Done> {
        std::mem::take(&mut self.ready)
    }
    fn log(&mut self, line: &str) {
        self.fake.log(line)
    }
}

use crate::company::host::Host;

fn person(id: &str, name: &str, role: &str, rank: Rank, floor: Option<&str>, boss: Option<&str>) -> serde_json::Value {
    json!({ "id": id, "name": name, "role": role, "rank": rank, "floor": floor, "boss": boss, "cli": "claude", "look": { "shirt": "#fff", "hair": "#000", "prop": "none" } })
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore]
fn timing_of_the_engines_file_work_on_the_ui_thread() {
    let root = tempfile::tempdir().unwrap();
    let floors: Vec<tempfile::TempDir> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let dirs: Vec<String> = floors.iter().map(|f| f.path().to_str().unwrap().to_string()).collect();
    let ids = ["a", "b", "c"];
    let mut people = vec![person("founder", "Founder", "founder", Rank::Founder, None, None)];
    let names = ["Ana", "Bob", "Cid", "Dot", "Eve", "Fin", "Gio"];
    for (i, name) in names.iter().enumerate() {
        let floor = ids[i % 3];
        let lead = i < 3;
        let boss = if lead { "founder".to_string() } else { format!("p{}", i % 3) };
        people.push(person(&format!("p{i}"), name, if lead { "lead" } else { "developer" }, if lead { Rank::Lead } else { Rank::Staff }, Some(floor), Some(&boss)));
    }
    let company = json!({
        "version": 1,
        "floors": ids.iter().zip(&dirs).map(|(id, d)| json!({ "id": id, "dir": d, "name": id })).collect::<Vec<_>>(),
        "people": people,
        "nextJob": 3
    });
    std::fs::write(root.path().join("company.json"), serde_json::to_string_pretty(&company).unwrap()).unwrap();
    std::fs::create_dir_all(root.path().join("jobs")).unwrap();
    for (n, id) in ["J-0001", "J-0002"].iter().enumerate() {
        let job = json!({ "id": id, "title": "t", "ask": "a", "status": "running", "plan": null, "tokenCap": 5_000_000, "tokensUsed": 0, "tokenBase": {}, "people": ["p0", "p1", "p2"], "createdAt": n });
        std::fs::write(root.path().join("jobs").join(format!("{id}.json")), job.to_string()).unwrap();
    }
    for d in &dirs {
        let mut b = crate::board::default_board();
        for i in 0..20 {
            let mut c = crate::board::mk_card(&format!("Card {i}"));
            c.job = Some(if i % 2 == 0 { "J-0001" } else { "J-0002" }.into());
            c.assignee_id = Some("p0".into());
            c.checklist = (0..4).map(|k| crate::board::ChecklistItem { id: format!("x{k}"), text: "step".into(), done: false }).collect();
            b.lists[i % 3].cards.push(c);
        }
        write_board_file(d, &b, None).unwrap();
    }
    let mcp = root.path().join("bytehive-mcp.mjs");
    std::fs::write(&mcp, "").unwrap();
    let mut fake = Fake::new();
    fake.projects = ids.iter().zip(&dirs).map(|(id, d)| Project { name: id.to_string(), dir: Some(d.clone()) }).collect();
    let mut host = Real { fake, mcp, pending: Vec::new(), ready: Vec::new() };

    let t = Instant::now();
    let mut e = Engine::start(root.path(), &mut host);
    let start = ms(t);
    host.finish();
    let t = Instant::now();
    e.tick(&mut host);
    let loaded = ms(t);
    assert!(e.state().is_some(), "{:?}", e.error());
    host.finish();
    let t = Instant::now();
    e.tick(&mut host);
    let files = ms(t);

    // A request: a message between two people, as agents send all day.
    let line = json!({ "type": "message", "ts": 1, "from": "p3", "to": "Ana", "message": "done" });
    std::fs::write(root.path().join("requests.jsonl"), format!("{line}\n")).unwrap();
    host.fake.now += 1500;
    let t = Instant::now();
    e.tick(&mut host);
    let request = ms(t);

    // A Job waits for approval with one card on each floor and a hire.
    e.give_job(&mut host, "more").unwrap();
    let plan = JobPlan { cards: ids.iter().map(|f| plan_for(f, 0).cards[0].clone()).collect(), ..plan_for("a", 1) };
    let plan_line = json!({ "type": "plan", "ts": 2, "from": "founder", "job": "J-0003", "plan": plan });
    std::fs::write(root.path().join("requests.jsonl"), format!("{line}\n{plan_line}\n")).unwrap();
    host.fake.now += 1500;
    e.tick(&mut host);
    host.finish();
    e.tick(&mut host);
    assert_eq!(e.state().unwrap().jobs[2].status, JobStatus::AwaitingPlan, "{:?}", host.fake.logs);
    let t = Instant::now();
    e.approve(&mut host, "J-0003").unwrap();
    let approve = ms(t);
    assert_eq!(e.state().unwrap().jobs[2].status, JobStatus::Running);
    host.finish();
    let t = Instant::now();
    e.tick(&mut host);
    let placed = ms(t);

    // The watch: every floor's board read off the UI thread, then the Jobs measured.
    host.fake.now += 30_000;
    let t = Instant::now();
    e.tick(&mut host);
    let watch_begin = ms(t);
    host.finish();
    let t = Instant::now();
    e.tick(&mut host);
    let watched = ms(t);

    // A tick with nothing to do.
    host.fake.now += 1500;
    let t = Instant::now();
    e.tick(&mut host);
    let idle = ms(t);

    eprintln!("TIMING start {start:.1}, loaded {loaded:.1}, files applied {files:.1}, request {request:.1}, approve {approve:.1}, placed {placed:.1}, watch {watch_begin:.1} + {watched:.1}, idle {idle:.1} (ms)");
}
