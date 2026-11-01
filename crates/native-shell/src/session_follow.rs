//! Resume carries on the conversation an agent was last in. ByteHive starts a
//! Claude agent on a session id it chose and resumes that id later, but the
//! conversation can move on inside the CLI: /clear starts a new one, /resume
//! switches. Claude Code writes the session each running process is in to
//! `~/.claude/sessions/<pid>.json`, so every 8 s this finds each agent's
//! process (by the session it was started on, else by folder and start time)
//! and moves the agent's session along: Restart, History and hand-offs then
//! follow the conversation it is really in. Port of the web app's sessionfollow.ts.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use agent_core::claude::{live_sessions, LiveSession};

use crate::app::ShellApp;
use crate::projects::normalize;
use crate::sessions_ui::Cli;
use crate::states::pane_cwd;

const EVERY: Duration = Duration::from_secs(8);
/// A process started this close to the agent's spawn may be its CLI.
pub const START_SLACK_MS: u64 = 20_000;

/// One running Claude agent, as the follower sees it.
pub struct Mine {
    /// Pane id and run (a restart is a new process).
    pub key: (String, u64),
    pub session: Option<String>,
    pub ran_in: String,
    pub spawned: u64,
}

#[derive(Default)]
pub struct Follow {
    pid_of: HashMap<(String, u64), u32>,
    last: Option<Instant>,
}

impl Follow {
    /// The agents whose conversation moved on: (index in `mine`, its session now).
    pub fn step(&mut self, mine: &[Mine], live: &[LiveSession]) -> Vec<(usize, String)> {
        self.pid_of.retain(|k, _| mine.iter().any(|m| &m.key == k));
        let mut found: HashMap<usize, &LiveSession> = HashMap::new();
        let mut claimed = HashSet::new();
        // First the sure ones: a process seen before, or the session it started on.
        for (i, m) in mine.iter().enumerate() {
            let s = match self.pid_of.get(&m.key) {
                Some(pid) => live.iter().find(|l| l.pid == *pid),
                None => live.iter().find(|l| Some(&l.session_id) == m.session.as_ref()),
            };
            if let Some(s) = s.filter(|s| !claimed.contains(&s.pid)) {
                claimed.insert(s.pid);
                self.pid_of.insert(m.key.clone(), s.pid);
                found.insert(i, s);
            }
        }
        // Then one started in its folder when it was, if only one fits.
        for (i, m) in mine.iter().enumerate() {
            if found.contains_key(&i) || self.pid_of.contains_key(&m.key) || m.ran_in.is_empty() {
                continue;
            }
            let near: Vec<&LiveSession> = live.iter().filter(|l| !claimed.contains(&l.pid) && normalize(&l.cwd).eq_ignore_ascii_case(&normalize(&m.ran_in)) && l.started_ms.abs_diff(m.spawned) <= START_SLACK_MS).collect();
            if let [s] = near[..] {
                claimed.insert(s.pid);
                self.pid_of.insert(m.key.clone(), s.pid);
                found.insert(i, s);
            }
        }
        let mut moved: Vec<(usize, String)> = found.into_iter().filter(|(i, s)| mine[*i].session.as_deref() != Some(s.session_id.as_str())).map(|(i, s)| (i, s.session_id.clone())).collect();
        moved.sort();
        moved
    }
}

impl ShellApp {
    /// Follow each running Claude agent's conversation (logic pass).
    pub(crate) fn poll_session_follow(&mut self) {
        let f = &mut self.extras.follow;
        if f.last.is_some_and(|t| t.elapsed() < EVERY) || cfg!(test) {
            return;
        }
        f.last = Some(Instant::now());
        let idx: Vec<usize> = (0..self.panes.len()).filter(|&i| self.panes[i].cli == Some(Cli::Claude) && self.panes[i].exit_code.is_none()).collect();
        if idx.is_empty() {
            f.pid_of.clear();
            return;
        }
        let mine: Vec<Mine> = idx.iter().map(|&i| {
            let p = &self.panes[i];
            Mine { key: (p.id.clone(), p.run), session: p.session_id.clone(), ran_in: pane_cwd(p), spawned: p.started_ms }
        }).collect();
        for (k, session) in self.extras.follow.step(&mine, &live_sessions()) {
            self.panes[idx[k]].session_id = Some(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(pid: u32, session: &str, cwd: &str, started: u64) -> LiveSession {
        LiveSession { pid, session_id: session.into(), cwd: cwd.into(), started_ms: started }
    }

    fn mine(id: &str, session: Option<&str>, ran_in: &str, spawned: u64) -> Mine {
        Mine { key: (id.into(), 1), session: session.map(str::to_string), ran_in: ran_in.into(), spawned }
    }

    #[test]
    fn follows_a_clear_by_its_process() {
        let mut f = Follow::default();
        let a = [mine("p1", Some("s1"), "D:/x", 1000)];
        assert!(f.step(&a, &[live(7, "s1", "D:/x", 1000)]).is_empty(), "same session: nothing moved");
        assert_eq!(f.step(&a, &[live(7, "s2", "D:/x", 1000)]), vec![(0, "s2".to_string())], "a /clear moved it on");
    }

    #[test]
    fn finds_an_unknown_one_by_folder_and_time_only_when_alone() {
        let mut f = Follow::default();
        let a = [mine("p1", None, "D:\\x\\", 10_000)];
        assert_eq!(f.step(&a, &[live(3, "s9", "d:/X", 15_000)]), vec![(0, "s9".to_string())]);
        let mut g = Follow::default();
        let two = [live(3, "s8", "D:/x", 9_000), live(4, "s9", "D:/x", 11_000)];
        assert!(g.step(&a, &two).is_empty(), "two could be it: guess none");
        assert!(Follow::default().step(&a, &[live(3, "s9", "D:/x", 40_000)]).is_empty(), "started too late");
    }
}
