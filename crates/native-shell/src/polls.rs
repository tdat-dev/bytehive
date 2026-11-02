//! Background polls: git paths, stuck verdicts, and the clash map they feed.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use supervise::clash::{clash_notes, ClashInput, ClashNote};
use supervise::stuck::{DiffPrint, StuckSnap};

use supervise::types::ChatItem;

use crate::app::{now_ms, ShellApp};
use crate::history_ui::{parse_history, read_history};
use crate::states::{pane_cwd, pane_status};

pub(crate) struct StuckRead {
    pub(crate) pane_id: String,
    pub(crate) run: u64,
    pub(crate) items: Vec<ChatItem>,
    pub(crate) diff: Option<(u64, u64, u64)>,
}

impl ShellApp {
    pub(crate) fn poll_git(&mut self) {
        while let Ok((cwd, files)) = self.git_rx.try_recv() {
            self.git_changed.insert(cwd.clone(), files);
            self.git_pending.remove(&cwd);
        }
        if self.git_last.elapsed() < Duration::from_secs(5) {
            return;
        }
        let next = self.panes.iter().map(pane_cwd).find(|cwd| !self.git_pending.contains(cwd) && !self.git_changed.contains_key(cwd));
        let Some(cwd) = next else {
            // Everything known: re-read the stalest next round.
            if self.git_pending.is_empty() {
                self.git_changed.clear();
            }
            self.git_last = Instant::now();
            return;
        };
        self.git_pending.insert(cwd.clone());
        self.git_last = Instant::now();
        let tx = self.git_tx.clone();
        std::thread::spawn(move || {
            let files = agent_core::git::changed_files(std::path::Path::new(&cwd));
            let _ = tx.send((cwd, files));
        });
    }

    /// Stuck verdicts per agent pane: transcript plus diff fingerprint, read
    /// off the UI thread one pane per round (~10s cadence).
    pub(crate) fn poll_stuck(&mut self) {
        while let Ok(read) = self.stuck_rx.try_recv() {
            self.stuck_pending.remove(&read.pane_id);
            let Some(pane) = self.panes.iter().find(|p| p.id == read.pane_id) else { continue };
            if pane.run != read.run {
                continue; // restarted while reading
            }
            let changed = self.git_changed.get(&pane_cwd(pane)).map(|f| f.len() as u32);
            let state = pane_status(pane, now_ms(), changed).state;
            let snap = StuckSnap {
                run: read.run,
                state,
                items: read.items,
                diff: read.diff.map(|(n, add, del)| DiffPrint { n, add, del }),
                now: now_ms(),
            };
            match self.stuck_watch.step(&read.pane_id, &snap) {
                Some(e) => {
                    self.stuck_evidence.insert(read.pane_id, e);
                }
                None => {
                    self.stuck_evidence.remove(&read.pane_id);
                }
            }
        }
        if self.stuck_last.elapsed() < Duration::from_secs(10) {
            return;
        }
        self.stuck_last = Instant::now();
        let next = self.panes.iter().find(|p| p.cli.is_some() && !self.stuck_pending.contains(&p.id));
        let Some(pane) = next else { return };
        let (id, run, cli, dir, session) = (pane.id.clone(), pane.run, pane.cli.unwrap(), pane_cwd(pane), pane.session_id.clone());
        self.stuck_pending.insert(id.clone());
        let tx = self.stuck_tx.clone();
        std::thread::spawn(move || {
            let chunk = read_history(cli, &dir, session.as_deref());
            let (items, _) = parse_history(cli, &chunk.text);
            let diff = agent_core::git::diff_stat(std::path::Path::new(&dir));
            let _ = tx.send(StuckRead { pane_id: id, run, items, diff });
        });
    }

    /// File-clash warnings across running panes, from the polled git paths.
    pub(crate) fn clash_map(&self) -> HashMap<String, ClashNote> {
        let inputs: Vec<ClashInput> = self
            .panes
            .iter()
            // A plain shell edits nothing of its own, so it overlaps no one.
            .filter(|p| p.cli.is_some())
            .map(|p| {
                let paths = self.git_changed.get(&pane_cwd(p)).cloned().unwrap_or_default();
                let status = pane_status(p, now_ms(), Some(paths.len() as u32));
                ClashInput {
                    pane_id: p.id.clone(),
                    name: p.name.clone(),
                    ws_id: pane_cwd(p),
                    state: status.state,
                    running: p.exit_code.is_none(),
                    title: None,
                    race: None,
                    paths,
                }
            })
            .collect();
        clash_notes(&inputs)
    }
}