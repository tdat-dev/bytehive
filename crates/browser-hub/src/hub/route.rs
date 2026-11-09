//! Routing an agent's call to its browser, and the per-peer socket loop.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::Message;

use super::*;

impl Hub {
    /// Route one agent call. True when the agent's activity changed.
    pub(super) fn call(&self, inner: &mut Inner, from: u64, v: &Value) -> bool {
        let id = v["id"].clone();
        let tool = v["tool"].as_str().unwrap_or("");
        let args = v.get("args").cloned().unwrap_or(json!({}));
        let browsers = browsers_of(inner);
        let (name, pick) = match &inner.peers[&from].role {
            Role::Agent { name, pick } => (name.clone(), *pick),
            _ => return false,
        };
        let tx = inner.peers[&from].tx.clone();
        let pick = pick.filter(|p| browsers.iter().any(|b| b.id == *p));

        match tool {
            "list_browsers" => {
                let text = if browsers.is_empty() {
                    NONE_CONNECTED.to_string()
                } else {
                    browsers
                        .iter()
                        .map(|b| format!("{} {} (id {}){}", if Some(b.id) == pick { "*" } else { "-" }, b.label(), b.id, if b.email.is_empty() { String::new() } else { format!(", {}", b.email) }))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let _ = tx.send(reply(&id, text));
                false
            }
            "select_browser" => {
                let want = args["browser"].as_str().map(|s| s.to_lowercase()).or_else(|| args["browser"].as_u64().map(|n| n.to_string())).unwrap_or_default();
                let hit: Vec<&BrowserInfo> = browsers
                    .iter()
                    .filter(|b| b.id.to_string() == want || b.label().to_lowercase().contains(&want) || (!b.email.is_empty() && b.email.to_lowercase().contains(&want)))
                    .collect();
                match hit.as_slice() {
                    [b] => {
                        if let Some(Peer { role: Role::Agent { pick, .. }, .. }) = inner.peers.get_mut(&from) {
                            *pick = Some(b.id);
                        }
                        let _ = tx.send(reply(&id, format!("{name} now uses {}.", b.label())));
                    }
                    [] => {
                        let _ = tx.send(fail(&id, format!("No connected browser matches \"{want}\". {}", listing(&browsers))));
                    }
                    _ => {
                        let _ = tx.send(fail(&id, format!("\"{want}\" matches more than one browser; be more specific. {}", listing(&browsers))));
                    }
                }
                false
            }
            _ => {
                if inner.activity.get(&name).is_some_and(|a| a.paused) {
                    let _ = tx.send(fail(&id, format!("The user stopped {name}'s browser use from ByteHive. Don't use the browser; tell the user where you are and wait until they say to go on.")));
                    return false;
                }
                let target = match (pick, browsers.as_slice()) {
                    (Some(p), _) => p,
                    (None, [only]) => only.id,
                    (None, []) => {
                        let _ = tx.send(fail(&id, NONE_CONNECTED.to_string()));
                        return false;
                    }
                    (None, _) => {
                        let _ = tx.send(fail(&id, format!("More than one browser is connected; pick one with browser_select first. {}", listing(&browsers))));
                        return false;
                    }
                };
                // Running an older extension than the one on disk: update it
                // first (once per version); the agent simply tries again.
                if let Some(b) = browsers.iter().find(|b| b.id == target) {
                    if let Some(want) = (self.wanted_version)().filter(|w| !b.version.is_empty() && *w != b.version) {
                        if inner.reloaded.insert(format!("{}|{}|{}", b.browser, b.email, want)) {
                            let _ = inner.peers[&target].tx.send(RELOAD.to_string());
                            let _ = tx.send(fail(&id, format!("ByteHive is updating the browser extension to {want}. Try again in a few seconds.")));
                            return false;
                        }
                    }
                }
                let label = browsers.iter().find(|b| b.id == target).map(|b| b.label()).unwrap_or_default();
                let action = args["action"].as_str().unwrap_or("").to_string();
                inner.activity.insert(name.clone(), Activity { agent: name.clone(), browser: target, label, tool: tool.to_string(), action, at: now_ms(), paused: false });
                inner.next += 1;
                let hid = inner.next;
                let mut args = if args.is_object() { args } else { json!({}) };
                args["_ask"] = json!(inner.ask_risky);
                let call = json!({ "type": "call", "id": hid, "agent": name, "tool": tool, "args": args });
                inner.pending.insert(hid, Pending { asker: Asker::Agent(from, id.clone()), browser: target, agent: name.clone(), call: call.clone() });
                if inner.peers[&target].tx.send(call.to_string()).is_err() {
                    inner.pending.remove(&hid);
                    let _ = tx.send(fail(&id, "That browser just disconnected.".into()));
                }
                true
            }
        }
    }
}




fn listing(browsers: &[BrowserInfo]) -> String {
    let names: Vec<String> = browsers.iter().map(|b| format!("{} (id {})", b.label(), b.id)).collect();
    format!("Connected: {}.", names.join("; "))
}

/// One peer connection: a non-blocking loop that reads its messages into the
/// hub and writes whatever the hub queued for it.
pub(super) fn serve(hub: Arc<Hub>, stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let Ok(mut ws) = tungstenite::accept(stream) else { return };
    let (tx, rx) = mpsc::channel::<String>();
    let id = hub.add(tx);
    if ws.get_mut().set_nonblocking(true).is_err() {
        hub.remove(id);
        return;
    }
    // Idle naps grow from 1 to 20 ms: quick in a burst of calls, and about 50
    // wake-ups a second per peer when nothing happens (was 250).
    let mut nap = 1u64;
    'conn: loop {
        let mut idle = true;
        loop {
            match ws.read() {
                Ok(Message::Text(t)) => {
                    idle = false;
                    if !hub.on_message(id, &t) {
                        break 'conn;
                    }
                }
                Ok(Message::Close(_)) => break 'conn,
                Ok(_) => idle = false,
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break 'conn,
            }
        }
        while let Ok(out) = rx.try_recv() {
            idle = false;
            match ws.write(Message::Text(out)) {
                Ok(()) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                Err(_) => break 'conn,
            }
        }
        match ws.flush() {
            Ok(()) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break 'conn,
        }
        if idle {
            std::thread::sleep(Duration::from_millis(nap));
            nap = (nap * 2).min(20);
        } else {
            nap = 1;
        }
    }
    hub.remove(id);
}
