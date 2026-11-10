//! The Company's slow work, off the UI thread: Node's version, git's exclude
//! file, Claude's folder question, a hire's worktree. Each piece runs on a
//! thread of its own and leaves its result in one queue for the app; the next
//! tick takes it in through `Host::done`. A seat whose folder is ready starts
//! then, on the UI thread, as a terminal must.

use super::host::{Done, Loaded, Op};
use crate::fleet::Fleet;
use gpui_kit::App;
use std::sync::Mutex;

/// git's exclude file is one per repo, and floors share repos: one at a time.
static EXCLUDING: Mutex<()> = Mutex::new(());

/// One board change at a time, so two changes to one board never cross.
static BOARDS: Mutex<()> = Mutex::new(());

/// What slow work left for the next tick. One engine runs per app.
static BACK: Mutex<Back> = Mutex::new(Back { done: Vec::new(), ready: Vec::new() });

/// A seat to make ready before it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Prepare {
    pub seat: String,
    pub person: String,
    pub name: String,
    /// The folder its CLI will run in, or the project its worktree comes from.
    pub dir: String,
    pub claude: bool,
    /// Make it a worktree of its own first.
    pub worktree: bool,
}

/// A seat ready to start.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Ready {
    pub seat: String,
    pub person: String,
    pub trust: Result<(), String>,
    /// Its new worktree and branch, or why it works in the project folder.
    pub worktree: Option<Result<(String, String), String>>,
}

pub(crate) struct Back {
    pub done: Vec<Done>,
    pub ready: Vec<Ready>,
}

fn back() -> std::sync::MutexGuard<'static, Back> {
    BACK.lock().unwrap_or_else(|p| p.into_inner())
}

/// Run one piece of slow work, blocking. Off the UI thread only.
pub(crate) fn run_op(op: Op) -> Done {
    match op {
        Op::NodeCheck => Done::Node(super::server::node_check()),
        Op::Exclude { dir } => {
            let _one = EXCLUDING.lock().unwrap_or_else(|p| p.into_inner());
            let result = crate::gitexclude::keep_out(&dir);
            Done::Excluded { dir, result }
        }
        Op::Load { root } => Done::Loaded(load(&root)),
        Op::Files { root, people, roles, company, report } => {
            let failed = match super::server::mcp_server() {
                Some(mcp) => super::launch::refresh_files(&root, &people, &roles, &company, &mcp),
                None => people.iter().map(|p| format!("{}'s files were not written: the app's MCP server is missing.", p.name)).collect(),
            };
            Done::Files { failed, report }
        }
        Op::SaveRoles { root, roles } => Done::RolesSaved(super::roles::save(&root, &roles)),
        Op::ChangeBoard { floor, dir, change } => {
            let _one = BOARDS.lock().unwrap_or_else(|p| p.into_inner());
            let result = super::engine::change_board_with(
                &mut || crate::board::read(&dir).map_err(|e| match e {
                    crate::board::ReadError::Corrupt(why) | crate::board::ReadError::Unreadable(why) => why,
                }),
                &mut |b, seen| super::fleet_host::write_board_file(&dir, b, seen),
                &change,
            );
            Done::BoardChanged { floor, change, result }
        }
        Op::ReadBoards { floors } => {
            let read = floors
                .into_iter()
                .map(|(id, dir)| {
                    let board = crate::board::read(&dir).map(|b| b.map(|(b, _)| b)).map_err(|e| format!("{e:?}"));
                    (id, board)
                })
                .collect();
            Done::BoardsRead(read)
        }
    }
}

/// Seed the roles, then read the roles, the company and its Jobs.
fn load(root: &std::path::Path) -> Result<Loaded, String> {
    super::roles::seed(root).map_err(|e| format!("the roles could not be written: {e}"))?;
    let roles = super::roles::load(root).map_err(|e| format!("the roles could not be read: {e}"))?;
    let company = super::store::load_company(root).map_err(|e| format!("{e:#}"))?;
    let (jobs, unreadable) = super::store::load_jobs(root).map_err(|e| format!("{e:#}"))?;
    Ok(Loaded { company, jobs, unreadable, roles })
}

/// Settle Claude's folder question, then make the worktree. Blocking.
pub(crate) fn prepare(p: &Prepare) -> Ready {
    let trust = if p.claude { super::trust::trust_folder(&p.dir) } else { Ok(()) };
    let worktree = p.worktree.then(|| {
        let root = agent_core::git::repo_root(std::path::Path::new(&p.dir)).ok_or_else(|| "the project is not a git repo".to_string())?;
        let branch = crate::worktree::branch_name(&p.name, &p.seat);
        crate::worktree::add(&root, &branch).map(|path| (path, branch))
    });
    Ready { seat: p.seat.clone(), person: p.person.clone(), trust, worktree }
}

/// Run slow work on a thread of its own; its result waits for the next tick.
pub(crate) fn begin(op: Op) {
    std::thread::spawn(move || {
        let done = run_op(op);
        back().done.push(done);
    });
}

/// Make a seat's folder ready on a thread of its own; the next tick starts it.
pub(crate) fn make_ready(p: Prepare) {
    std::thread::spawn(move || {
        let ready = prepare(&p);
        back().ready.push(ready);
    });
}

/// Start the seats whose folder is ready, then hand over everything finished.
pub(crate) fn take(fleet: &mut Fleet, cx: &mut App) -> Vec<Done> {
    let ready = std::mem::take(&mut back().ready);
    let started: Vec<Done> = ready.into_iter().map(|r| start_ready(fleet, cx, r)).collect();
    let mut done = std::mem::take(&mut back().done);
    done.extend(started);
    done
}

/// Start a seat whose folder is ready, on the UI thread.
fn start_ready(fleet: &mut Fleet, cx: &mut App, r: Ready) -> Done {
    let Some(index) = fleet.seats.iter().position(|s| s.spec.id == r.seat) else {
        return Done::Started { person: r.person, trust: r.trust, start: Err("its seat was closed before it started".into()) };
    };
    match r.worktree {
        Some(Ok((path, branch))) => {
            let spec = &mut fleet.seats[index].spec;
            spec.worktree = Some(path);
            spec.branch = Some(branch);
        }
        // It works in the project folder, as a launch outside a repo does.
        Some(Err(why)) => eprintln!("[bytehive] {} has no worktree and works in the project folder: {why}", fleet.seats[index].spec.name),
        None => {}
    }
    let start = fleet.start(index, cx).map_err(|e| e.to_string());
    Done::Started { person: r.person, trust: r.trust, start }
}

#[cfg(test)]
#[path = "drive_tests.rs"]
mod tests;
