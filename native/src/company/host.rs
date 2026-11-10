//! What the engine asks of the app: its clock, the open projects, the people's
//! seats and terminals, the floors' boards and the person files. The engine keeps
//! the company's own files (company.json, jobs, the lock, requests) itself;
//! everything else goes through here, so the engine is tested with a fake.

use super::types::{Company, Job, Person, PlanCard, RoleDef};
use crate::board::Board;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// An open project: every one with a folder is a floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub dir: Option<String>,
}

/// A person's seat as the engine sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeatView {
    pub running: bool,
    /// Starting: not running yet, and must not be started again.
    pub spawning: bool,
    /// A question is on screen: a message's Enter would answer it.
    pub asking: bool,
    /// At its input box, ready to be typed into.
    pub at_prompt: bool,
    pub spawned_at: Option<i64>,
    pub last_output_at: i64,
    pub last_input_at: i64,
    /// What the seat runs on now, written down so a closed seat comes back on it.
    pub session: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    /// The folder its CLI runs in: its worktree, else its project.
    pub run_dir: Option<String>,
    /// Why it did not start, when it did not.
    pub error: Option<String>,
    /// Fresh tokens so far: None before the first read, Some(None) for a read with no figure.
    pub fresh_tokens: Option<Option<u64>>,
}

/// What a person comes back on, less whatever could not be found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Restore {
    pub session: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
}

/// When a board file was read, to tell a write that would land on an agent's.
pub type BoardStamp = Option<SystemTime>;

#[derive(Debug, PartialEq, Eq)]
pub enum BoardWrite {
    /// Someone wrote it since it was read: read it again and redo the change.
    Conflict,
    Failed(String),
}

/// The company as it was read off the disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub company: Company,
    pub jobs: Vec<Job>,
    /// Job files that could not be read, left in place.
    pub unreadable: Vec<String>,
    pub roles: Vec<RoleDef>,
}

/// A change to a floor's board.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardChange {
    /// A Job's cards for this floor, at the end of its To do list.
    Place { job: Job, cards: Vec<PlanCard>, company: Company },
    /// A stopped Job's open cards marked as stopped.
    MarkStopped { job: String },
}

/// Slow work the engine starts and never waits for: it runs off the UI thread,
/// and its result comes back through `Host::done`.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Whether Node can run the MCP server.
    NodeCheck,
    /// Keep a project's .bytehive/ out of its git status.
    Exclude { dir: String },
    /// Seed the roles, read the company, its Jobs and its roles.
    Load { root: PathBuf },
    /// Write these people's prompt, MCP config, plugin and settings. `report`:
    /// failures go to the Founder, not only the log.
    Files { root: PathBuf, people: Vec<Person>, roles: Vec<RoleDef>, company: Company, report: bool },
    /// Write the roles a plan drafted, now the owner approved them.
    SaveRoles { root: PathBuf, roles: Vec<RoleDef> },
    /// Change a floor's board, trying again on an agent's write in between.
    ChangeBoard { floor: String, dir: String, change: BoardChange },
    /// Read these floors' boards (floor id, folder), for the watch.
    ReadBoards { floors: Vec<(String, String)> },
}

/// What came of slow work since the last tick.
#[derive(Clone, Debug, PartialEq)]
pub enum Done {
    /// Err is why a job is refused.
    Node(Result<(), String>),
    Excluded { dir: String, result: Result<(), String> },
    /// A person's start or restart is over: Claude's folder question settled
    /// first (Ok when there was none to settle), then the CLI started or not.
    Started { person: String, trust: Result<(), String>, start: Result<(), String> },
    Loaded(Result<Loaded, String>),
    /// What could not be written, as words.
    Files { failed: Vec<String>, report: bool },
    RolesSaved(Vec<String>),
    BoardChanged { floor: String, change: BoardChange, result: Result<(), String> },
    /// Each floor's board: None when it has none, Err when it could not be read.
    BoardsRead(Vec<(String, Result<Option<Board>, String>)>),
}

/// No Host call blocks for more than a few ms: whatever takes longer (git, a
/// worktree, Claude's config lock, Node) runs off the UI thread and reports
/// through `done`.
pub trait Host {
    fn now_ms(&self) -> i64;
    /// This app's process id.
    fn pid(&self) -> u32;
    /// A process that can't be asked counts as alive.
    fn process_alive(&self, pid: u64) -> bool;
    /// Every open project, in the order they are listed.
    fn projects(&self) -> Vec<Project>;
    /// The project on screen, if any.
    fn active_project(&self) -> Option<Project>;
    fn seat(&self, person: &str) -> Option<SeatView>;
    /// Every seat's name: a new hire must not share one.
    fn seat_names(&self) -> Vec<String>;
    /// Write a person's prompt, MCP config, plugin and settings. Gives back what
    /// could not be written, as words for the Founder.
    fn write_person_files(&mut self, root: &Path, person: &Person, role: &RoleDef, company: &Company) -> Vec<String>;
    /// Give a person a seat in `dir`, on what `restore` still has, and start it
    /// off the UI thread: the seat is spawning at once, and `Done::Started` says
    /// how it went. Err only for what is wrong before anything runs.
    fn spawn_person(&mut self, root: &Path, person: &Person, role: &RoleDef, dir: &str, restore: &Restore) -> Result<(), String>;
    /// Start a stopped seat again, the same way: spawning now, `Done::Started` later.
    fn restart(&mut self, person: &str);
    /// Type a message and Enter into a person's terminal.
    fn type_message(&mut self, person: &str, text: &str);
    /// Esc into a person's terminal, to end their turn.
    fn interrupt(&mut self, person: &str);
    fn folder_exists(&self, dir: &str) -> bool;
    fn session_exists(&self, dir: &str, session: &str) -> bool;
    /// Start slow work off the UI thread.
    fn begin(&mut self, op: Op);
    /// What slow work has finished since the last call.
    fn done(&mut self) -> Vec<Done>;
    /// A line for the log, where the TS wrote console.error.
    fn log(&mut self, line: &str);
}
