//! The phone dashboard (Settings → Remote): the server lives in the
//! `dashboard` crate; this starts and stops it off the UI thread, pushes the
//! fleet every second in the shape the page reads (dashboard.ts), and turns
//! what the page sends (raw keys, or a message) into typing. Settings' rows
//! for it are in settings/remote.rs.

mod snapshot;
mod terms;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use dashboard::{Dashboard, DashboardInfo};

use super::*;
use snapshot::{Inbound, parse_inbound};
use terms::FleetTerms;

/// How often the page's fleet is brought up to date (dashboard.ts pushes
/// every second).
const PUSH_EVERY: Duration = Duration::from_millis(1000);

/// Where the password's hash is kept: with the ByteHive app's config, so one
/// password serves both apps; a test run's own data folder instead.
fn auth_dir() -> Option<PathBuf> {
    std::env::var_os("BYTEHIVE_NATIVE_DIR").map(PathBuf::from).or_else(dashboard::auth::default_dir)
}

/// Settings → Remote → Set password: the two boxes, while open.
pub(super) struct PwForm {
    pub(super) first: Entity<InputState>,
    pub(super) again: Entity<InputState>,
    pub(super) error: Option<String>,
    _enter: [Subscription; 2],
}

pub(super) struct Remote {
    dash: Arc<Dashboard>,
    terms: Arc<FleetTerms>,
    pub(super) info: DashboardInfo,
    pub(super) has_password: bool,
    /// A start, stop or password change is running.
    pub(super) busy: bool,
    /// Why it couldn't start, in place of where to open it.
    pub(super) error: Option<String>,
    /// Said under Password for a while: changed, or couldn't remove it.
    pub(super) note: Option<String>,
    pub(super) form: Option<PwForm>,
    /// "Remove the dashboard password?" waiting for an answer.
    pub(super) confirm_clear: bool,
    /// Settings → Remote → Port, made when Settings opens.
    pub(super) port_input: Option<(Entity<InputState>, Subscription)>,
    /// Each agent's latest finish (kind, ms), until it works again.
    finishes: HashMap<String, (&'static str, u64)>,
    inbound: (async_channel::Sender<String>, async_channel::Receiver<String>),
    pusher: Option<Task<()>>,
}

impl Remote {
    pub(super) fn new() -> Self {
        let dash = Arc::new(Dashboard::new(auth_dir()));
        let has_password = dash.has_password();
        Remote {
            dash,
            terms: Arc::default(),
            info: DashboardInfo::default(),
            has_password,
            busy: false,
            error: None,
            note: None,
            form: None,
            confirm_clear: false,
            port_input: None,
            finishes: HashMap::new(),
            inbound: async_channel::unbounded(),
            pusher: None,
        }
    }
}

impl Inbox {
    /// At start: listen for what the page sends, and start the server when
    /// Settings left it on.
    pub(super) fn boot_remote(&mut self, cx: &mut Context<Self>) {
        let rx = self.remote.inbound.1.clone();
        cx.spawn(async move |this, cx| {
            while let Ok(body) = rx.recv().await {
                if this.update(cx, |this, cx| this.remote_inbound(&body, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        if self.prefs.remote_on {
            self.remote_apply(cx);
        }
    }

    /// Start or stop the server to match Settings, off the UI thread.
    pub(super) fn remote_apply(&mut self, cx: &mut Context<Self>) {
        let r = &mut self.remote;
        r.busy = true;
        r.error = None;
        let (dash, terms, on, lan, port) = (r.dash.clone(), r.terms.clone(), self.prefs.remote_on, self.prefs.remote_lan, self.prefs.remote_port);
        let send = r.inbound.0.clone();
        let hook: dashboard::SendHook = Arc::new(move |body| {
            let _ = send.try_send(body);
        });
        let job = cx.background_executor().spawn(async move { if on { dash.start(port, lan, terms, hook) } else { Ok(dash.stop()) } });
        cx.spawn(async move |this, cx| {
            let out = job.await;
            let _ = this.update(cx, |this, cx| this.remote_applied(out, port, cx));
        })
        .detach();
        cx.notify();
    }

    fn remote_applied(&mut self, out: Result<DashboardInfo, String>, port: u16, cx: &mut Context<Self>) {
        let r = &mut self.remote;
        r.busy = false;
        match out {
            Ok(info) => r.info = info,
            Err(e) => {
                // 10013: Windows keeps the port for itself (an excluded range).
                let hint = if e.contains("10013") { ". Windows keeps that port for itself here: pick another one below." } else { "" };
                r.error = Some(format!("Couldn't start on port {port}: {e}{hint}"));
                r.info = r.dash.status();
                self.prefs.remote_on = r.info.running;
                self.prefs.save();
            }
        }
        if self.remote.info.running {
            self.start_pushing(cx);
        } else {
            self.remote.pusher = None;
            self.remote.terms.set(HashMap::new());
        }
        cx.notify();
    }

    fn start_pushing(&mut self, cx: &mut Context<Self>) {
        if self.remote.pusher.is_some() {
            return;
        }
        self.remote.pusher = Some(cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |this, cx| this.remote_push(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(PUSH_EVERY).await;
            }
        }));
    }

    /// The page's fleet and the terminals it may open, now.
    fn remote_push(&mut self, cx: &mut Context<Self>) {
        if !self.remote.info.running {
            return;
        }
        let (snapshot, live) = self.fleet_snapshot(cx);
        self.remote.terms.set(live);
        self.remote.dash.push(snapshot);
    }

    /// A message or a raw key from the page, into that agent if it runs.
    /// Keys go as they are, so a menu can be driven; a message is typed in
    /// the way a hand-off is, Enter on its own once the text has landed.
    fn remote_inbound(&mut self, body: &str, cx: &mut Context<Self>) {
        let Some(msg) = parse_inbound(body) else { return };
        let id = match &msg {
            Inbound::Keys(id, _) | Inbound::Message(id, _) => id.clone(),
        };
        let Some(terminal) = self.fleet.seats.iter().find(|s| s.spec.id == id).and_then(|s| s.terminal.clone()) else { return };
        let t = terminal.read(cx);
        if t.exit_code().is_some() {
            return;
        }
        match msg {
            Inbound::Keys(_, keys) => t.write_raw(keys.into_bytes()),
            Inbound::Message(_, text) => {
                let now = now_ms();
                let typer = Some(Typer::new(&text, t.bracketed_paste(), now));
                self.queue_typing(Job { id, text, started: now, typer }, cx);
            }
        }
        // Show the effect soon (dashboard.ts pushes again at 150 and 450 ms).
        cx.spawn(async move |this, cx| {
            for wait in [150, 300] {
                cx.background_executor().timer(Duration::from_millis(wait)).await;
                if this.update(cx, |this, cx| this.remote_push(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Set or change the password, off the UI thread (argon2 takes a moment).
    pub(super) fn remote_set_password(&mut self, password: String, cx: &mut Context<Self>) {
        let r = &mut self.remote;
        r.busy = true;
        let (dash, changed) = (r.dash.clone(), r.has_password);
        let job = cx.background_executor().spawn(async move { dash.set_password(&password) });
        cx.spawn(async move |this, cx| {
            let out = job.await;
            let _ = this.update(cx, |this, cx| {
                let r = &mut this.remote;
                r.busy = false;
                r.has_password = r.dash.has_password();
                match out {
                    Ok(()) => {
                        r.form = None;
                        // a new password signs every device out
                        r.note = changed.then(|| "Changed. Devices that were signed in have to sign in again.".to_string());
                    }
                    Err(e) => {
                        if let Some(f) = &mut r.form {
                            f.error = Some(format!("Couldn't save it: {e}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Remove the password. A server other devices could reach stops first;
    /// it comes back for this computer only.
    pub(super) fn remote_clear_password(&mut self, cx: &mut Context<Self>) {
        let r = &mut self.remote;
        r.confirm_clear = false;
        let lan_on = r.info.running && self.prefs.remote_lan;
        match r.dash.clear_password() {
            Ok(info) => {
                r.has_password = r.dash.has_password();
                r.note = None;
                self.prefs.remote_lan = false;
                self.prefs.save();
                if lan_on && self.prefs.remote_on && !info.running {
                    self.remote_apply(cx);
                } else {
                    self.remote.info = info;
                }
            }
            Err(e) => r.note = Some(format!("Couldn't remove it: {e}")),
        }
        cx.notify();
    }

    /// Open the password boxes, empty, the first one focused.
    pub(super) fn remote_open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = cx.new(|cx| InputState::new(window, cx).placeholder("Password").masked(true));
        let again = cx.new(|cx| InputState::new(window, cx).placeholder("Same password again").masked(true));
        let enter = |input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>| {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.remote_save_password(window, cx);
                }
            })
        };
        let subs = [enter(&first, window, cx), enter(&again, window, cx)];
        first.update(cx, |input, cx| input.focus(window, cx));
        self.remote.form = Some(PwForm { first, again, error: None, _enter: subs });
        cx.notify();
    }

    /// Save password: check the two boxes, then hand it over.
    pub(super) fn remote_save_password(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.remote.busy {
            return;
        }
        let Some(f) = &mut self.remote.form else { return };
        let (a, b) = (f.first.read(cx).value().to_string(), f.again.read(cx).value().to_string());
        if let Some(problem) = password_problem(&a, &b) {
            f.error = Some(problem);
            cx.notify();
            return;
        }
        f.error = None;
        self.remote_set_password(a, cx);
    }
}

/// What is wrong with a new password typed twice, or none when it will do
/// (dashpw.ts passwordProblem).
pub(super) fn password_problem(first: &str, again: &str) -> Option<String> {
    if first.is_empty() {
        return Some("Type a password.".into());
    }
    if first.chars().count() < dashboard::auth::MIN_LEN {
        return Some(format!("Use at least {} characters.", dashboard::auth::MIN_LEN));
    }
    (first != again).then(|| "The two don't match.".into())
}

/// The dashboard is plain http: on the LAN the password and cookie travel as is.
const LAN_CLEARTEXT: &str = "On your Wi-Fi the password goes unencrypted: use it only on a network you trust.";

/// The Password row's line, and the LAN toggle's, for whether one is set
/// (dashpw.ts passwordNotes).
pub(super) fn password_notes(has_password: bool, port: u16) -> (String, String) {
    if has_password {
        (
            "Set. Anyone opening the dashboard signs in with it, this computer included.".into(),
            format!("Reachable from devices on the same Wi-Fi, after signing in. {LAN_CLEARTEXT} From another network: put Tailscale on this computer and your phone, then open this computer's Tailscale address (100.x.y.z) with :{port}."),
        )
    } else {
        ("None yet. Other devices need one before they can connect.".into(), format!("Set a password first. Without one, anyone on your network could type into your agents. {LAN_CLEARTEXT}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{password_notes, password_problem};

    #[test]
    fn checks_a_new_password() {
        assert_eq!(password_problem("", ""), Some("Type a password.".into()));
        assert_eq!(password_problem("short", "short"), Some("Use at least 10 characters.".into()));
        assert_eq!(password_problem("correct horse", "correct hors"), Some("The two don't match.".into()));
        assert_eq!(password_problem("correct horse", "correct horse"), None);
    }

    #[test]
    fn the_lan_note_names_the_port() {
        assert!(password_notes(true, 18477).1.ends_with("with :18477."));
        assert!(password_notes(false, 8477).1.starts_with("Set a password first."));
    }
}
