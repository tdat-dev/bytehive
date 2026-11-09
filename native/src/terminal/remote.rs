//! A terminal as the phone dashboard reaches it from its server threads: a
//! live copy of the CLI's output, typing into it, and the PTY sized to the
//! page's terminal while the page has it open (back to this window's size
//! when it lets go). Weak, so a stopped agent is never kept alive by it.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};

use agent_core::Agent;
use alacritty_terminal::event::WindowSize;

use super::Terminal;

#[derive(Clone)]
pub struct RemoteTerm {
    agent: Weak<Mutex<Agent>>,
    writer: Sender<Vec<u8>>,
    /// This window's grid, as it last fit.
    own_size: Arc<Mutex<WindowSize>>,
}

impl Terminal {
    pub fn remote(&self) -> RemoteTerm {
        RemoteTerm { agent: Arc::downgrade(&self.agent), writer: self.writer.clone(), own_size: self.cell_px.clone() }
    }
}

impl RemoteTerm {
    /// The CLI's output from now on, its recent scrollback first.
    pub fn tap(&self) -> Option<Receiver<Vec<u8>>> {
        let agent = self.agent.upgrade()?;
        let rx = agent.lock().unwrap_or_else(|p| p.into_inner()).tap();
        Some(rx)
    }

    /// Bytes to the CLI's input, in order with this window's own typing.
    pub fn write(&self, bytes: &[u8]) {
        let _ = self.writer.send(bytes.to_vec());
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 {
            return;
        }
        if let Some(agent) = self.agent.upgrade() {
            let _ = agent.lock().unwrap_or_else(|p| p.into_inner()).resize(cols, rows);
        }
    }

    /// The PTY back to this window's grid.
    pub fn restore(&self) {
        let size = *self.own_size.lock().unwrap_or_else(|p| p.into_inner());
        self.resize(size.num_cols, size.num_lines);
    }
}
