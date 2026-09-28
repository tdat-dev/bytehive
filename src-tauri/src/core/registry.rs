//! Holds MANY concurrently-running agents keyed by id. Killing an agent simply
//! removes it from the map — its `Drop` performs the (non-blocking) teardown.
//!
//! The app shares one registry behind a mutex that every keystroke, resize and
//! dashboard request also takes. So `spawn_shared` starts the ConPTY with the
//! lock released, and callers take agents OUT under the lock and drop them
//! after releasing it (`take`, `drain`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{anyhow, Result};
use portable_pty::PtySize;

use crate::core::agent::{Agent, Sink};
use crate::core::command_spec::CommandSpec;

/// After an agent's process exits, how long its output reader gets to drain the
/// last bytes (a final error message) before the ConPTY is closed.
const EXIT_DRAIN: Duration = Duration::from_millis(300);

struct Entry {
    /// Which spawn this is: an id restarted in place gets a new run, so the old
    /// process's late exit can't remove the new one.
    run: u64,
    agent: Agent,
}

#[derive(Default)]
pub struct Registry {
    agents: HashMap<String, Entry>,
}

/// Lock a shared registry, surviving a panic elsewhere (poison) rather than
/// wedging every terminal after it.
pub fn lock(registry: &Mutex<Registry>) -> MutexGuard<'_, Registry> {
    registry.lock().unwrap_or_else(|p| p.into_inner())
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self) -> usize {
        self.agents.len()
    }

    pub fn has(&self, id: &str) -> bool {
        self.agents.contains_key(id)
    }

    /// Spawn a new agent under `id`. Rejects a duplicate id.
    pub fn spawn(
        &mut self,
        id: String,
        spec: &CommandSpec,
        size: PtySize,
        on_bytes: impl FnMut(&[u8]) + Send + 'static,
        on_exit: impl FnOnce(u32) + Send + 'static,
    ) -> Result<()> {
        if self.agents.contains_key(&id) {
            return Err(anyhow!("agent '{id}' already exists"));
        }
        let agent = Agent::spawn(spec, size, on_bytes, on_exit)?;
        self.agents.insert(id, Entry { run: 0, agent });
        Ok(())
    }

    /// Spawn `id` (run number `run`) into the shared `registry`:
    /// - the ConPTY starts WITHOUT the lock held, so typing into, resizing or
    ///   watching every other agent doesn't stall while this one boots;
    /// - when its process exits, `on_exit` fires and then the agent is taken
    ///   out of the registry and torn down (ConPTY, job, reader thread,
    ///   scrollback), unless the id has been re-spawned since.
    pub fn spawn_shared(
        registry: &Arc<Mutex<Registry>>,
        id: String,
        run: u64,
        spec: &CommandSpec,
        size: PtySize,
        on_bytes: impl FnMut(&[u8]) + Send + 'static,
        on_exit: impl FnOnce(u32) + Send + 'static,
    ) -> Result<()> {
        let exists = || anyhow!("agent '{id}' already exists");
        if lock(registry).has(&id) {
            return Err(exists()); // before starting a process for nothing
        }
        let exited = Arc::new(AtomicBool::new(false));
        let (flag, weak, exit_id) = (exited.clone(), Arc::downgrade(registry), id.clone());
        let agent = Agent::spawn(spec, size, on_bytes, move |code| {
            flag.store(true, Ordering::SeqCst);
            on_exit(code);
            std::thread::sleep(EXIT_DRAIN);
            if let Some(reg) = weak.upgrade() {
                let gone = lock(&reg).take_run(&exit_id, run);
                drop(gone); // teardown outside the lock
            }
        })?;
        let mut reg = lock(registry);
        if reg.has(&id) {
            drop(reg);
            drop(agent);
            return Err(exists());
        }
        reg.agents.insert(id.clone(), Entry { run, agent });
        // Exited before it was in the map: its own removal may have missed it.
        if exited.load(Ordering::SeqCst) {
            let gone = reg.take_run(&id, run);
            drop(reg);
            drop(gone);
        }
        Ok(())
    }

    fn get(&self, id: &str) -> Result<&Agent> {
        self.agents
            .get(id)
            .map(|e| &e.agent)
            .ok_or_else(|| anyhow!("no agent '{id}'"))
    }

    /// Re-point a running agent's output at a new sink (window detach).
    /// Replays the buffered scrollback through the new sink first.
    pub fn attach(&self, id: &str, sink: Sink) -> Result<()> {
        self.get(id)?.attach(sink);
        Ok(())
    }

    /// Subscribe to an agent's live output (remote web terminal). Scrollback is
    /// replayed into the returned channel first.
    pub fn tap(&self, id: &str) -> Result<std::sync::mpsc::Receiver<Vec<u8>>> {
        Ok(self.get(id)?.tap())
    }

    /// Start recording an agent's output to `path` (see `Agent::record_start`).
    pub fn record_start(&self, id: &str, path: &str) -> Result<()> {
        self.get(id)?.record_start(path)
    }

    /// Stop recording an agent's output. No-op if the agent is gone.
    pub fn record_stop(&self, id: &str) {
        if let Ok(a) = self.get(id) {
            a.record_stop();
        }
    }

    pub fn write_input(&mut self, id: &str, bytes: &[u8]) -> Result<()> {
        self.agents
            .get_mut(id)
            .ok_or_else(|| anyhow!("no agent '{id}'"))?
            .agent
            .write_input(bytes)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        self.get(id)?.resize(cols, rows)
    }

    /// Kill (remove) one agent. No-op if absent. Drop tears the agent down.
    pub fn kill(&mut self, id: &str) {
        self.agents.remove(id);
    }

    /// Kill every agent (each Drop tears its tree down).
    pub fn clear(&mut self) {
        self.agents.clear();
    }

    /// Take one agent out; dropping the result (after releasing the lock) kills it.
    pub fn take(&mut self, id: &str) -> Option<Agent> {
        self.agents.remove(id).map(|e| e.agent)
    }

    /// Take every agent out; dropping the result kills them all.
    pub fn drain(&mut self) -> Vec<Agent> {
        self.agents.drain().map(|(_, e)| e.agent).collect()
    }

    /// Take `id` out only if it is still run `run`.
    fn take_run(&mut self, id: &str, run: u64) -> Option<Agent> {
        if self.agents.get(id)?.run != run {
            return None;
        }
        self.take(id)
    }
}
