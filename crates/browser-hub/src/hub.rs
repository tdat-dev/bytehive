//! The browser hub: one local WebSocket server inside Maestro that both sides
//! dial into.
//!
//! - **Browsers**: each Chrome/Edge/Brave profile running the "Maestro for
//!   Chrome" extension reaches us through the native host (`host.rs`).
//! - **Agents**: every maestro-mcp process (one per agent) dials in to call
//!   browser tools.
//!
//! The hub routes an agent's call to the browser that agent picked (or the only
//! one connected) and routes the answer back. It never runs browser actions
//! itself. The port and a random token are written to
//! `~/.maestro/browser-hub.json`, readable only by this user's processes; every
//! peer must present the token in its first message.

use std::collections::HashMap;
use std::net::{Ipv4Addr, TcpListener};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BrowserInfo {
    pub id: u64,
    pub browser: String,
    pub email: String,
    pub profile: String,
    pub version: String,
}

impl BrowserInfo {
    pub fn label(&self) -> String {
        let who = if !self.profile.is_empty() {
            self.profile.clone()
        } else if !self.email.is_empty() {
            self.email.clone()
        } else {
            "unnamed profile".into()
        };
        format!("{} · {}", self.browser, who)
    }
}

enum Role {
    Unknown,
    Browser(BrowserInfo),
    Agent { name: String, pick: Option<u64> },
}

struct Peer {
    tx: mpsc::Sender<String>,
    role: Role,
}

/// Who asked. Maestro itself (the live view) asks too, and waits on a channel.
enum Asker {
    Agent(u64, Value),
    App(mpsc::Sender<Value>),
}

struct Pending {
    asker: Asker,
    browser: u64,
    agent: String,
    /// The call as sent, so a click held for the user's OK can be sent again.
    call: Value,
}

/// A risky click waiting for the user's OK (see RISKY in the extension).
struct Held {
    agent_conn: u64,
    cid: Value,
    browser: u64,
    agent: String,
    call: Value,
    what: String,
    url: String,
    at: u64,
}

/// How long a held click waits for the user. The agent's call gives up after
/// the same time (HOLD_TIMEOUT_MS in maestro-mcp), so a late OK never clicks
/// for an agent that has moved on.
pub const HOLD_MS: u64 = 10 * 60_000;

/// What Maestro shows for a held click.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Ask {
    pub id: u64,
    pub agent: String,
    pub what: String,
    pub url: String,
    pub at: u64,
}

/// A page that wants a person (a login or a CAPTCHA).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Blocker {
    pub agent: String,
    pub kind: String,
    pub url: String,
}

/// What an agent last did in a browser, for the live view and the Stop button.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Activity {
    pub agent: String,
    pub browser: u64,
    pub label: String,
    pub tool: String,
    pub action: String,
    pub at: u64,
    pub paused: bool,
}

#[derive(Default)]
struct Inner {
    peers: HashMap<u64, Peer>,
    pending: HashMap<u64, Pending>,
    activity: HashMap<String, Activity>,
    /// Browsers already told to reload onto the extension on disk (once each).
    reloaded: std::collections::HashSet<String>,
    held: HashMap<u64, Held>,
    /// Hold sending, posting, paying and deleting clicks for the user's OK.
    ask_risky: bool,
    next: u64,
}

pub enum HubEvent {
    Browsers(Vec<BrowserInfo>),
    Activity(Vec<Activity>),
    Asks(Vec<Ask>),
    Blocker(Blocker),
}

fn asks(inner: &Inner) -> Vec<Ask> {
    let now = now_ms();
    let mut v: Vec<Ask> = inner
        .held
        .iter()
        .filter(|(_, h)| now.saturating_sub(h.at) < HOLD_MS)
        .map(|(id, h)| Ask { id: *id, agent: h.agent.clone(), what: h.what.clone(), url: h.url.clone(), at: h.at })
        .collect();
    v.sort_by_key(|a| a.id);
    v
}

type OnChange = Box<dyn Fn(HubEvent) + Send + Sync>;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn activities(inner: &Inner) -> Vec<Activity> {
    let mut v: Vec<Activity> = inner.activity.values().cloned().collect();
    v.sort_by(|a, b| a.agent.cmp(&b.agent));
    v
}

fn browsers_of(inner: &Inner) -> Vec<BrowserInfo> {
    let mut l: Vec<BrowserInfo> = inner.peers.values().filter_map(|p| match &p.role { Role::Browser(b) => Some(b.clone()), _ => None }).collect();
    l.sort_by_key(|b| b.id);
    l
}

pub struct Hub {
    inner: Mutex<Inner>,
    token: String,
    pub port: u16,
    on_change: OnChange,
    /// The version of the extension on disk, read each time a browser says
    /// hello. A browser running another one is told to reload, so an update
    /// (or an edit during development) reaches every profile without a click.
    pub wanted_version: Box<dyn Fn() -> Option<String> + Send + Sync>,
    /// Maps a browser's signed-in email to its profile name. Swappable in tests.
    profile_of: Box<dyn Fn(&str, &str) -> String + Send + Sync>,
}


mod file;
mod route;
#[cfg(test)]
mod tests;

pub use file::{hub_file, own_hub_file, owns_browser, random_token};
use file::{answers, fail, publish, reply, should_claim};
use route::serve;



impl Hub {
    pub fn new(token: String, port: u16, on_change: OnChange) -> Self {
        Hub { inner: Mutex::new(Inner { ask_risky: true, ..Inner::default() }), token, port, on_change, wanted_version: Box::new(|| None), profile_of: Box::new(crate::profiles::profile_name) }
    }

    #[cfg(test)]
    pub fn for_test(token: &str, profile_of: impl Fn(&str, &str) -> String + Send + Sync + 'static) -> Self {
        Hub { inner: Mutex::new(Inner { ask_risky: true, ..Inner::default() }), token: token.into(), port: 0, on_change: Box::new(|_| {}), wanted_version: Box::new(|| None), profile_of: Box::new(profile_of) }
    }

    /// Bind 127.0.0.1 on a free port, publish it, and serve in the background.
    pub fn start(on_change: OnChange, wanted_version: Box<dyn Fn() -> Option<String> + Send + Sync>) -> std::io::Result<Arc<Hub>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let mut hub = Hub::new(random_token(), port, on_change);
        hub.wanted_version = wanted_version;
        let hub = Arc::new(hub);
        publish(port, &hub.token);
        // Every Maestro writes the one hub file, the last one opened wins, and
        // one that closes leaves its port there with nobody behind it: the
        // extension and every agent then find no browser. Look again now and
        // then, and take the file back when it names a hub that left.
        let token = hub.token.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(5));
            let raw = own_hub_file().and_then(|f| std::fs::read_to_string(f).ok());
            if should_claim(raw.as_deref(), port, answers) {
                publish(port, &token);
            }
        });
        let h = hub.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let h = h.clone();
                std::thread::spawn(move || serve(h, stream));
            }
        });
        Ok(hub)
    }

    pub fn browsers(&self) -> Vec<BrowserInfo> {
        browsers_of(&self.inner.lock().unwrap_or_else(|p| p.into_inner()))
    }

    pub fn activity(&self) -> Vec<Activity> {
        activities(&self.inner.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Stop (or let go on) an agent's browser use. While paused, every browser
    /// call from that agent is refused with a message telling it to wait.
    pub fn set_paused(&self, agent: &str, paused: bool) {
        let list = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let e = inner.activity.entry(agent.to_string()).or_insert_with(|| Activity {
                agent: agent.to_string(), browser: 0, label: String::new(), tool: String::new(), action: String::new(), at: now_ms(), paused,
            });
            e.paused = paused;
            activities(&inner)
        };
        (self.on_change)(HubEvent::Activity(list));
    }

    pub fn asks(&self) -> Vec<Ask> {
        asks(&self.inner.lock().unwrap_or_else(|p| p.into_inner()))
    }

    pub fn set_ask_risky(&self, on: bool) {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).ask_risky = on;
    }

    /// The user's answer to a held click: send it on, or tell the agent no.
    pub fn answer(&self, id: u64, ok: bool) {
        let list = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let Some(h) = inner.held.remove(&id) else { return };
            let agent_tx = inner.peers.get(&h.agent_conn).map(|p| p.tx.clone());
            let expired = now_ms().saturating_sub(h.at) >= HOLD_MS;
            if expired {
                // The agent stopped waiting; its call already failed.
            } else if ok {
                inner.next += 1;
                let hid = inner.next;
                let mut call = h.call.clone();
                call["id"] = json!(hid);
                call["args"]["confirmed"] = json!(true);
                match inner.peers.get(&h.browser) {
                    Some(b) if b.tx.send(call.to_string()).is_ok() => {
                        inner.pending.insert(hid, Pending { asker: Asker::Agent(h.agent_conn, h.cid), browser: h.browser, agent: h.agent, call });
                    }
                    _ => {
                        if let Some(tx) = &agent_tx {
                            let _ = tx.send(fail(&h.cid, "The user allowed it, but the browser has gone.".into()));
                        }
                    }
                }
            } else if let Some(tx) = &agent_tx {
                let _ = tx.send(fail(&h.cid, format!("The user said no to: {}. Don't do it; tell them what you were about to do and why.", h.what)));
            }
            asks(&inner)
        };
        (self.on_change)(HubEvent::Asks(list));
    }

    /// Maestro's own call into a browser on behalf of `agent` (the live view),
    /// answered on a channel. Uses the browser the agent last used.
    pub fn app_call(&self, agent: &str, tool: &str, args: Value, timeout: Duration) -> Result<Value, String> {
        let (tx, rx) = mpsc::channel();
        let hid = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let browsers = browsers_of(&inner);
            let target = inner
                .activity
                .get(agent)
                .map(|a| a.browser)
                .filter(|b| browsers.iter().any(|x| x.id == *b))
                .or_else(|| if browsers.len() == 1 { Some(browsers[0].id) } else { None })
                .ok_or_else(|| format!("{agent} isn't using a connected browser."))?;
            inner.next += 1;
            let hid = inner.next;
            let call = json!({ "type": "call", "id": hid, "agent": agent, "tool": tool, "args": args });
            let _ = inner.peers[&target].tx.send(call.to_string());
            inner.pending.insert(hid, Pending { asker: Asker::App(tx), browser: target, agent: agent.to_string(), call });
            hid
        };
        let got = rx.recv_timeout(timeout);
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).pending.remove(&hid);
        match got {
            Ok(v) if v["type"] == "result" => Ok(v["result"].clone()),
            Ok(v) => Err(v["error"].as_str().unwrap_or("The browser did not answer.").to_string()),
            Err(_) => Err("The browser took too long.".into()),
        }
    }

    pub fn add(&self, tx: mpsc::Sender<String>) -> u64 {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.next += 1;
        let id = inner.next;
        inner.peers.insert(id, Peer { tx, role: Role::Unknown });
        id
    }

    pub fn remove(&self, id: u64) {
        let (was_browser, orphans) = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let Some(peer) = inner.peers.remove(&id) else { return };
            let label = match &peer.role {
                Role::Browser(b) => Some(b.label()),
                _ => None,
            };
            let held_gone: Vec<u64> = inner.held.iter().filter(|(_, h)| h.browser == id || h.agent_conn == id).map(|(k, _)| *k).collect();
            for k in held_gone {
                let h = inner.held.remove(&k).unwrap();
                if h.browser == id {
                    if let Some(a) = inner.peers.get(&h.agent_conn) {
                        let _ = a.tx.send(fail(&h.cid, "The browser disconnected while waiting for the user.".into()));
                    }
                }
            }
            let gone: Vec<u64> = inner
                .pending
                .iter()
                .filter(|(_, p)| p.browser == id || matches!(&p.asker, Asker::Agent(a, _) if *a == id))
                .map(|(k, _)| *k)
                .collect();
            let mut orphans = vec![];
            for k in gone {
                let p = inner.pending.remove(&k).unwrap();
                let Some(l) = &label else { continue };
                let why = format!("{l} disconnected before it answered.");
                match p.asker {
                    Asker::Agent(a, cid) => {
                        if let Some(agent) = inner.peers.get(&a) {
                            let _ = agent.tx.send(fail(&cid, why));
                        }
                    }
                    Asker::App(tx) => orphans.push((tx, json!({ "type": "error", "error": why }))),
                }
            }
            (label.is_some(), orphans)
        };
        for (tx, v) in orphans {
            let _ = tx.send(v);
        }
        if was_browser {
            (self.on_change)(HubEvent::Browsers(self.browsers()));
        }
    }

    /// Handle one text message from peer `from`. False means: close it.
    pub fn on_message(&self, from: u64, text: &str) -> bool {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return true };
        let ty = v["type"].as_str().unwrap_or("");
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let Some(peer) = inner.peers.get_mut(&from) else { return false };
        match &mut peer.role {
            Role::Unknown => {
                if ty != "hello" || v["token"].as_str() != Some(self.token.as_str()) {
                    return false;
                }
                peer.role = match v["role"].as_str() {
                    Some("browser") => Role::Browser(BrowserInfo { id: from, browser: "Browser".into(), email: String::new(), profile: String::new(), version: String::new() }),
                    Some("agent") => Role::Agent { name: v["agent"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or("Agent").trim().to_string(), pick: None },
                    _ => return false,
                };
                let _ = peer.tx.send(json!({ "type": "hello_ok" }).to_string());
                let is_browser = matches!(peer.role, Role::Browser(_));
                drop(inner);
                if is_browser {
                    (self.on_change)(HubEvent::Browsers(self.browsers()));
                }
                true
            }
            Role::Browser(info) => {
                match ty {
                    "browser_info" => {
                        info.browser = v["browser"].as_str().unwrap_or("Browser").to_string();
                        info.email = v["email"].as_str().unwrap_or("").to_string();
                        info.version = v["version"].as_str().unwrap_or("").to_string();
                        info.profile = (self.profile_of)(&info.browser, &info.email);
                        let tx = peer.tx.clone();
                        let info = info.clone();
                        if let Some(want) = (self.wanted_version)().filter(|w| *w != info.version) {
                            if inner.reloaded.insert(format!("{}|{}|{}", info.browser, info.email, want)) {
                                let _ = tx.send(RELOAD.to_string());
                            }
                        }
                        drop(inner);
                        (self.on_change)(HubEvent::Browsers(self.browsers()));
                    }
                    "pause" => {
                        // Stop, pressed on the pill the extension draws over the page.
                        let agent = v["agent"].as_str().unwrap_or("").to_string();
                        drop(inner);
                        if !agent.is_empty() {
                            self.set_paused(&agent, true);
                        }
                    }
                    "result" | "error" => {
                        let Some(hid) = v["id"].as_u64() else { return true };
                        let Some(p) = inner.pending.remove(&hid) else { return true };
                        let confirm = v["result"]["needsConfirm"].clone();
                        let blocker = v["result"]["blocker"].clone();
                        match p.asker {
                            Asker::Agent(a, cid) if confirm.is_object() => {
                                // Hold the click; the agent's call waits for the user.
                                inner.next += 1;
                                let id = inner.next;
                                if let Some(agent) = inner.peers.get(&a) {
                                    let _ = agent.tx.send(json!({ "type": "hold", "id": cid }).to_string());
                                }
                                let what = confirm["what"].as_str().unwrap_or("a click").to_string();
                                let url = confirm["url"].as_str().unwrap_or("").to_string();
                                inner.held.retain(|_, h| now_ms().saturating_sub(h.at) < HOLD_MS);
                                inner.held.insert(id, Held { agent_conn: a, cid, browser: p.browser, agent: p.agent, call: p.call, what, url, at: now_ms() });
                                let list = asks(&inner);
                                drop(inner);
                                (self.on_change)(HubEvent::Asks(list));
                                return true;
                            }
                            Asker::Agent(a, cid) => {
                                if let Some(agent) = inner.peers.get(&a) {
                                    let mut out = v.clone();
                                    out["id"] = cid;
                                    let _ = agent.tx.send(out.to_string());
                                }
                            }
                            Asker::App(tx) => {
                                let _ = tx.send(v);
                            }
                        }
                        if blocker.is_object() {
                            let b = Blocker {
                                agent: p.agent,
                                kind: blocker["kind"].as_str().unwrap_or("").to_string(),
                                url: blocker["url"].as_str().unwrap_or("").to_string(),
                            };
                            drop(inner);
                            (self.on_change)(HubEvent::Blocker(b));
                        }
                    }
                    _ => {}
                }
                true
            }
            Role::Agent { .. } => {
                if ty == "call" && self.call(&mut inner, from, &v) {
                    let list = activities(&inner);
                    drop(inner);
                    (self.on_change)(HubEvent::Activity(list));
                }
                true
            }
        }
    }

}

const RELOAD: &str = r#"{"type":"call","id":0,"agent":"Maestro","tool":"reload","args":{}}"#;
const NONE_CONNECTED: &str = "No browser is connected to Maestro. Open Chrome (or Edge/Brave) in a profile that has the \"Maestro for Chrome\" extension; see Maestro Settings → Browser to install it.";
