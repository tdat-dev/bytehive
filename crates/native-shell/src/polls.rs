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
        while let Ok((cwd, read)) = self.git_rx.try_recv() {
            // What was already dirty when each agent here first looked is not its work.
            let here = self.panes.iter().filter(|p| p.cli.is_some() && pane_cwd(p) == cwd).map(|p| (p.id.as_str(), p.started_ms));
            crate::touched::fill_baselines(&mut self.extras.git_base, &read, here);
            self.git_changed.insert(cwd.clone(), read.files.clone());
            self.extras.git_reads.insert(cwd.clone(), read);
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
                self.extras.git_reads.clear();
            }
            self.git_last = Instant::now();
            return;
        };
        self.git_pending.insert(cwd.clone());
        self.git_last = Instant::now();
        let tx = self.git_tx.clone();
        std::thread::spawn(move || {
            let read = crate::touched::read(std::path::Path::new(&cwd));
            let _ = tx.send((cwd, read));
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
            let state = pane_status(pane, now_ms(), self.changed_count(pane)).state;
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
                let paths = self.touched_files(p).unwrap_or_default();
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

    /// The changed files that are this agent's own work (touched.rs), none
    /// until its folder was read.
    pub(crate) fn touched_files(&self, p: &crate::term_pane::Pane) -> Option<Vec<String>> {
        let read = self.extras.git_reads.get(&pane_cwd(p))?;
        Some(crate::touched::touched(read, p.started_ms, self.extras.git_base.get(&p.id)))
    }

    /// How many files this agent changed: what its state reads as Review.
    pub(crate) fn changed_count(&self, p: &crate::term_pane::Pane) -> Option<u32> {
        self.touched_files(p).map(|f| f.len() as u32)
    }

    /// [`Self::changed_count`] for every pane, by pane id.
    pub(crate) fn changed_counts(&self) -> HashMap<String, u32> {
        self.panes.iter().filter_map(|p| Some((p.id.clone(), self.changed_count(p)?))).collect()
    }
}