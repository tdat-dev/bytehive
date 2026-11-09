//! What the dashboard needs of the host app's terminals: a live copy of an
//! agent's output, typing into it, and sizing it to the page's terminal.
//! The host owns its agents; the server only reaches them through this.

use std::sync::mpsc::Receiver;
use std::sync::Arc;

pub trait Terminals: Send + Sync {
    /// The agent's output from now on, its recent scrollback first. None when
    /// no agent of that id runs.
    fn tap(&self, id: &str) -> Option<Receiver<Vec<u8>>>;
    /// Bytes straight to the agent's input, as typed on the page.
    fn write(&self, id: &str, bytes: &[u8]);
    /// The page's terminal is `cols` by `rows`.
    fn resize(&self, id: &str, cols: u16, rows: u16);
    /// A page's terminal of this agent closed.
    fn release(&self, _id: &str) {}
}

pub type SharedTerminals = Arc<dyn Terminals>;
