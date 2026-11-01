//! The remote fleet dashboard, run by the native shell: the server lives in
//! the `dashboard` crate; this module starts and stops it off the UI thread,
//! pushes the fleet snapshot every 2 s in the shape the page reads (the web
//! app's `dashboard.ts`), and turns what the page sends (a message, or raw
//! keys) into typing on the UI thread.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashboard::{Dashboard, DashboardInfo};
use eframe::egui;
use serde_json::{json, Value};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::projects::{normalize, pane_home, project_name};
use crate::remote_ui::{RemoteCmd, RemoteForm};
use crate::states::{pane_cwd, pane_status};

const PUSH_EVERY: Duration = Duration::from_secs(2);
/// Rows of each agent's screen the page shows.
const OUTPUT_ROWS: usize = 40;

/// What a background start, stop or password change came back with.
enum Done {
    Info(Result<DashboardInfo, String>),
    Password(Result<(), String>),
    Cleared(Result<DashboardInfo, String>),
}

pub struct Remote {
    pub dash: Arc<Dashboard>,
    send_tx: Sender<String>,
    send_rx: Receiver<String>,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    pub form: RemoteForm,
    booted: bool,
    last_push: Option<Instant>,
    /// Each agent's latest finish (kind, ms), until it works again.
    finishes: HashMap<String, (&'static str, u64)>,
}

impl Default for Remote {
    fn default() -> Self {
        let (send_tx, send_rx) = channel();
        let (done_tx, done_rx) = channel();
        // Tests never touch the real password file.
        let dir = if cfg!(test) { None } else { dashboard::auth::default_dir() };
        let dash = Arc::new(Dashboard::new(dir));
        let mut form = RemoteForm::default();
        form.has_password = dash.has_password();
        Remote { dash, send_tx, send_rx, done_tx, done_rx, form, booted: false, last_push: None, finishes: HashMap::new() }
    }
}

/// One agent row of the page.
pub struct DashRow {
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

/// Grouped by project, projects that need you first, a stable order inside
/// (the web app's `orderForDash`).
pub fn order_for_dash(rows: &mut [DashRow]) {
    let mut best: HashMap<String, u8> = HashMap::new();
    for r in rows.iter() {
        let b = best.entry(r.key.clone()).or_insert(3);
        *b = (*b).min(rank(r.status));
    }
    rows.sort_by(|a, b| {
        (best[&a.key], &a.project, &a.key, rank(a.status), &a.name).cmp(&(best[&b.key], &b.project, &b.key, rank(b.status), &b.name))
    });
}

/// "1h5m", "12m", "40s".
pub fn uptime(ms: u64) -> String {
    let s = ms / 1000;
    let (m, h) = (s / 60, s / 3600);
    if h > 0 { format!("{h}h{}m", m % 60) } else if m > 0 { format!("{m}m") } else { format!("{s}s") }
}

/// The page's word for a state: needs, active, idle or stopped.
pub fn status_word(state: TaskState, running: bool) -> &'static str {
    match state {
        _ if !running => "stopped",
        TaskState::Needs | TaskState::Blocked => "needs",
        TaskState::Working => "active",
        TaskState::Stopped | TaskState::Merged => "stopped",
        _ => "idle",
    }
}

/// What the page sends, as an action: raw keys, or a message to type.
#[derive(Debug, PartialEq)]
pub enum Inbound {
    Keys(String, String),
    Message(String, String),
}

pub fn parse_inbound(body: &str) -> Option<Inbound> {
    let v: Value = serde_json::from_str(body).ok()?;
    let id = v["paneId"].as_str()?.to_string();
    if let Some(k) = v["keys"].as_str().filter(|k| !k.is_empty()) {
        return Some(Inbound::Keys(id, k.to_string()));
    }
    let m = v["message"].as_str().filter(|m| !m.trim().is_empty())?;
    Some(Inbound::Message(id, m.to_string()))
}

impl ShellApp {
    /// Start or stop the server to match Settings, off the UI thread.
    fn remote_apply(&mut self, ctx: &egui::Context) {
        let r = &mut self.extras.remote;
        r.form.busy = true;
        r.form.error = None;
        let (dash, done, on, lan, port) = (r.dash.clone(), r.done_tx.clone(), self.prefs.remote_on, self.prefs.remote_lan, self.prefs.remote_port);
        let (registry, send, wake) = (self.registry.clone(), r.send_tx.clone(), ctx.clone());
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let out = if on {
                let hook: dashboard::SendHook = Arc::new(move |body| {
                    let _ = send.send(body);
                    wake.request_repaint();
                });
                dash.start(port, lan, registry, hook)
            } else {
                Ok(dash.stop())
            };
            let _ = done.send(Done::Info(out));
            ctx.request_repaint();
        });
    }

    fn remote_cmd(&mut self, ctx: &egui::Context, cmd: RemoteCmd) {
        let r = &mut self.extras.remote;
        let (dash, done, wake) = (r.dash.clone(), r.done_tx.clone(), ctx.clone());
        match cmd {
            RemoteCmd::Apply => return self.remote_apply(ctx),
            RemoteCmd::SetPassword(pw) => {
                r.form.busy = true;
                std::thread::spawn(move || {
                    let _ = done.send(Done::Password(dash.set_password(&pw)));
                    wake.request_repaint();
                });
            }
            RemoteCmd::ClearPassword => {
                r.form.busy = true;
                std::thread::spawn(move || {
                    let _ = done.send(Done::Cleared(dash.clear_password()));
                    wake.request_repaint();
                });
            }
        }
    }

    pub(crate) fn poll_remote(&mut self, ctx: &egui::Context) {
        if !self.extras.remote.booted {
            self.extras.remote.booted = true;
            if self.prefs.remote_on && !cfg!(test) {
                self.remote_apply(ctx);
            }
        }
        if let Some(cmd) = self.extras.remote.form.want.take() {
            self.remote_cmd(ctx, cmd);
        }
        let (mut save, mut reapply) = (false, false);
        while let Ok(done) = self.extras.remote.done_rx.try_recv() {
            let r = &mut self.extras.remote;
            let f = &mut r.form;
            f.busy = false;
            match done {
                Done::Info(Ok(info)) => f.info = info,
                Done::Info(Err(e)) => {
                    let port = self.prefs.remote_port;
                    // 10013: Windows keeps the port for itself (an excluded range).
                    let hint = if e.contains("10013") { " Windows keeps that port for itself here: pick another one below." } else { "" };
                    f.error = Some(format!("Couldn't start on port {port}: {e}.{hint}"));
                    f.info = r.dash.status();
                    self.prefs.remote_on = f.info.running;
                    save = true;
                }
                Done::Password(res) => {
                    let changed = f.has_password;
                    f.has_password = r.dash.has_password();
                    f.note = Some(match res {
                        Ok(()) if changed => "Changed. Devices that were signed in have to sign in again.".into(),
                        Ok(()) => "Set.".into(),
                        Err(e) => format!("Couldn't save it: {e}"),
                    });
                }
                Done::Cleared(res) => {
                    f.has_password = r.dash.has_password();
                    match res {
                        Ok(info) => {
                            // It was shared on the network: bring it back here, for this computer.
                            reapply |= self.prefs.remote_lan && self.prefs.remote_on;
                            self.prefs.remote_lan = false;
                            save = true;
                            f.info = info;
                        }
                        Err(e) => f.note = Some(format!("Couldn't remove it: {e}")),
                    }
                }
            }
        }
        if save {
            self.save_prefs();
        }
        if reapply {
            self.remote_apply(ctx);
        }
        while let Ok(body) = self.extras.remote.send_rx.try_recv() {
            self.remote_inbound(&body);
            self.extras.remote.last_push = None; // show the effect soon
        }
        if !self.extras.remote.form.info.running {
            return;
        }
        ctx.request_repaint_after(PUSH_EVERY);
        if self.extras.remote.last_push.is_some_and(|t| t.elapsed() < PUSH_EVERY) {
            return;
        }
        self.extras.remote.last_push = Some(Instant::now());
        let snap = self.fleet_snapshot();
        self.extras.remote.dash.push(snap);
    }

    fn remote_inbound(&mut self, body: &str) {
        let Some(msg) = parse_inbound(body) else { return };
        let id = match &msg {
            Inbound::Keys(id, _) | Inbound::Message(id, _) => id.clone(),
        };
        if !self.panes.iter().any(|p| p.id == id && p.exit_code.is_none()) {
            return;
        }
        match msg {
            Inbound::Keys(_, keys) => {
                let _ = agent_core::lock(&self.registry).write_keys(&id, keys.as_bytes());
            }
            Inbound::Message(_, text) => self.tell(id, text),
        }
    }

    /// The JSON the page renders: every agent and the tail of its screen.
    pub(crate) fn fleet_snapshot(&mut self) -> String {
        let now = now_ms();
        let mut rows = Vec::new();
        let mut output = serde_json::Map::new();
        for p in &self.panes {
            let changed = self.git_changed.get(&pane_cwd(p)).map(|f| f.len() as u32);
            let st = pane_status(p, now, changed);
            let running = p.exit_code.is_none();
            let status = status_word(st.state, running);
            let finished = match st.state {
                TaskState::Review => Some("review"),
                TaskState::Idle if p.task.is_some() && st.limit.is_none() && running => Some("done"),
                _ => None,
            };
            let kind = finished.or((!running && p.exit_code != Some(0)).then_some("crash"));
            let finish = match kind {
                Some(k) => {
                    let e = self.extras.remote.finishes.entry(p.id.clone()).or_insert((k, now));
                    if e.0 != k {
                        *e = (k, now);
                    }
                    json!({ "kind": e.0, "at": e.1 })
                }
                None => {
                    self.extras.remote.finishes.remove(&p.id);
                    Value::Null
                }
            };
            let home = pane_home(p);
            let dir = self.projects.owner(&home).map(str::to_string).unwrap_or(home);
            let (project, key) = (project_name(&dir), normalize(&dir).to_lowercase());
            let value = json!({
                "id": p.id, "name": p.name, "wsId": "main", "wsName": project,
                "project": project, "projectKey": key, "projectDir": dir,
                "status": status,
                "uptime": if running { uptime(now.saturating_sub(p.started_ms)) } else { String::new() },
                "finished": finished, "files": changed, "finish": finish,
            });
            if running {
                let screen = p.screen_text();
                let lines: Vec<&str> = screen.lines().collect();
                let tail = lines[lines.len().saturating_sub(OUTPUT_ROWS)..].join("\n");
                if !tail.trim().is_empty() {
                    output.insert(p.id.clone(), Value::String(tail.trim_end().to_string()));
                }
            }
            rows.push(DashRow { value, status, project, key, name: p.name.clone() });
        }
        order_for_dash(&mut rows);
        json!({ "agents": rows.into_iter().map(|r| r.value).collect::<Vec<_>>(), "output": output }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn words_and_uptimes() {
        assert_eq!(status_word(TaskState::Needs, true), "needs");
        assert_eq!(status_word(TaskState::Working, true), "active");
        assert_eq!(status_word(TaskState::Working, false), "stopped");
        assert_eq!(uptime(3_900_000), "1h5m");
        assert_eq!(uptime(40_000), "40s");
    }

    #[test]
    fn the_snapshot_has_what_the_page_reads() {
        let mut app = ShellApp::new().unwrap();
        app.panes[0].feed_bytes(b"hello from the shell\r\n");
        let v: Value = serde_json::from_str(&app.fleet_snapshot()).unwrap();
        let a = &v["agents"][0];
        for k in ["id", "name", "project", "projectKey", "status", "uptime", "finished", "files", "finish"] {
            assert!(a.get(k).is_some(), "{k} in {a}");
        }
        assert!(["active", "idle"].contains(&a["status"].as_str().unwrap()), "{a}");
        let id = a["id"].as_str().unwrap();
        assert!(v["output"][id].as_str().unwrap().contains("hello from the shell"));
    }

    #[test]
    fn a_message_from_the_page_is_typed_in() {
        let mut app = ShellApp::new().unwrap();
        let id = app.panes[0].id.clone();
        app.remote_inbound(&format!(r#"{{"paneId":"{id}","message":"echo hi"}}"#));
        assert_eq!(app.deliveries.len(), 1);
        app.remote_inbound(r#"{"paneId":"nobody","message":"x"}"#);
        assert_eq!(app.deliveries.len(), 1, "an unknown agent gets nothing");
    }
}
