//! The one place the page meets the engine (office/source.ts): it starts and
//! ticks the engine over the fleet, runs the owner's actions, carries out what
//! the host hands back (messages for the typer, worktrees made off the UI
//! thread), and turns the engine's view plus every seat into this frame's office.

use std::collections::HashMap;

use crate::company::engine::Engine;
use crate::company::fleet_host::{FleetHost, Outbound, fresh_tokens};
use crate::company::host::{Host, Project};

use super::model::{Pane, office_model};
use super::*;

/// The engine's heartbeat: the lock, requests, and what waits to be typed.
const BEAT_MS: u64 = 1500;
/// How often what each person has used is read again, off the UI thread.
const FRESH_MS: u64 = 30_000;

impl Inbox {
    /// Every open project, the current one first, and the current one.
    fn co_projects(&self) -> (Vec<Project>, Option<Project>) {
        let project = |dir: &str| Project { name: crate::fleet::project_of(dir), dir: Some(dir.to_string()) };
        let active = self.prefs.project.as_deref().map(project);
        let mut dirs: Vec<&str> = self.prefs.project.iter().map(String::as_str).collect();
        for s in &self.fleet.seats {
            if !dirs.iter().any(|d| crate::presets::same_folder(d, &s.spec.cwd)) {
                dirs.push(&s.spec.cwd);
            }
        }
        (dirs.into_iter().map(project).collect(), active)
    }

    /// Run `f` on the engine with the fleet as its host, then carry out what it
    /// handed back. None while there is no engine.
    pub(super) fn co_engine<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Engine, &mut dyn Host) -> R) -> Option<R> {
        let (projects, active) = self.co_projects();
        let Inbox { company, fleet, .. } = self;
        let engine = company.engine.as_mut()?;
        let mut host = FleetHost { fleet, cx, projects, active, fresh: &company.fresh, out: Outbound::default() };
        let r = f(engine, &mut host);
        let out = std::mem::take(&mut host.out);
        self.co_carry_out(out, cx);
        Some(r)
    }

    /// Messages go to the typer, which waits for a quiet prompt; a new hire's
    /// worktree is made at the next frame, which has a window to start it in.
    fn co_carry_out(&mut self, out: Outbound, cx: &mut Context<Self>) {
        for (id, text) in out.typed {
            self.queue_message(id, text, cx);
        }
        if !out.isolate.is_empty() {
            self.company.isolate.extend(out.isolate);
            cx.notify();
        }
    }

    /// From the Inbox's timer: start the engine the first time, then beat every 1.5 s.
    pub(in crate::inbox) fn company_tick(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        if super::demo::wanted() {
            if self.company.demo.is_none() {
                self.company.demo = Some(super::demo::Demo::new());
                self.company.land = true;
                cx.notify();
            }
            return;
        }
        self.co_read_fresh(now, cx);
        if now.saturating_sub(self.company.ticked) < BEAT_MS {
            return;
        }
        self.company.ticked = now;
        if self.company.engine.is_none() {
            let Some(root) = crate::company::store::root() else { return };
            let (projects, active) = self.co_projects();
            let Inbox { company, fleet, .. } = self;
            let mut host = FleetHost { fleet, cx, projects, active, fresh: &company.fresh, out: Outbound::default() };
            let engine = Engine::start(&root, &mut host);
            let out = std::mem::take(&mut host.out);
            // Opening the app with a Job on disk lands on the Company.
            company.land = !engine.view().jobs.is_empty();
            company.engine = Some(engine);
            self.co_carry_out(out, cx);
            cx.notify();
            return;
        }
        let before = self.company.engine.as_ref().map(|e| e.changes);
        self.co_engine(cx, |e, host| e.tick(host));
        if self.company.open && self.company.engine.as_ref().map(|e| e.changes) != before {
            cx.notify();
        }
    }

    /// What each person has used so far, read off the UI thread every 30 s.
    fn co_read_fresh(&mut self, now: u64, cx: &mut Context<Self>) {
        if self.company.fresh_busy || now.saturating_sub(self.company.fresh_at) < FRESH_MS || self.company.engine.is_none() {
            return;
        }
        let asks: Vec<(String, String, String, Option<String>, u64)> = self
            .fleet
            .seats
            .iter()
            .filter_map(|s| {
                let id = s.spec.person.clone()?;
                let dir = s.spec.worktree.clone().unwrap_or_else(|| s.spec.cwd.clone());
                Some((id, s.spec.cli.clone(), dir, s.spec.session_id.clone(), s.spec.started_ms))
            })
            .collect();
        if asks.is_empty() {
            return;
        }
        self.company.fresh_busy = true;
        self.company.fresh_at = now;
        let read = cx.background_spawn(async move {
            asks.into_iter().map(|(id, cli, dir, session, since)| (id, fresh_tokens(&cli, &dir, session.as_deref(), since))).collect::<HashMap<_, _>>()
        });
        cx.spawn(async move |this, cx| {
            let fresh = read.await;
            let _ = this.update(cx, |this, _| {
                this.company.fresh_busy = false;
                this.company.fresh.extend(fresh);
            });
        })
        .detach();
    }

    /// Once a frame: land on the Company after the engine first read a Job,
    /// and give new hires their worktrees.
    pub(in crate::inbox) fn company_land(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.company.land) {
            self.open_company(window, cx);
        }
        let waiting = std::mem::take(&mut self.company.isolate);
        let mut by_dir: Vec<(String, Vec<String>)> = Vec::new();
        for (dir, id) in waiting {
            match by_dir.iter_mut().find(|(d, _)| *d == dir) {
                Some((_, ids)) => ids.push(id),
                None => by_dir.push((dir, vec![id])),
            }
        }
        for (dir, ids) in by_dir {
            self.isolate(dir, ids, window, cx);
        }
    }

    /// This frame's office, from the engine and every person's seat.
    pub(super) fn company_snapshot(&self, cx: &App) -> Snapshot {
        if let Some(d) = &self.company.demo {
            return Snapshot { model: Some(d.model()), note: None, error: None };
        }
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
        let view = engine.view();
        let Some(company) = &view.company else {
            let note = view.locked_by_other.then(|| words::LOCKED.to_string());
            return Snapshot { model: None, note: note.or_else(|| view.error.is_none().then(|| words::STARTING.into())), error: view.error.clone() };
        };
        // Someone who never gave a job keeps a quiet rail.
        let alerts = if view.jobs.is_empty() { Vec::new() } else { view.alerts.clone() };
        let (projects, _) = self.co_projects();
        let open: Vec<String> = projects.into_iter().filter_map(|p| p.dir).collect();
        let model = office_model(company, &view.jobs, &alerts, &panes, &view.roles, &view.job_cards, &open);
        Snapshot { model: Some(model), note: None, error: view.error.clone() }
    }
}
