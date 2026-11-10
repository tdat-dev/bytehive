//! Slow work runs off the UI thread (git, worktrees, Claude's config, Node):
//! the engine starts it, carries on, and takes in what came of it each tick.
//! Work that outlives OP_LIMIT_MS is reported once, as queue.ts did; its result
//! still counts when it comes.

use super::super::host::{BoardChange, Done, Host, Op};
use super::super::org::person_by_id;
use super::super::reduce::Effect;
use super::super::types::{FOUNDER_ID, Rank};
use super::Engine;

/// Starting a plan with five hires and a slow git can take minutes.
pub(crate) const OP_LIMIT_MS: i64 = 300_000;

/// Slow work under way.
#[derive(Clone, Debug)]
pub(crate) struct Running {
    key: String,
    label: String,
    started: i64,
}

pub(crate) fn exclude_key(dir: &str) -> String {
    format!("exclude-{}", dir.to_lowercase())
}

pub(crate) fn start_key(person: &str) -> String {
    format!("start-{person}")
}

impl Engine {
    /// Note slow work under way, so a hang is reported.
    pub(crate) fn track(&mut self, host: &dyn Host, key: String, label: String) {
        self.ops.retain(|r| r.key != key);
        self.ops.push(Running { key, label, started: host.now_ms() });
    }

    pub(crate) fn begin(&mut self, host: &mut dyn Host, op: Op, key: String, label: String) {
        self.track(host, key, label);
        host.begin(op);
    }

    fn untrack(&mut self, key: &str) {
        self.ops.retain(|r| r.key != key);
    }

    /// Whether slow work under this key is still going.
    pub(crate) fn ops_running(&self, key: &str) -> bool {
        self.ops.iter().any(|r| r.key == key)
    }

    /// Words for the Founder, as a failed approve would have put them.
    fn tell_founder(&mut self, host: &mut dyn Host, failed: Vec<String>) {
        if failed.is_empty() || self.state.is_none() {
            return;
        }
        self.run(host, vec![Effect::Deliver { person_id: FOUNDER_ID.into(), text: failed.join(" ") }]);
    }

    /// Take in what slow work has finished, and report what ran out of time.
    /// What one result starts may finish at once: take that in too, a few rounds at most.
    pub(crate) fn take_done(&mut self, host: &mut dyn Host) {
        for _ in 0..4 {
            let done = host.done();
            if done.is_empty() {
                break;
            }
            for d in done {
                self.one_done(host, d);
            }
        }
        let now = host.now_ms();
        let late: Vec<Running> = self.ops.iter().filter(|r| now - r.started > OP_LIMIT_MS).cloned().collect();
        for r in late {
            self.untrack(&r.key);
            let text = format!("{} took over {} s, so ByteHive moved on. It may still finish.", r.label, OP_LIMIT_MS / 1000);
            self.standing.raise(&format!("late-{}", r.label), &text, None);
            self.changed();
        }
    }

    fn one_done(&mut self, host: &mut dyn Host, d: Done) {
        match d {
            Done::Node(result) => {
                self.untrack("node");
                self.node = Some(result);
            }
            Done::Excluded { dir, result } => {
                self.untrack(&exclude_key(&dir));
                if let Err(e) = result {
                    self.excluded.remove(&dir.to_lowercase()); // the next job tries again
                    host.log(&format!(".bytehive/ could not be excluded in {dir}: {e}"));
                }
            }
            Done::Started { person, trust, start } => {
                self.untrack(&start_key(&person));
                self.started(host, &person, trust, start);
            }
            Done::Loaded(read) => {
                self.untrack("load");
                self.loaded(host, read);
            }
            Done::Files { failed, report } => {
                self.ops.retain(|r| !r.key.starts_with("files-"));
                if report {
                    self.tell_founder(host, failed);
                } else {
                    failed.iter().for_each(|f| host.log(f));
                }
            }
            Done::RolesSaved(failed) => {
                self.ops.retain(|r| !r.key.starts_with("roles-"));
                self.tell_founder(host, failed);
            }
            Done::BoardChanged { floor, change, result } => {
                let job = match &change {
                    BoardChange::Place { job, .. } => job.id.clone(),
                    BoardChange::MarkStopped { job } => job.clone(),
                };
                self.untrack(&format!("board-{job}-{floor}"));
                match (result, change) {
                    (Ok(()), _) => {}
                    (Err(e), BoardChange::Place { .. }) => self.tell_founder(host, vec![format!("Cards for {floor} were not placed: {e}.")]),
                    (Err(e), BoardChange::MarkStopped { .. }) => host.log(&format!("{job}'s cards on {floor} were not marked: {e}")),
                }
            }
            Done::BoardsRead(boards) => {
                self.untrack("watch");
                self.watched(host, boards);
            }
        }
    }

    /// A person's start is over. Claude's folder question first: when it could
    /// not be settled the person may wait on it, and the owner hears why. A start
    /// that failed goes to the Founder, as one that failed at once does.
    fn started(&mut self, host: &mut dyn Host, person: &str, trust: Result<(), String>, start: Result<(), String>) {
        let Some(p) = self.state.as_ref().and_then(|s| person_by_id(&s.company, person)).cloned() else { return };
        let key = format!("trust-{}", p.id);
        match trust {
            Ok(()) => {
                if self.standing.clear(&key) {
                    self.changed();
                }
            }
            Err(e) => {
                host.log(&format!("Claude's folder question stays: {e}"));
                self.standing.raise(&key, &format!("{} may be stuck on Claude's folder question: {e}.", p.name), Some(&p.id));
                self.changed();
            }
        }
        match start {
            Ok(()) => {
                if self.remember(host) {
                    self.try_persist(host, &[]);
                }
            }
            Err(why) if p.rank == Rank::Founder => {
                self.standing.raise("founder-start", &format!("The Founder did not start: {why}."), Some(FOUNDER_ID));
                self.changed();
            }
            Err(why) => {
                let text = format!("{} did not start: {why}.", p.name);
                self.run(host, vec![Effect::Deliver { person_id: FOUNDER_ID.into(), text }]);
            }
        }
    }
}
