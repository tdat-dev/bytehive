//! The board's background timer (kanban.ts's watch interval): every 3.5 s,
//! open or not, the current project's plan.json is imported and the
//! conductor feeds its agents (board/round.rs does the files, off the UI
//! thread; the cards are typed in here). The mode is kept per folder.

use std::collections::{HashMap, HashSet};

use crate::board::round;
use crate::conductor::{Agent, Mode};

use super::keep;
use super::*;

/// kanban.ts's interval.
const EVERY_MS: u64 = 3500;

#[derive(Default)]
pub(in crate::inbox) struct Conductor {
    /// The project followed; its mode and seen titles are loaded with it.
    dir: Option<String>,
    pub(super) mode: Mode,
    seen: HashSet<String>,
    plan_mtime: Option<SystemTime>,
    /// Pipeline: card id → the stage it was dispatched into.
    assigned: HashMap<String, String>,
    seed: bool,
    /// A round is out; one at a time.
    busy: bool,
    /// Bumped when the mode or project changes: an older round's moves are dropped.
    era: u64,
    last: u64,
}

fn runs(seat: &Seat, cx: &App) -> bool {
    seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none())
}

impl Inbox {
    /// From the Inbox tick.
    pub(in crate::inbox) fn conductor_tick(&mut self, cx: &mut Context<Self>) {
        if !self.conductor.busy && now_ms().saturating_sub(self.conductor.last) >= EVERY_MS {
            self.conductor_round(cx);
        }
    }

    /// The current project, its mode and seen titles loaded when it changed
    /// (setContext restores them).
    pub(super) fn conductor_follow(&mut self) -> Option<String> {
        let now = self.project_dir();
        let c = &mut self.conductor;
        let same = match (&c.dir, &now) {
            (Some(a), Some(n)) => presets::same_folder(a, n),
            (a, n) => a.is_none() && n.is_none(),
        };
        if !same {
            c.mode = now.as_deref().map(keep::load_mode).unwrap_or_default();
            c.seen = now.as_deref().map(keep::load_seen).unwrap_or_default();
            c.dir = now;
            c.plan_mtime = None;
            c.assigned.clear();
            c.seed = c.mode == Mode::Pipeline;
            c.era += 1;
        }
        c.dir.clone()
    }

    /// Off goes on, or the mode moves on: saved, and acted on at once.
    pub(super) fn conductor_set(&mut self, mode: Mode, cx: &mut Context<Self>) {
        let Some(dir) = self.conductor_follow() else { return };
        let c = &mut self.conductor;
        c.mode = mode;
        keep::save_mode(&dir, mode);
        // Entering pipeline: in-flight memory from cards already assigned.
        // Leaving: forget it.
        c.assigned.clear();
        c.seed = mode == Mode::Pipeline;
        c.era += 1;
        c.last = 0;
        if !c.busy {
            self.conductor_round(cx);
        }
        cx.notify();
    }

    fn conductor_round(&mut self, cx: &mut Context<Self>) {
        self.conductor.last = now_ms();
        let Some(dir) = self.conductor_follow() else { return };
        let agents = self.fleet.seats.iter().filter(|s| presets::same_folder(&s.spec.cwd, &dir)).map(|s| Agent { id: s.spec.id.clone(), name: s.spec.name.clone(), running: runs(s, cx) }).collect();
        let c = &mut self.conductor;
        c.busy = true;
        let era = c.era;
        let ask = round::Ask { dir: dir.clone(), mode: c.mode, agents, seen: c.seen.clone(), plan_mtime: c.plan_mtime, assigned: c.assigned.clone(), seed: c.seed };
        let work = cx.background_spawn(async move { round::run(ask) });
        cx.spawn(async move |this, cx| {
            let got = work.await;
            let _ = this.update(cx, |this, cx| this.conductor_after(dir, era, got, cx));
        })
        .detach();
    }

    /// The round back: what was imported is remembered, the cards are typed
    /// into agents still running, then claimed on the board.
    fn conductor_after(&mut self, dir: String, era: u64, got: round::Round, cx: &mut Context<Self>) {
        let c = &mut self.conductor;
        if !c.dir.as_ref().is_some_and(|d| presets::same_folder(d, &dir)) {
            c.busy = false;
            return;
        }
        c.plan_mtime = got.plan_mtime;
        if !got.imported.is_empty() {
            c.seen.extend(got.imported);
            keep::save_seen(&dir, &c.seen);
        }
        if c.era != era {
            c.busy = false;
            return;
        }
        c.assigned = got.assigned;
        if got.seeded {
            c.seed = false;
        }
        let mut sent = Vec::new();
        for s in got.sends {
            if !self.fleet.seats.iter().any(|seat| seat.spec.id == s.agent && runs(seat, cx)) {
                continue;
            }
            self.queue_message(s.agent.clone(), s.prompt.clone(), cx);
            if let Some(stage) = &s.stage {
                self.conductor.assigned.insert(s.card.clone(), stage.clone());
            }
            sent.push(s);
        }
        if sent.is_empty() {
            self.conductor.busy = false;
            return;
        }
        let board = got.board;
        let claim = cx.background_spawn(async move { round::claim(&dir, &board, &sent) });
        cx.spawn(async move |this, cx| {
            claim.await;
            let _ = this.update(cx, |this, _| this.conductor.busy = false);
        })
        .detach();
    }
}
