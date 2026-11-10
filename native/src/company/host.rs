//! What the engine asks of the app: its clock, the open projects, the people's
//! seats and terminals, the floors' boards and the person files. The engine keeps
//! the company's own files (company.json, jobs, the lock, requests) itself;
//! everything else goes through here, so the engine is tested with a fake.

use super::types::{Company, Person, RoleDef};
use crate::board::Board;
use std::path::Path;
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

pub trait Host {
    fn now_ms(&self) -> i64;
    /// This app's process id.
    fn pid(&self) -> u32;
    fn process_alive(&self, pid: u64) -> bool;
    /// Every open project, in the order they are listed.
    fn projects(&self) -> Vec<Project>;
    /// The project on screen, if any.
    fn active_project(&self) -> Option<Project>;
    fn seat(&self, person: &str) -> Option<SeatView>;
    /// Every seat's name: a new hire must not share one.
    fn seat_names(&self) -> Vec<String>;
    /// The roles, the shipped ones written first where none is on disk.
    fn load_roles(&mut self, root: &Path) -> Result<Vec<RoleDef>, String>;
    /// Write a person's prompt, MCP config, plugin and settings. Gives back what
    /// could not be written, as words for the Founder.
    fn write_person_files(&mut self, root: &Path, person: &Person, role: &RoleDef, company: &Company) -> Vec<String>;
    /// Start a person's seat in `dir`, on what `restore` still has.
    fn spawn_person(&mut self, root: &Path, person: &Person, role: &RoleDef, dir: &str, restore: &Restore) -> Result<(), String>;
    /// Start a stopped seat again.
    fn restart(&mut self, person: &str);
    /// Type a message and Enter into a person's terminal.
    fn type_message(&mut self, person: &str, text: &str);
    /// Esc into a person's terminal, to end their turn.
    fn interrupt(&mut self, person: &str);
    fn folder_exists(&self, dir: &str) -> bool;
    fn session_exists(&self, dir: &str, session: &str) -> bool;
    /// A floor's board, or None when it has none yet. Err for one that can't be read.
    fn read_board(&mut self, dir: &str) -> Result<Option<(Board, BoardStamp)>, String>;
    fn write_board(&mut self, dir: &str, board: &Board, seen: BoardStamp) -> Result<(), BoardWrite>;
    /// Keep a project's .bytehive/ out of its git status.
    fn exclude_from_git(&mut self, dir: &str) -> Result<(), String>;
    /// Whether Node can run the MCP server: Err is why a job is refused.
    fn node_check(&mut self) -> Result<(), String>;
    /// A line for the log, where the TS wrote console.error.
    fn log(&mut self, line: &str);
    /// Tell Claude that `dir` is trusted, so it does not open on its folder question.
    fn trust_folder(&mut self, dir: &str) -> Result<(), String>;
}
