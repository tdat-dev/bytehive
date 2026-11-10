//! People: starting them on what they had, typing messages in when they are
//! ready for them, ending a turn (engine.ts ensurePerson, deliver, interrupt).

use super::super::host::{Host, Restore};
use super::super::org::person_by_id;
use super::super::reduce::{Effect, Seen, note_person, typeable};
use super::super::requests::{self, not_understood};
use super::super::types::{FOUNDER_ID, Person, Rank};
use super::{Engine, FOUNDER_WAITS, NO_PROJECT};

/// How long a new CLI gets to reach its prompt.
const BOOT_MS: i64 = 8000;
/// How long a message waits for an input box a screen never shows: typing anyway
/// beats losing it.
const PROMPT_WAIT_MS: i64 = 2 * 60_000;
/// How long a message waits on a question that nobody answers.
const GIVE_UP_MS: i64 = 15 * 60_000;
/// How soon a terminal echoes a key back. Output later than this is the agent's own.
const ECHO_MS: i64 = 500;
/// Output this recent means a turn is going on (supervise's WORKING_MS).
const WORKING_MS: i64 = supervise::task_state::WORKING_MS as i64;

/// A message waiting for its person to be ready (deliverTask in spawnmodal.ts).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub person: String,
    pub text: String,
    /// Not before then.
    start: i64,
    /// A question was on screen: when it went away, None while it is still there.
    answered: Option<Option<i64>>,
}

impl Engine {
    /// The folder a Founder with no seat starts in: the active project's, else
    /// the first open project with one.
    pub(crate) fn founder_dir(&self, host: &dyn Host) -> Option<String> {
        host.active_project().and_then(|p| p.dir).or_else(|| host.projects().into_iter().find_map(|p| p.dir))
    }

    /// Write down what every seat runs on, so closing one loses nothing. Gives back whether anything moved.
    pub(crate) fn remember(&mut self, host: &dyn Host) -> bool {
        let Some(before) = self.state.clone() else { return false };
        let mut s = before.clone();
        for p in &before.company.people {
            if let Some(seat) = host.seat(&p.id) {
                s = note_person(&s, &p.id, &Seen { session: seat.session, worktree: seat.worktree, branch: seat.branch });
            }
        }
        let moved = s != before;
        self.state = Some(s);
        moved
    }

    /// A person comes back on the conversation and branch they had. What is gone
    /// can't be: they start without it, and the owner hears they start blank. A
    /// conversation belongs to its folder, so it goes with a worktree.
    fn check_restore(&mut self, host: &dyn Host, p: &Person, dir: &str) -> Restore {
        let mut r = Restore { session: p.session.clone(), worktree: p.worktree.clone(), branch: p.branch.clone() };
        let mut gone = Vec::new();
        let mut commits = String::new();
        if r.worktree.as_deref().is_some_and(|w| !host.folder_exists(w)) {
            gone.push("worktree");
            if let Some(b) = &r.branch {
                commits = format!(" Anything they committed is on branch {b}.");
            }
            r.worktree = None;
            r.branch = None;
        }
        if let Some(session) = r.session.clone() {
            if !host.session_exists(r.worktree.as_deref().unwrap_or(dir), &session) {
                // The Founder moves with the owner's projects, and the Job files carry its work.
                if p.rank != Rank::Founder {
                    gone.push("conversation");
                }
                r.session = None;
            }
        }
        if !gone.is_empty() {
            let text = format!("{}'s {} could not be found, so they start blank.{commits}", p.name, gone.join(" and "));
            self.standing.raise(&format!("lost-{}", p.id), &text, Some(&p.id));
        }
        r
    }

    /// Start someone who has no seat yet, on what they had.
    pub(crate) fn ensure_person(&mut self, host: &mut dyn Host, p: &Person) -> Result<(), String> {
        let Some(s) = &self.state else { return Ok(()) };
        if host.seat(&p.id).is_some() {
            return Ok(());
        }
        let founder = p.rank == Rank::Founder;
        let role = s.roles.iter().find(|r| r.id == p.role).cloned().ok_or_else(|| format!("no role {}", p.role))?;
        let floor_dir = s.company.floors.iter().find(|f| Some(&f.id) == p.floor.as_ref()).map(|f| f.dir.clone());
        let company = s.company.clone();
        let dir = if founder { self.founder_dir(host) } else { floor_dir };
        let dir = dir.ok_or_else(|| if founder { NO_PROJECT.to_string() } else { format!("no floor {}", p.floor.as_deref().unwrap_or("")) })?;
        if founder {
            self.keep_out_of_git(host, &dir);
        }
        let failed = host.write_person_files(&self.root, p, &role, &company);
        if let Some(first) = failed.first() {
            return Err(first.clone());
        }
        let restore = self.check_restore(host, p, &dir);
        host.spawn_person(&self.root, p, &role, &dir, &restore)?;
        self.track(host, super::ops::start_key(&p.id), format!("Starting {}", p.name));
        if founder && self.standing.clear(FOUNDER_WAITS) {
            self.changed();
        }
        if self.remember(host) {
            self.try_persist(host, &[]);
        }
        Ok(())
    }

    /// Type into a person's terminal, starting or waking them first. A CLI that
    /// only just started isn't at its prompt yet, and a question on screen would
    /// take the message's Enter as its answer: such a message waits.
    pub(crate) fn deliver(&mut self, host: &mut dyn Host, person: &str, text: &str) {
        // Whatever reaches a terminal is text: a control key in it would be a keypress.
        let text = &typeable(text);
        let Some(p) = self.state.as_ref().and_then(|s| person_by_id(&s.company, person)).cloned() else { return };
        // Starting them would fail, and take the rest of this round's effects with it.
        if p.rank == Rank::Founder && host.seat(&p.id).is_none() && self.founder_dir(host).is_none() {
            self.standing.raise(FOUNDER_WAITS, "No project is open, so a message for the Founder was not delivered. Open a project: the Founder starts there with its next job and sees the open work.", None);
            self.changed();
            return;
        }
        if let Err(e) = self.ensure_person(host, &p) {
            host.log(&format!("{} could not be started: {e}", p.name));
        }
        let Some(seat) = host.seat(&p.id) else { return };
        let now = host.now_ms();
        let mut running = seat.running;
        let mut spawned_at = seat.spawned_at;
        if !seat.running && !seat.spawning {
            host.restart(&p.id);
            self.track(host, super::ops::start_key(&p.id), format!("Starting {}", p.name));
            let after = host.seat(&p.id).unwrap_or_default();
            running = after.running;
            spawned_at = after.spawned_at;
        }
        let left = if running { BOOT_MS - (now - spawned_at.unwrap_or(0)) } else { BOOT_MS };
        let ready = left <= 0 && !seat.asking && seat.at_prompt;
        #[cfg(test)]
        self.said.push((p.id.clone(), text.to_string(), !ready));
        if ready {
            host.type_message(&p.id, text);
        } else {
            self.outbox.push(Pending { person: p.id, text: text.to_string(), start: now + left.max(0), answered: None });
        }
    }

    /// Type each waiting message whose person is ready for it.
    pub(crate) fn send_waiting(&mut self, host: &mut dyn Host) {
        let now = host.now_ms();
        let mut keep = Vec::new();
        for mut m in std::mem::take(&mut self.outbox) {
            if now < m.start {
                keep.push(m);
                continue;
            }
            let Some(seat) = host.seat(&m.person) else {
                host.log(&format!("A message for {} was not delivered: they have no seat any more.", m.person));
                continue;
            };
            if !seat.running {
                // Still starting: it waits, as long as a question would.
                if seat.spawning && now - m.start <= GIVE_UP_MS {
                    keep.push(m);
                } else {
                    host.log(&format!("A message for {} was not delivered: they did not start.", m.person));
                }
                continue;
            }
            if seat.asking {
                m.answered = Some(None);
                // Never answer it for them.
                if now - m.start <= GIVE_UP_MS {
                    keep.push(m);
                } else {
                    host.log(&format!("A message for {} was not delivered: a question stayed on their screen.", m.person));
                }
                continue;
            }
            if let Some(at) = m.answered {
                let at = at.unwrap_or(now);
                m.answered = Some(Some(at));
                if now - at < BOOT_MS {
                    keep.push(m);
                    continue;
                }
            }
            // A resume can take half a minute with hooks and MCP servers starting.
            if !seat.at_prompt && now - m.start < PROMPT_WAIT_MS {
                keep.push(m);
                continue;
            }
            host.type_message(&m.person, &m.text);
        }
        self.outbox = keep;
    }

    /// What waits to be typed, oldest first.
    pub fn waiting(&self) -> &[Pending] {
        &self.outbox
    }

    pub(crate) fn run(&mut self, host: &mut dyn Host, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::Owner { key, text } => {
                    self.standing.raise(&key, &text, None);
                    self.changed();
                }
                Effect::Deliver { person_id, text } => self.deliver(host, &person_id, &text),
            }
        }
    }

    /// One line of requests.jsonl. One that can't be read is logged, and its
    /// sender, when the line names one, is told: nothing is dropped without a word.
    pub(crate) fn apply_line(&mut self, host: &mut dyn Host, line: &str) {
        let Some(s) = &self.state else { return };
        let req = match requests::parse(line) {
            Ok(r) => r,
            Err(bad) => {
                host.log(&format!("a request was not understood ({}): {line}", bad.why));
                let sender = bad.from.as_deref().and_then(|f| person_by_id(&s.company, f)).map(|p| p.id.clone());
                if let Some(to) = sender {
                    self.deliver(host, &to, &not_understood(&bad));
                }
                return;
            }
        };
        let r = super::super::reduce::apply_request(s, &req);
        if &r.state != s {
            // Only the Jobs it changed: an idle Job's file is left as it is.
            let jobs: Vec<_> = r.state.jobs.iter().filter(|j| !s.jobs.contains(j)).cloned().collect();
            self.state = Some(r.state);
            self.try_persist(host, &jobs);
        }
        self.run(host, r.effects);
    }

    /// End a person's turn, if they are in one. Quiet means they are at a prompt,
    /// where an Esc would only clear what the owner typed; so does output that is
    /// just the echo of that typing. On a question, Esc is its no.
    pub(crate) fn interrupt(&mut self, host: &mut dyn Host, person: &str) {
        let Some(seat) = host.seat(person) else { return };
        if !seat.running || seat.asking {
            return;
        }
        let fresh = host.now_ms() - seat.last_output_at <= WORKING_MS;
        if fresh && seat.last_output_at > seat.last_input_at + ECHO_MS {
            host.interrupt(person);
        }
    }

    /// The owner types to someone. Their word goes straight through.
    pub fn message_person(&mut self, host: &mut dyn Host, id: &str, text: &str) {
        let words = typeable(text);
        let words = words.trim();
        if self.state.is_some() && !words.is_empty() {
            self.deliver(host, id, &format!("[From the owner] {words}"));
        }
    }

    /// Start someone who has no running seat, on the conversation and branch they had.
    pub fn wake_person(&mut self, host: &mut dyn Host, id: &str) -> Result<(), String> {
        let Some(p) = self.state.as_ref().and_then(|s| person_by_id(&s.company, id)).cloned() else { return Ok(()) };
        self.ensure_person(host, &p)?;
        if host.seat(id).is_some_and(|s| !s.running && !s.spawning) {
            host.restart(id);
            self.track(host, super::ops::start_key(id), format!("Starting {}", p.name));
        }
        Ok(())
    }

    /// A seat restarts on the files it was given: write everyone's again. Gives back whose could not be.
    pub(crate) fn refresh_files(&mut self, host: &mut dyn Host, people: &[Person]) -> Vec<String> {
        let Some(s) = &self.state else { return Vec::new() };
        let company = s.company.clone();
        let mut failed = Vec::new();
        for p in people {
            let Some(role) = s.roles.iter().find(|r| r.id == p.role) else { continue };
            failed.extend(host.write_person_files(&self.root, p, role, &company));
        }
        failed
    }
}

/// The Founder's id, for the owner actions that always start with it.
pub(crate) const FOUNDER: &str = FOUNDER_ID;
