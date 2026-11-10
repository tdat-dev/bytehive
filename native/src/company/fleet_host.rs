//! The engine's Host over the native app: people are fleet seats with
//! `spec.person` set, boards are `.bytehive/board.json`, person files come from
//! launch.rs. Slow work (a seat's folder, git, Node) runs off the UI thread
//! through drive.rs. Typing a message takes several frames, so it is handed
//! back in `Outbound` for the Inbox's typer.

use super::drive::{self, Prepare};
use super::host::{BoardStamp, BoardWrite, Done, Host, Op, Project, Restore, SeatView};
use super::types::{Company, Person, Rank, RoleDef};
use super::{launch, roles, server};
use crate::board::{self, Board};
use crate::fleet::{Fleet, Seat, now_ms};
use gpui_kit::App;
use std::collections::HashMap;
use std::path::Path;
use supervise::ask::parse_ask;

/// What the Inbox does once the engine's tick is over.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outbound {
    /// (seat id, text): type it through the Inbox's typer.
    pub typed: Vec<(String, String)>,
    /// Always empty now: drive.rs makes a hire's worktree off the UI thread and
    /// starts the seat itself. Kept until the Inbox stops reading it.
    pub isolate: Vec<(String, String)>,
}

pub struct FleetHost<'a> {
    pub fleet: &'a mut Fleet,
    pub cx: &'a mut App,
    /// Every open project, in the order they are listed, and the one on screen.
    pub projects: Vec<Project>,
    pub active: Option<Project>,
    /// Fresh tokens by person id, read off the UI thread (fresh_tokens). A
    /// person missing here has not been read yet.
    pub fresh: &'a HashMap<String, Option<u64>>,
    pub out: Outbound,
}

/// A horizontal rule of Claude Code's input box.
fn rule(line: &str) -> bool {
    let t = line.trim();
    t.chars().count() > 10 && t.chars().all(|c| c == '\u{2500}' || c == '\u{2014}' || c == '-')
}

/// Claude Code drew its input box: a `>` line between the last two rules (composerDrawn).
pub fn composer_drawn(screen: &str) -> bool {
    let lines: Vec<&str> = screen.lines().collect();
    let rules: Vec<usize> = (0..lines.len()).filter(|&i| rule(lines[i])).collect();
    let [.., a, b] = rules[..] else { return false };
    lines[a + 1..b].iter().any(|l| matches!(l.trim_start().chars().next(), Some('>' | '\u{203A}')))
}

/// Whether a seat is ready to be typed into. Only Claude draws a box to look for.
pub fn at_prompt(cli: &str, screen: &str) -> bool {
    !cli.eq_ignore_ascii_case("claude") || composer_drawn(screen)
}

/// What a person has used in fresh tokens: input, output and cache writes, not
/// cache reads (agentcost.ts `fresh`). Blocking: run it off the UI thread.
pub fn fresh_tokens(cli: &str, dir: &str, session: Option<&str>, since_ms: u64) -> Option<u64> {
    match cli {
        "claude" => {
            let rows = agent_core::usage::claude_session_usage(dir, session?);
            let fresh: u64 = rows.iter().map(|r| r.input_tokens + r.output_tokens + r.cache_creation).sum();
            (fresh > 0).then_some(fresh)
        }
        "codex" => agent_core::usage::codex_usage(dir, since_ms.saturating_sub(5000)).and_then(|r| r.tokens),
        "opencode" => agent_core::usage::opencode_usage(dir, since_ms.saturating_sub(5000)).and_then(|r| r.tokens),
        _ => None,
    }
}

/// Whether a process runs. One that can't be asked counts as alive, as
/// processAlive's catch did: staying out costs less than starting everyone twice.
/// Only a pid Windows calls invalid is dead; NO_PID never reaches here (not a u32).
pub fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, GetLastError, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: a handle opened here is closed here, and nothing else is touched.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return GetLastError() != ERROR_INVALID_PARAMETER;
        }
        let mut code = 0u32;
        let asked = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        !asked || code == STILL_ACTIVE as u32
    }
}

/// Write a board unless an agent wrote it since `seen` (changeBoard's Conflict).
pub fn write_board_file(dir: &str, board: &Board, seen: BoardStamp) -> Result<(), BoardWrite> {
    if seen.is_some() && board::mtime(dir) != seen {
        return Err(BoardWrite::Conflict);
    }
    board::save(dir, board).map_err(BoardWrite::Failed)?;
    board::write_mirror(dir, board);
    Ok(())
}

impl FleetHost<'_> {
    /// Make a seat's folder ready off the UI thread; it starts on a later tick.
    fn prepare(&mut self, p: Prepare) {
        drive::make_ready(p);
    }

    fn seat_of(&self, person: &str) -> Option<&Seat> {
        self.fleet.seats.iter().rev().find(|s| s.spec.person.as_deref() == Some(person))
    }

    fn index_of(&self, person: &str) -> Option<usize> {
        self.fleet.seats.iter().rposition(|s| s.spec.person.as_deref() == Some(person))
    }
}

impl Host for FleetHost<'_> {
    fn now_ms(&self) -> i64 {
        now_ms() as i64
    }

    fn pid(&self) -> u32 {
        std::process::id()
    }

    fn process_alive(&self, pid: u64) -> bool {
        u32::try_from(pid).is_ok_and(pid_alive)
    }

    fn projects(&self) -> Vec<Project> {
        self.projects.clone()
    }

    fn active_project(&self) -> Option<Project> {
        self.active.clone()
    }

    fn seat(&self, person: &str) -> Option<SeatView> {
        let s = self.seat_of(person)?;
        let mut v = SeatView {
            spawning: s.preparing,
            spawned_at: Some(s.spec.started_ms as i64),
            session: s.spec.session_id.clone(),
            worktree: s.spec.worktree.clone(),
            branch: s.spec.branch.clone(),
            run_dir: Some(s.spec.run_dir().to_string()),
            fresh_tokens: self.fresh.get(person).copied(),
            ..SeatView::default()
        };
        if let Some(t) = s.terminal.as_ref().map(|t| t.read(self.cx)) {
            let screen = t.screen_text();
            v.running = t.exit_code().is_none();
            v.last_output_at = t.last_output_ms() as i64;
            v.last_input_at = t.last_input_ms() as i64;
            v.asking = parse_ask(&screen).is_some();
            v.at_prompt = at_prompt(&s.spec.cli, &screen);
            if let Some(code) = t.exit_code().filter(|c| *c != 0) {
                v.error = Some(format!("its CLI ended with code {code}"));
            }
        }
        Some(v)
    }

    fn seat_names(&self) -> Vec<String> {
        self.fleet.seats.iter().map(|s| s.spec.name.clone()).collect()
    }

    fn load_roles(&mut self, root: &Path) -> Result<Vec<RoleDef>, String> {
        roles::seed(root).map_err(|e| format!("the roles could not be written: {e}"))?;
        roles::load(root).map_err(|e| format!("the roles could not be read: {e}"))
    }

    fn write_person_files(&mut self, root: &Path, person: &Person, role: &RoleDef, company: &Company) -> Vec<String> {
        let Some(mcp) = server::mcp_server() else {
            return vec![format!("{}'s files were not written: the app's MCP server is missing.", person.name)];
        };
        launch::refresh_files(root, std::slice::from_ref(person), std::slice::from_ref(role), company, &mcp)
    }

    fn spawn_person(&mut self, _root: &Path, person: &Person, role: &RoleDef, dir: &str, restore: &Restore) -> Result<(), String> {
        let preset = crate::presets::preset(&person.cli).ok_or_else(|| format!("unknown CLI {}", person.cli))?;
        let args = preset.skip_args.iter().map(|a| a.to_string()).collect();
        let index = self.fleet.reserve(preset.program, args, dir, Some(role.title.clone()), None);
        let spec = &mut self.fleet.seats[index].spec;
        spec.name = person.name.clone();
        spec.person = Some(person.id.clone());
        spec.session_id = restore.session.clone();
        spec.worktree = restore.worktree.clone();
        spec.branch = restore.branch.clone();
        let run_dir = spec.run_dir().to_string();
        let seat = spec.id.clone();
        self.fleet.save();
        // Hired staff work in a worktree of their own; the Founder in the project.
        let worktree = person.rank != Rank::Founder && restore.worktree.is_none();
        self.prepare(Prepare { seat, person: person.id.clone(), name: person.name.clone(), dir: run_dir, claude: person.cli.eq_ignore_ascii_case("claude"), worktree });
        Ok(())
    }

    fn restart(&mut self, person: &str) {
        let Some(i) = self.index_of(person) else { return };
        let seat = &mut self.fleet.seats[i];
        seat.preparing = true;
        let p = Prepare {
            seat: seat.spec.id.clone(),
            person: person.to_string(),
            name: seat.spec.name.clone(),
            dir: seat.spec.run_dir().to_string(),
            claude: seat.spec.cli.eq_ignore_ascii_case("claude"),
            worktree: false,
        };
        self.prepare(p);
    }

    fn type_message(&mut self, person: &str, text: &str) {
        if let Some(s) = self.seat_of(person) {
            self.out.typed.push((s.spec.id.clone(), text.to_string()));
        }
    }

    fn interrupt(&mut self, person: &str) {
        if let Some(t) = self.seat_of(person).and_then(|s| s.terminal.clone()) {
            t.read(self.cx).write_raw(vec![0x1b]);
        }
    }

    fn folder_exists(&self, dir: &str) -> bool {
        Path::new(dir).is_dir()
    }

    fn session_exists(&self, dir: &str, session: &str) -> bool {
        agent_core::claude::session_exists(dir, session)
    }

    fn read_board(&mut self, dir: &str) -> Result<Option<(Board, BoardStamp)>, String> {
        board::read(dir).map_err(|e| match e {
            board::ReadError::Corrupt(why) | board::ReadError::Unreadable(why) => why,
        })
    }

    fn write_board(&mut self, dir: &str, board: &Board, seen: BoardStamp) -> Result<(), BoardWrite> {
        write_board_file(dir, board, seen)
    }

    fn begin(&mut self, op: Op) {
        drive::begin(op);
    }

    fn done(&mut self) -> Vec<Done> {
        drive::take(self.fleet, self.cx)
    }

    fn log(&mut self, line: &str) {
        eprintln!("[bytehive] {line}");
    }

}

#[cfg(test)]
#[path = "fleet_host_tests.rs"]
mod tests;
