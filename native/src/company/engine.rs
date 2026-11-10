//! Runs the company (engine.ts): keeps the lock, reads requests, applies the
//! rules, starts people, types messages into their terminals. Every decision
//! lives in reduce.rs and watch.rs; every outside effect goes through Host.
//! Calls come one at a time through `&mut self`, which is the TS queue.

use super::alerts::{Alert, Standing};
use super::host::{Host, Loaded, Op};
use super::lock::{self, EngineLock, LOCK_STALE_MS, LockMove, LockRead};
use super::reduce::State;
use super::requests::Drain;
use super::store::{self, job_file_alert};
use super::types::{Company, Job, RoleDef};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[path = "engine_ops.rs"]
mod ops;
#[path = "engine_owner.rs"]
mod owner;
#[path = "engine_people.rs"]
mod people;
#[path = "engine_watch.rs"]
mod watching;

pub use people::Pending;
pub(crate) use owner::change_board_with;

#[cfg(test)]
#[path = "engine_fake.rs"]
pub(crate) mod fake;
#[cfg(test)]
#[path = "engine_floor_tests.rs"]
mod floor_tests;
#[cfg(test)]
#[path = "engine_harness.rs"]
mod harness;
#[cfg(test)]
#[path = "engine_lock_tests.rs"]
mod lock_tests;
#[cfg(test)]
#[path = "engine_ops_tests.rs"]
mod ops_tests;
#[cfg(test)]
#[path = "engine_people_tests.rs"]
mod people_tests;
#[cfg(test)]
#[path = "engine_restore_tests.rs"]
mod restore_tests;
#[cfg(test)]
#[path = "engine_stop_tests.rs"]
mod stop_tests;
#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "engine_timing_tests.rs"]
mod timing_tests;
#[cfg(test)]
#[path = "engine_watch_tests.rs"]
mod watch_tests;

const REQUESTS: &str = "requests.jsonl";
pub(crate) const WATCH_MS: i64 = 30_000;
pub(crate) const NO_PROJECT: &str = "Open a project first.";
pub(crate) const FOUNDER_WAITS: &str = "founder-no-project";
const ELSEWHERE: &str = "The Company is running in another ByteHive window.";

/// A Job's cards on the boards: how many are done, of how many.
pub type JobCards = (u32, u32);

/// What the Company surface shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompanyView {
    /// None while another app runs the company, or it could not be loaded.
    pub company: Option<Company>,
    pub jobs: Vec<Job>,
    pub roles: Vec<RoleDef>,
    /// The standing alerts and the watch's, what Needs you takes.
    pub alerts: Vec<Alert>,
    /// Job id to its cards (done, total) on the boards the watch read; a Job with
    /// no card on any board is absent.
    pub job_cards: BTreeMap<String, JobCards>,
    /// Another running ByteHive holds the lock.
    pub locked_by_other: bool,
    /// Why the company could not be loaded.
    pub error: Option<String>,
}

pub struct Engine {
    root: PathBuf,
    /// This app runs the company: only then is there a state, and only then is
    /// anything read or written.
    holding: bool,
    /// Another running app does, as far as the lock says.
    elsewhere: bool,
    /// When the last heartbeat ran, and when this app last woke or saw its clock jump.
    last_beat: i64,
    awake_since: i64,
    last_watch: i64,
    state: Option<State>,
    load_error: Option<String>,
    standing: Standing,
    /// What the last watch raised.
    alerts: Vec<Alert>,
    nudges: HashMap<String, u32>,
    /// When each watched card went quiet, by list and id.
    quiet: HashMap<String, i64>,
    requests: Drain,
    /// Folders whose git has been told about .bytehive/ in this run, lowercased.
    excluded: HashSet<String>,
    /// Messages waiting for their person to reach the prompt.
    outbox: Vec<Pending>,
    /// Each floor's last good tally of cards by Job, by floor id.
    floor_cards: HashMap<String, BTreeMap<String, JobCards>>,
    /// What the last Node check said; None until it has said anything.
    node: Option<Result<(), String>>,
    /// Slow work under way off the UI thread.
    ops: Vec<ops::Running>,
    /// Goes up on every change the screen should follow.
    pub changes: u64,
    /// What was handed to someone, typed at once (false) or waiting (true).
    #[cfg(test)]
    pub said: Vec<(String, String, bool)>,
}

/// Two spellings of one folder.
pub(crate) fn same_folder(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

impl Engine {
    /// Take charge of the company at `root` when nobody else runs it. Either way
    /// the heartbeat goes on in `tick`, and takes over when the lock frees.
    pub fn start(root: &Path, host: &mut dyn Host) -> Engine {
        let now = host.now_ms();
        let mut e = Engine {
            root: root.to_path_buf(),
            holding: false,
            elsewhere: false,
            last_beat: now,
            awake_since: 0,
            last_watch: now,
            state: None,
            load_error: None,
            standing: Standing::default(),
            alerts: Vec::new(),
            nudges: HashMap::new(),
            quiet: HashMap::new(),
            requests: Drain::default(),
            excluded: HashSet::new(),
            outbox: Vec::new(),
            floor_cards: HashMap::new(),
            node: None,
            ops: Vec::new(),
            changes: 0,
            #[cfg(test)]
            said: Vec::new(),
        };
        let _ = std::fs::create_dir_all(root);
        e.heartbeat(host);
        e
    }

    pub(crate) fn changed(&mut self) {
        self.changes += 1;
    }

    /// Every 1.5 s: keep the lock, read and carry out requests, type what waits,
    /// and every 30 s watch the work.
    pub fn tick(&mut self, host: &mut dyn Host) {
        self.heartbeat(host);
        if !self.holding {
            return;
        }
        self.take_done(host);
        if self.state.is_none() {
            return;
        }
        self.read_requests(host);
        self.send_waiting(host);
        let now = host.now_ms();
        // A clock moved back would hold the watch off for as long as it moved.
        if now < self.last_watch {
            self.last_watch = now;
        }
        if now - self.last_watch >= WATCH_MS {
            self.last_watch = now;
            self.watch(host);
        }
        self.take_done(host);
    }

    /// Who runs the company.
    fn heartbeat(&mut self, host: &mut dyn Host) {
        let read = lock::read(&self.root);
        let now = host.now_ms();
        let me = host.pid();
        // A beat comes every 1.5 s: a gap this long, either way, is a sleep or a moved clock.
        if now.saturating_sub(self.last_beat).unsigned_abs() >= LOCK_STALE_MS as u64 {
            self.awake_since = now;
        }
        self.last_beat = now;
        let silent = match read {
            LockRead::Held(l) if lock::lock_silent(&l, me, now) => Some(l),
            _ => None,
        };
        // A lock with no real pid (lock::NO_PID) belongs to no process: process_alive says no.
        let alive = silent.is_some_and(|l| host.process_alive(l.pid));
        match lock::lock_move(&read, me, now, alive, self.awake_since) {
            LockMove::Skip => {}
            LockMove::StayOut => {
                let news = !self.elsewhere;
                self.elsewhere = true;
                if self.holding {
                    self.step_down();
                } else if news {
                    self.changed();
                }
            }
            moved => {
                let _ = lock::write(&self.root, &EngineLock { pid: u64::from(me), at: now });
                // Two apps can write it at the same moment: the one whose lock stayed runs.
                if !lock::won(&lock::read(&self.root), me) {
                    self.elsewhere = true;
                    if self.holding {
                        self.step_down();
                    }
                    return;
                }
                self.elsewhere = false;
                if moved == LockMove::TakeOver {
                    let pid = silent.map_or(0, |l| l.pid);
                    self.standing.raise(
                        "lock-takeover",
                        &format!("Another ByteHive app (process {pid}) stopped answering for 10 minutes, so this one took over the Company. If that app is still open, close it: the people it started are still running, next to the ones started here."),
                        None,
                    );
                }
                if !self.holding {
                    self.holding = true;
                    self.load(host);
                }
            }
        }
    }

    /// Another app holds the lock: let go of everything, the files are its now.
    fn step_down(&mut self) {
        self.holding = false;
        self.state = None;
        self.alerts.clear();
        self.nudges.clear();
        self.quiet.clear();
        self.outbox.clear();
        self.floor_cards.clear();
        self.requests.reset();
        self.changed();
    }

    /// Load the company and take charge of it, off the UI thread (`loaded`).
    fn load(&mut self, host: &mut dyn Host) {
        self.load_error = None;
        self.begin(host, Op::Load { root: self.root.clone() }, "load".into(), "Loading the company".into());
    }

    /// The company is read. Nothing loaded means nothing writes: a file that
    /// could not be read stays as it is for you to mend.
    pub(crate) fn loaded(&mut self, host: &mut dyn Host, read: Result<Loaded, String>) {
        if !self.holding || self.state.is_some() {
            return;
        }
        let l = match read {
            Ok(l) => l,
            Err(e) => {
                host.log(&format!("the company could not be loaded: {e}"));
                self.load_error = Some(e);
                self.changed();
                return;
            }
        };
        for name in &l.unreadable {
            let (key, text) = job_file_alert(name);
            self.standing.raise(&key, &text, None);
        }
        self.state = Some(State { company: l.company, jobs: l.jobs, roles: l.roles });
        // Every member's MCP server runs on Node: asked once, off the UI thread.
        self.begin(host, Op::NodeCheck, "node".into(), "Checking Node.js".into());
        self.sync_floors(host);
        if self.state.as_ref().is_some_and(|s| !s.jobs.is_empty()) {
            self.exclude_floors(host);
        }
        if let Some(s) = &self.state {
            if let Err(e) = store::save_company(&self.root, &s.company) {
                host.log(&format!("the company could not be saved: {e:#}"));
            }
        }
        // A seat restarts on the files it was given: after an update those are old laws.
        let people = self.state.as_ref().map(|s| s.company.people.clone()).unwrap_or_default();
        self.refresh_files(host, &people, false);
        self.changed();
    }

    /// Save, and carry on if the disk says no: what a request asked for must still happen.
    pub(crate) fn try_persist(&mut self, host: &mut dyn Host, jobs: &[Job]) {
        if let Err(e) = self.persist(jobs) {
            host.log(&format!("the company could not be saved: {e}"));
        }
    }

    pub(crate) fn persist(&mut self, jobs: &[Job]) -> Result<(), String> {
        self.changed(); // the screen follows memory, saved or not
        let Some(s) = &self.state else { return Ok(()) };
        store::save_company(&self.root, &s.company).map_err(|e| format!("{e:#}"))?;
        for j in jobs {
            store::save_job(&self.root, j).map_err(|e| format!("{e:#}"))?;
        }
        Ok(())
    }

    /// Read and carry out whatever has come in.
    fn read_requests(&mut self, host: &mut dyn Host) {
        let lines = self.requests.lines(&self.root.join(REQUESTS));
        for line in lines {
            // These lines are read only once: one that breaks something never costs the rest.
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.apply_line(host, &line)));
            if ran.is_err() {
                host.log(&format!("a request went wrong and was skipped: {line}"));
            }
        }
    }

    pub fn alerts(&self) -> Vec<Alert> {
        self.standing.list().iter().chain(self.alerts.iter()).cloned().collect()
    }

    pub fn dismiss(&mut self, key: &str) {
        if self.standing.clear(key) {
            self.changed();
        }
    }

    /// Why the company could not be loaded, when it could not (the view carries it too).
    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// The company as this app holds it; None beside another app.
    #[cfg(test)]
    pub fn state(&self) -> Option<&State> {
        self.state.as_ref()
    }

    #[cfg(test)]
    pub fn state_mut(&mut self) -> &mut State {
        self.state.as_mut().expect("the company is loaded")
    }

    /// A floor someone from the company works on: an open project with nobody
    /// on it is still just your project. For the Conductor, which does not ask yet.
    #[allow(dead_code)]
    pub fn is_company_floor(&self, dir: &str) -> bool {
        let Some(s) = &self.state else { return false };
        s.company.floors.iter().find(|f| same_folder(&f.dir, dir)).is_some_and(|f| s.company.people.iter().any(|p| p.floor.as_deref() == Some(&f.id)))
    }

    /// Ok while this app runs the company; else why an owner action did nothing.
    pub(crate) fn running(&self) -> Result<(), String> {
        if self.state.is_some() {
            return Ok(());
        }
        if self.elsewhere {
            return Err(ELSEWHERE.into());
        }
        Err(self.load_error.clone().unwrap_or_else(|| "The Company is still loading.".into()))
    }

    pub fn view(&self) -> CompanyView {
        let alerts = self.alerts();
        let mut job_cards: BTreeMap<String, JobCards> = BTreeMap::new();
        for floor in self.floor_cards.values() {
            for (job, (done, total)) in floor {
                let t = job_cards.entry(job.clone()).or_default();
                t.0 += done;
                t.1 += total;
            }
        }
        let base = CompanyView { alerts, job_cards, locked_by_other: self.elsewhere, error: self.load_error.clone(), ..CompanyView::default() };
        let Some(s) = &self.state else { return base };
        CompanyView { company: Some(s.company.clone()), jobs: s.jobs.clone(), roles: s.roles.clone(), ..base }
    }
}
