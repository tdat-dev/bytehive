//! What the phone page reads every second (dashboard.ts fleetSnapshotJson):
//! every agent with its project, state and run time, and the tail of each
//! running agent's screen; grouped as dashorder.ts orderForDash groups them.
//! And what the page sends back: raw keys, or a message to type.

use std::collections::HashMap;

use serde_json::{Value, json};
use supervise::task_state::TaskState;

use super::super::Inbox;
use crate::fleet::{now_ms, project_of};
use crate::terminal::RemoteTerm;
use gpui_kit::Context;

/// Rows of each agent's screen the page shows (DASH_OUTPUT_ROWS).
const OUTPUT_ROWS: usize = 40;

/// One agent row of the page, with what it is sorted by.
pub(super) struct DashRow {
    pub value: Value,
    pub status: &'static str,
    pub project: String,
    pub key: String,
    pub name: String,
}

fn rank(status: &str) -> u8 {
    match status {
        "needs" => 0,
        "stopped" => 2,
        _ => 1,
    }
}

/// Grouped by project, projects that need you first, a stable order inside.
pub(super) fn order_for_dash(rows: &mut [DashRow]) {
    let mut best: HashMap<String, u8> = HashMap::new();
    for r in rows.iter() {
        let b = best.entry(r.key.clone()).or_insert(3);
        *b = (*b).min(rank(r.status));
    }
    rows.sort_by(|a, b| (best[&a.key], &a.project, &a.key, rank(a.status), &a.name).cmp(&(best[&b.key], &b.project, &b.key, rank(b.status), &b.name)));
}

/// "1h5m", "12m", "40s" (fmtUptimeShort).
pub(super) fn uptime(ms: u64) -> String {
    let s = ms / 1000;
    let (m, h) = (s / 60, s / 3600);
    if h > 0 {
        format!("{h}h{}m", m % 60)
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

/// The page's word for a state: needs, active, idle or stopped.
pub(super) fn status_word(state: TaskState, running: bool) -> &'static str {
    match state {
        _ if !running => "stopped",
        TaskState::Needs | TaskState::Blocked => "needs",
        TaskState::Working => "active",
        TaskState::Stopped | TaskState::Merged => "stopped",
        _ => "idle",
    }
}

/// Through with its work: "done" or "review" (finishnote.ts finishedLabel).
pub(super) fn finished_label(state: TaskState, worked: bool, limited: bool) -> Option<&'static str> {
    match state {
        TaskState::Review => Some("review"),
        TaskState::Idle if worked && !limited => Some("done"),
        _ => None,
    }
}

/// The key that groups agents of one folder (dashorder.ts projectOf).
pub(super) fn project_key(dir: &str) -> String {
    dir.trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase()
}

/// The last rows of a screen, or none when it is blank.
pub(super) fn screen_tail(screen: &str) -> Option<String> {
    let lines: Vec<&str> = screen.lines().collect();
    let tail = lines[lines.len().saturating_sub(OUTPUT_ROWS)..].join("\n");
    (!tail.trim().is_empty()).then(|| tail.trim_end().to_string())
}

/// What the page sends, as an action: raw keys, or a message to type.
#[derive(Debug, PartialEq)]
pub(super) enum Inbound {
    Keys(String, String),
    Message(String, String),
}

/// A POST /api/send body: `keys` is sent as is (arrows, Enter, Esc, ^C, Tab),
/// else a non-blank `message` is typed in.
pub(super) fn parse_inbound(body: &str) -> Option<Inbound> {
    let v: Value = serde_json::from_str(body).ok()?;
    let id = v["paneId"].as_str()?.to_string();
    if let Some(k) = v["keys"].as_str().filter(|k| !k.is_empty()) {
        return Some(Inbound::Keys(id, k.to_string()));
    }
    let m = v["message"].as_str().filter(|m| !m.trim().is_empty())?;
    Some(Inbound::Message(id, m.to_string()))
}

impl Inbox {
    /// The JSON the page renders, and the running agents' terminals by id.
    pub(super) fn fleet_snapshot(&mut self, cx: &mut Context<Self>) -> (String, HashMap<String, RemoteTerm>) {
        let now = now_ms();
        let mut rows = Vec::new();
        let mut output = serde_json::Map::new();
        let mut live = HashMap::new();
        for seat in &self.fleet.seats {
            let st = seat.status(cx);
            let terminal = seat.terminal.as_ref().map(|t| t.read(cx));
            let exit = terminal.and_then(|t| t.exit_code());
            let running = terminal.is_some_and(|t| t.exit_code().is_none()) || (seat.terminal.is_none() && seat.preparing);
            let status = status_word(st.state, running);
            let id = &seat.spec.id;
            let finished = finished_label(st.state, self.worked.contains(id), st.limit.is_some()).filter(|_| running);
            let kind = finished.or((terminal.is_some() && !running && exit != Some(0)).then_some("crash"));
            let finish = match kind {
                Some(k) => {
                    let e = self.remote.finishes.entry(id.clone()).or_insert((k, now));
                    if e.0 != k {
                        *e = (k, now);
                    }
                    json!({ "kind": e.0, "at": e.1 })
                }
                None => {
                    self.remote.finishes.remove(id);
                    Value::Null
                }
            };
            let dir = seat.spec.cwd.clone();
            let (project, key) = (project_of(&dir), project_key(&dir));
            let value = json!({
                "id": id, "name": seat.spec.name, "wsId": "main", "wsName": project,
                "project": project, "projectKey": key, "projectDir": dir,
                "status": status,
                "uptime": if running { uptime(now.saturating_sub(seat.spec.started_ms)) } else { String::new() },
                "finished": finished, "files": seat.changes.count, "finish": finish,
            });
            if let Some(t) = terminal.filter(|t| t.exit_code().is_none()) {
                if let Some(tail) = screen_tail(&t.screen_text()) {
                    output.insert(id.clone(), Value::String(tail));
                }
                live.insert(id.clone(), t.remote());
            }
            rows.push(DashRow { value, status, project, key, name: seat.spec.name.clone() });
        }
        order_for_dash(&mut rows);
        let json = json!({ "agents": rows.into_iter().map(|r| r.value).collect::<Vec<_>>(), "output": output });
        (json.to_string(), live)
    }
}

#[cfg(test)]
mod tests {
    use super::{DashRow, Inbound, finished_label, order_for_dash, parse_inbound, project_key, screen_tail, status_word, uptime};
    use serde_json::json;
    use supervise::task_state::TaskState;

    fn row(name: &str, project: &str, status: &'static str) -> DashRow {
        DashRow { value: json!(name), status, project: project.into(), key: project.to_lowercase(), name: name.into() }
    }

    #[test]
    fn projects_that_need_you_come_first() {
        let mut rows = vec![row("Bob", "alpha", "idle"), row("Ana", "zeta", "needs"), row("Cid", "alpha", "stopped"), row("Dan", "zeta", "active")];
        order_for_dash(&mut rows);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Ana", "Dan", "Bob", "Cid"]);
    }

    #[test]
    fn reads_what_the_page_sends() {
        assert_eq!(parse_inbound(r#"{"paneId":"p1","keys":"\u001b[A"}"#), Some(Inbound::Keys("p1".into(), "\u{1b}[A".into())));
        assert_eq!(parse_inbound(r#"{"paneId":"p1","message":"go on"}"#), Some(Inbound::Message("p1".into(), "go on".into())));
        assert_eq!(parse_inbound(r#"{"paneId":"p1","message":"  "}"#), None);
        assert_eq!(parse_inbound(r#"{"message":"x"}"#), None);
        assert_eq!(parse_inbound("nope"), None);
    }

    #[test]
    fn words_uptimes_and_keys() {
        assert_eq!(status_word(TaskState::Needs, true), "needs");
        assert_eq!(status_word(TaskState::Working, true), "active");
        assert_eq!(status_word(TaskState::Working, false), "stopped");
        assert_eq!(finished_label(TaskState::Idle, true, false), Some("done"));
        assert_eq!(finished_label(TaskState::Idle, true, true), None);
        assert_eq!(finished_label(TaskState::Review, false, false), Some("review"));
        assert_eq!(uptime(3_900_000), "1h5m");
        assert_eq!(uptime(40_000), "40s");
        assert_eq!(project_key("D:/Work/App/"), "d:\\work\\app");
    }

    #[test]
    fn keeps_the_last_rows_of_a_screen() {
        let screen: String = (0..50).map(|i| format!("line {i}\n")).collect();
        let tail = screen_tail(&screen).unwrap();
        assert!(tail.starts_with("line 10\n") && tail.ends_with("line 49"), "{tail}");
        assert_eq!(screen_tail("\n\n  \n"), None);
    }
}
