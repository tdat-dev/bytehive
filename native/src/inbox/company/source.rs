//! The one place the page meets the engine (office/source.ts): it starts and
//! ticks the engine over the fleet, runs the owner's actions, and turns what
//! the engine holds plus every person's seat into this frame's office.

use std::collections::BTreeMap;

use crate::company::engine::Engine;
use crate::company::fleet_host::FleetHost;
use crate::company::host::Host;

use super::model::{Pane, office_model};
use super::*;

/// The engine's heartbeat: the lock, requests, and what waits to be typed.
const BEAT_MS: u64 = 1500;

impl Inbox {
    /// Run `f` on the engine with the fleet as its host. None while there is no engine.
    pub(super) fn co_engine<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Engine, &mut dyn Host) -> R) -> Option<R> {
        let Inbox { company, fleet, .. } = self;
        let engine = company.engine.as_mut()?;
        let mut host = FleetHost::new(fleet, cx);
        let out = f(engine, &mut host);
        Some(out)
    }

    /// From the Inbox's timer: start the engine the first time, then beat every 1.5 s.
    pub(in crate::inbox) fn company_tick(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        if now.saturating_sub(self.company.ticked) < BEAT_MS {
            return;
        }
        self.company.ticked = now;
        let before = self.company.engine.as_ref().map(|e| e.changes);
        if self.company.engine.is_none() {
            let Some(root) = crate::company::store::root() else { return };
            let Inbox { company, fleet, .. } = self;
            let mut host = FleetHost::new(fleet, cx);
            let engine = Engine::start(&root, &mut host);
            // Opening the app with a Job on disk lands on the Company.
            company.land = engine.state().is_some_and(|s| !s.jobs.is_empty());
            company.engine = Some(engine);
            cx.notify();
            return;
        }
        self.co_engine(cx, |e, host| e.tick(host));
        if self.company.open && self.company.engine.as_ref().map(|e| e.changes) != before {
            cx.notify();
        }
    }

    /// Once, after the engine first read the company.
    pub(in crate::inbox) fn company_land(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.company.land) {
            self.open_company(window, cx);
        }
    }

    /// This frame's office, from the engine and every person's seat.
    pub(super) fn company_snapshot(&self, cx: &App) -> Snapshot {
        let Some(engine) = &self.company.engine else {
            return Snapshot { model: None, note: Some(words::STARTING.into()), error: None };
        };
        let panes: Vec<Pane> = self
            .fleet
            .seats
            .iter()
            .filter_map(|s| {
                let id = s.spec.person.clone()?;
                Some(Pane { person_id: id, status: s.status(cx), doing: s.spec.title.clone() })
            })
            .collect();
        let merged = |id: &str| panes.iter().any(|p| p.person_id == id && p.status.state == State::Merged);
        let view = engine.view(merged);
        let Some(company) = &view.company else {
            let note = view.locked_by_other.then(|| words::LOCKED.to_string());
            return Snapshot { model: None, note: note.or_else(|| view.error.is_none().then(|| words::STARTING.into())), error: view.error.clone() };
        };
        let roles = engine.state().map(|s| s.roles.clone()).unwrap_or_default();
        // Someone who never gave a job keeps a quiet rail.
        let alerts = if view.jobs.is_empty() { Vec::new() } else { view.alerts.clone() };
        let open: Vec<String> = self.fleet.seats.iter().map(|s| s.spec.cwd.clone()).collect();
        let model = office_model(company, &view.jobs, &alerts, &panes, &roles, &BTreeMap::new(), &open);
        Snapshot { model: Some(model), note: None, error: view.error.clone() }
    }
}
