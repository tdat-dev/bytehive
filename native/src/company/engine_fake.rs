//! A Host for the engine's tests: projects, seats and boards in memory, a clock
//! the test moves, and a record of everything the engine did to them.

use super::super::host::{BoardStamp, BoardWrite, Host, Project, Restore, SeatView};
use super::super::types::{Company, Look, Person, Rank, RoleDef};
use crate::board::Board;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub const A: &str = "D:\\a";
pub const B: &str = "D:\\b";

pub struct Seat {
    pub view: SeatView,
    pub name: String,
    pub dir: String,
}

pub struct Fake {
    pub now: i64,
    pub pid: u32,
    pub projects: Vec<Project>,
    pub active: Option<String>,
    pub seats: HashMap<String, Seat>,
    /// Names, in the order they were started.
    pub spawned: Vec<String>,
    pub spawn_fails: HashSet<String>,
    /// What a seat starting under this name boots into, as a CLI writes its session.
    pub on_boot: HashMap<String, Restore>,
    pub asking: bool,
    pub starting: bool,
    pub typed: Vec<(String, String)>,
    pub keys: Vec<String>,
    pub restarts: Vec<String>,
    pub boards: HashMap<String, Board>,
    pub board_reads: Vec<String>,
    pub read_fails: HashMap<String, String>,
    pub write_fails: VecDeque<BoardWrite>,
    pub excluded: Vec<String>,
    pub exclude_fails: bool,
    pub node: Result<(), String>,
    pub node_checks: usize,
    pub alive: HashSet<u64>,
    pub roles: Vec<RoleDef>,
    pub roles_fail: Option<String>,
    /// Whose files were written, in order.
    pub files: Vec<String>,
    pub gone: HashSet<String>,
    pub sessions: HashSet<(String, String)>,
    pub logs: Vec<String>,
    /// Folders Claude was told to trust, in order.
    pub trusted: Vec<String>,
    pub trust_fails: Option<String>,
    /// Typing text with this in it panics, as a bug in a host would.
    pub panic_on: Option<String>,
}

fn role(id: &str, rank: Rank) -> RoleDef {
    RoleDef {
        id: id.into(),
        title: id.to_uppercase(),
        department: "d".into(),
        rank,
        cli: "claude".into(),
        deny_tools: Vec::new(),
        look: Look::default(),
        prompt: format!("You are the {id}."),
        skills: Vec::new(),
    }
}

pub fn project(dir: &str) -> Project {
    Project { name: dir.rsplit('\\').next().unwrap_or(dir).to_string(), dir: Some(dir.to_string()) }
}

impl Fake {
    pub fn new() -> Fake {
        Fake {
            now: 1_000_000,
            pid: 7,
            projects: vec![project(A), project(B)],
            active: None,
            seats: HashMap::new(),
            spawned: Vec::new(),
            spawn_fails: HashSet::new(),
            on_boot: HashMap::new(),
            asking: false,
            starting: false,
            typed: Vec::new(),
            keys: Vec::new(),
            restarts: Vec::new(),
            boards: HashMap::new(),
            board_reads: Vec::new(),
            read_fails: HashMap::new(),
            write_fails: VecDeque::new(),
            excluded: Vec::new(),
            exclude_fails: false,
            node: Ok(()),
            node_checks: 0,
            alive: HashSet::new(),
            roles: vec![
                role("founder", Rank::Founder),
                role("lead", Rank::Lead),
                role("developer", Rank::Staff),
                role("designer", Rank::Staff),
                role("marketing", Rank::Staff),
            ],
            roles_fail: None,
            files: Vec::new(),
            gone: HashSet::new(),
            sessions: HashSet::new(),
            logs: Vec::new(),
            trusted: Vec::new(),
            trust_fails: None,
            panic_on: None,
        }
    }

    /// A seat already there, as the app left it.
    pub fn add_seat(&mut self, person: &str, name: &str, dir: &str, mut view: SeatView) {
        view.run_dir = view.run_dir.or_else(|| Some(dir.to_string()));
        self.seats.insert(person.into(), Seat { view, name: name.into(), dir: dir.into() });
    }

    pub fn seat_mut(&mut self, person: &str) -> &mut SeatView {
        &mut self.seats.get_mut(person).expect("a seat").view
    }

    pub fn close(&mut self, person: &str) {
        self.seats.remove(person);
    }

    pub fn dir_of(&self, person: &str) -> Option<&str> {
        self.seats.get(person).map(|s| s.dir.as_str())
    }
}

impl Host for Fake {
    fn now_ms(&self) -> i64 {
        self.now
    }
    fn pid(&self) -> u32 {
        self.pid
    }
    fn process_alive(&self, pid: u64) -> bool {
        self.alive.contains(&pid)
    }
    fn projects(&self) -> Vec<Project> {
        self.projects.clone()
    }
    fn active_project(&self) -> Option<Project> {
        let at = self.active.as_deref()?;
        self.projects.iter().find(|p| p.name == at || p.dir.as_deref() == Some(at)).cloned()
    }
    fn seat(&self, person: &str) -> Option<SeatView> {
        let s = self.seats.get(person)?;
        let mut v = s.view.clone();
        v.asking |= self.asking;
        v.at_prompt = v.at_prompt && !self.starting;
        Some(v)
    }
    fn seat_names(&self) -> Vec<String> {
        self.seats.values().map(|s| s.name.clone()).collect()
    }
    fn load_roles(&mut self, _root: &Path) -> Result<Vec<RoleDef>, String> {
        match &self.roles_fail {
            Some(e) => Err(e.clone()),
            None => Ok(self.roles.clone()),
        }
    }
    fn write_person_files(&mut self, _root: &Path, person: &Person, _role: &RoleDef, _company: &Company) -> Vec<String> {
        self.files.push(person.id.clone());
        Vec::new()
    }
    fn spawn_person(&mut self, _root: &Path, person: &Person, _role: &RoleDef, dir: &str, restore: &Restore) -> Result<(), String> {
        if self.spawn_fails.contains(&person.name) {
            return Err("no terminal".into());
        }
        let r = self.on_boot.get(&person.name).cloned().unwrap_or_else(|| restore.clone());
        let run_dir = Some(r.worktree.clone().unwrap_or_else(|| dir.to_string()));
        let view = SeatView { running: true, at_prompt: true, spawned_at: Some(self.now), session: r.session, worktree: r.worktree, branch: r.branch, run_dir, ..SeatView::default() };
        self.spawned.push(person.name.clone());
        self.seats.insert(person.id.clone(), Seat { view, name: person.name.clone(), dir: dir.into() });
        Ok(())
    }
    fn restart(&mut self, person: &str) {
        self.restarts.push(person.into());
        let now = self.now;
        if let Some(s) = self.seats.get_mut(person) {
            s.view.running = true;
            s.view.spawned_at = Some(now);
        }
    }
    fn type_message(&mut self, person: &str, text: &str) {
        if self.panic_on.as_deref().is_some_and(|p| text.contains(p)) {
            panic!("the fake host broke on {text}");
        }
        self.typed.push((person.into(), text.into()));
    }
    fn interrupt(&mut self, person: &str) {
        self.keys.push(person.into());
    }
    fn folder_exists(&self, dir: &str) -> bool {
        !self.gone.contains(dir)
    }
    fn session_exists(&self, dir: &str, session: &str) -> bool {
        self.sessions.contains(&(dir.to_string(), session.to_string()))
    }
    fn read_board(&mut self, dir: &str) -> Result<Option<(Board, BoardStamp)>, String> {
        self.board_reads.push(dir.into());
        if let Some(e) = self.read_fails.get(dir) {
            return Err(e.clone());
        }
        Ok(self.boards.get(dir).map(|b| (b.clone(), None)))
    }
    fn write_board(&mut self, dir: &str, board: &Board, _seen: BoardStamp) -> Result<(), BoardWrite> {
        if let Some(fail) = self.write_fails.pop_front() {
            return Err(fail);
        }
        self.boards.insert(dir.into(), board.clone());
        Ok(())
    }
    fn exclude_from_git(&mut self, dir: &str) -> Result<(), String> {
        if self.exclude_fails {
            return Err("git's exclude file: access denied".into());
        }
        self.excluded.push(dir.into());
        Ok(())
    }
    fn node_check(&mut self) -> Result<(), String> {
        self.node_checks += 1;
        self.node.clone()
    }
    fn log(&mut self, line: &str) {
        self.logs.push(line.into());
    }
    fn trust_folder(&mut self, dir: &str) -> Result<(), String> {
        self.trusted.push(dir.into());
        match &self.trust_fails {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}
