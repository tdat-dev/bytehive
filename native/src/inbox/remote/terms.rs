//! The running agents' terminals, as the dashboard's server threads reach
//! them. The Inbox hands over a fresh list with every fleet push.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use crate::terminal::RemoteTerm;

#[derive(Default)]
pub struct FleetTerms {
    live: Mutex<HashMap<String, RemoteTerm>>,
    /// Agents whose PTY a page sized to its own terminal, and when.
    sized: Mutex<HashMap<String, Instant>>,
}

/// A page switching agents closes one socket and sizes the next at once;
/// a close heard this soon after a resize is that switch, not a goodbye.
const SWITCH: Duration = Duration::from_millis(400);

impl FleetTerms {
    /// The running agents now, by id.
    pub fn set(&self, live: HashMap<String, RemoteTerm>) {
        *self.live.lock().unwrap_or_else(|p| p.into_inner()) = live;
    }

    fn get(&self, id: &str) -> Option<RemoteTerm> {
        self.live.lock().unwrap_or_else(|p| p.into_inner()).get(id).cloned()
    }
}

impl dashboard::Terminals for FleetTerms {
    fn tap(&self, id: &str) -> Option<Receiver<Vec<u8>>> {
        self.get(id)?.tap()
    }

    fn write(&self, id: &str, bytes: &[u8]) {
        if let Some(t) = self.get(id) {
            t.write(bytes);
        }
    }

    fn resize(&self, id: &str, cols: u16, rows: u16) {
        if let Some(t) = self.get(id) {
            t.resize(cols, rows);
            self.sized.lock().unwrap_or_else(|p| p.into_inner()).insert(id.to_string(), Instant::now());
        }
    }

    /// The page let go: the PTY goes back to this window's size.
    fn release(&self, id: &str) {
        let mut sized = self.sized.lock().unwrap_or_else(|p| p.into_inner());
        if sized.get(id).is_some_and(|at| at.elapsed() >= SWITCH) {
            sized.remove(id);
            drop(sized);
            if let Some(t) = self.get(id) {
                t.restore();
            }
        }
    }
}
