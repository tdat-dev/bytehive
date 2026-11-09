//! Resume carries on the conversation an agent was last in (sessionfollow.ts).
//!
//! A Claude agent starts on a session id ByteHive chose and resumes that id
//! later, but the conversation moves on inside the CLI: /clear starts a new
//! one, /resume switches to another. Claude Code writes the session each
//! running process is in to ~/.claude/sessions/<pid>.json, so this finds the
//! agent's process (by the session it started on, else by its folder and
//! start time) and moves the agent's session id along.

use agent_core::claude::LiveSession;

/// How often the running Claude agents are checked.
pub const CHECK_MS: u64 = 8000;
/// A process started this close to the agent's start may be its CLI.
pub const START_SLACK_MS: u64 = 20_000;

/// One running Claude agent, as far as following it goes.
pub struct Run<'a> {
    pub session: Option<&'a str>,
    pub cwd: &'a str,
    pub started_ms: u64,
    /// Its process, once found on an earlier check.
    pub pid: Option<u32>,
}

fn norm(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// For each run, the process it is and the session that process is in now.
pub fn follow(runs: &[Run], live: &[LiveSession]) -> Vec<Option<(u32, String)>> {
    let mut found: Vec<Option<(u32, String)>> = vec![None; runs.len()];
    let mut claimed: Vec<u32> = Vec::new();
    // First the sure ones: a process seen before, or the session it started on.
    for (i, run) in runs.iter().enumerate() {
        let s = match run.pid {
            Some(pid) => live.iter().find(|l| l.pid == pid),
            None => live.iter().find(|l| run.session == Some(l.session_id.as_str())),
        };
        if let Some(s) = s.filter(|s| !claimed.contains(&s.pid)) {
            claimed.push(s.pid);
            found[i] = Some((s.pid, s.session_id.clone()));
        }
    }
    // Then one started in its folder when it was, if only one fits.
    for (i, run) in runs.iter().enumerate() {
        if found[i].is_some() || run.pid.is_some() || run.cwd.is_empty() {
            continue;
        }
        let near: Vec<&LiveSession> =
            live.iter().filter(|l| !claimed.contains(&l.pid) && norm(&l.cwd) == norm(run.cwd) && l.started_ms.abs_diff(run.started_ms) <= START_SLACK_MS).collect();
        if let [s] = near[..] {
            claimed.push(s.pid);
            found[i] = Some((s.pid, s.session_id.clone()));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{LiveSession, Run, follow};

    fn live(pid: u32, session: &str, cwd: &str, started_ms: u64) -> LiveSession {
        LiveSession { pid, session_id: session.to_string(), cwd: cwd.to_string(), started_ms }
    }

    #[test]
    fn moves_on_after_clear() {
        let runs = [Run { session: Some("a"), cwd: "D:/app", started_ms: 1000, pid: None }];
        // Found by the session it started on.
        assert_eq!(follow(&runs, &[live(7, "a", r"D:\app", 1500)]), vec![Some((7, "a".to_string()))]);
        // After /clear the process is known, and its new session is taken.
        let runs = [Run { session: Some("a"), cwd: "D:/app", started_ms: 1000, pid: Some(7) }];
        assert_eq!(follow(&runs, &[live(7, "b", r"D:\app", 1500)]), vec![Some((7, "b".to_string()))]);
    }

    #[test]
    fn guesses_by_folder_and_time_only_when_one_fits() {
        let runs = [Run { session: None, cwd: "D:/app/", started_ms: 100_000, pid: None }];
        assert_eq!(follow(&runs, &[live(9, "c", r"d:\APP", 110_000)]), vec![Some((9, "c".to_string()))]);
        assert_eq!(follow(&runs, &[live(9, "c", r"D:\app", 110_000), live(10, "d", r"D:\app", 105_000)]), vec![None]);
        assert_eq!(follow(&runs, &[live(9, "c", r"D:\app", 200_000)]), vec![None]);
    }

    #[test]
    fn one_process_per_agent() {
        let runs = [Run { session: Some("a"), cwd: "D:/app", started_ms: 0, pid: None }, Run { session: Some("a"), cwd: "D:/app", started_ms: 0, pid: None }];
        assert_eq!(follow(&runs, &[live(7, "a", "D:/app", 0)]), vec![Some((7, "a".to_string())), None]);
    }
}
