//! Every 30 s: who has gone quiet, and how many tokens each Job has used
//! (engine.ts watch, watchedCards, measure).

use super::super::alerts::{Alert, Level};
use super::super::host::Host;
use super::super::org::{floor_contact, person_by_id};
use super::super::reduce::{Effect, join_job, pause_job, paused_note};
use super::super::types::{Job, JobStatus};
use super::super::watch::{CardsIn, WatchList, WatchedCard, WorkIn, cards_to_watch, watch_budget, watch_work};
use super::{Engine, JobCards};
use crate::board::List;
use std::collections::{BTreeMap, HashSet};

/// Output this recent means someone is working a card.
const WORKING_WINDOW_MS: i64 = 60_000;

/// What everyone on a Job has added in fresh tokens since it last looked. Each
/// person's last-seen figure is kept in tokenBase and only what they add on top
/// is counted, so a closed seat costs nothing already counted, and a session that
/// starts over is rebased rather than subtracted. A person whose usage has not
/// been read yet is left out until it has. Gives back the same Job when nothing moved.
pub(crate) fn measure(job: &Job, usage: impl Fn(&str) -> Option<Option<u64>>) -> Job {
    if job.status != JobStatus::Running && job.status != JobStatus::Paused {
        return job.clone();
    }
    let mut seen = job.token_base.clone();
    let mut added = 0;
    let mut moved = false;
    for id in &job.people {
        let Some(read) = usage(id) else { continue };
        // No figure for a person seen before is a read that came back empty, not a restart.
        if read.is_none() && seen.contains_key(id) {
            continue;
        }
        let fresh = read.unwrap_or(0);
        match seen.get(id) {
            Some(&was) if was == fresh => continue,
            Some(&was) if fresh > was => added += fresh - was,
            _ => {}
        }
        seen.insert(id.clone(), fresh);
        moved = true;
    }
    if !moved {
        return job.clone();
    }
    Job { token_base: seen, tokens_used: job.tokens_used + added, ..job.clone() }
}

/// One board's cards by the Job they belong to: how many are done, of how many (cardsOnBoard).
pub(crate) fn cards_on_board(lists: &[List]) -> BTreeMap<String, JobCards> {
    let mut by_job: BTreeMap<String, JobCards> = BTreeMap::new();
    for l in lists {
        let done = l.title.trim().to_lowercase() == "done";
        for job in l.cards.iter().filter_map(|c| c.job.as_ref()).filter(|j| !j.is_empty()) {
            let t = by_job.entry(job.clone()).or_default();
            t.1 += 1;
            if done {
                t.0 += 1;
            }
        }
    }
    by_job
}

impl Engine {
    /// The cards that could go quiet on every floor with people.
    fn watched_cards(&mut self, host: &mut dyn Host) -> Vec<WatchedCard> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut read_all = true;
        let now = host.now_ms();
        let floors = self.floors_with_people();
        self.floor_cards.retain(|id, _| floors.iter().any(|f| &f.id == id));
        for floor in floors {
            // A read that failed keeps the last count; a board that is gone has none.
            let board = match host.read_board(&floor.dir) {
                Ok(Some((b, _))) => b,
                Ok(None) => {
                    self.floor_cards.remove(&floor.id);
                    continue;
                }
                Err(_) => {
                    read_all = false;
                    continue;
                }
            };
            self.floor_cards.insert(floor.id.clone(), cards_on_board(&board.lists));
            let Some(s) = &self.state else { break };
            let contact = floor_contact(&s.company, &floor.id).map(|p| p.id.clone());
            let working = |id: &str| host.seat(id).is_some_and(|x| x.running && now - x.last_output_at < WORKING_WINDOW_MS);
            let r = cards_to_watch(&CardsIn { lists: &board.lists, jobs: &s.jobs, contact: contact.as_deref(), is_working: &working, now }, &mut self.quiet);
            out.extend(r.cards);
            seen.extend(r.seen);
        }
        // A card that left its list starts its quiet time over when it comes back.
        if read_all {
            self.quiet.retain(|k, _| seen.contains(k));
        }
        out
    }

    pub(crate) fn watch(&mut self, host: &mut dyn Host) {
        if self.state.is_none() {
            return;
        }
        let cards = self.watched_cards(host);
        let Some(s) = self.state.clone() else { return };
        let boss = |id: &str| person_by_id(&s.company, id).and_then(|p| p.boss.clone());
        let work = watch_work(&WorkIn { cards: &cards, nudges: &self.nudges, limits: &s.company.limits, boss_of: &boss, now: host.now_ms() });
        self.nudges = work.nudges;
        let mut next: Vec<Alert> = work.alerts;
        let before = s.jobs.clone();
        // A plan can't name who a Lead hands a card to: they join when they pick it up.
        let mut state = cards.iter().filter(|c| c.list == WatchList::Doing).fold(s, |st, c| join_job(&st, &c.job, &c.assignee_id));
        state.jobs = state.jobs.iter().map(|j| measure(j, |id| host.seat(id).and_then(|x| x.fresh_tokens))).collect();
        let mut paused = Vec::new();
        for job in state.jobs.clone() {
            let b = watch_budget(&job);
            if b.pause {
                state = pause_job(&state, &job.id);
                paused.push(paused_note(&job.id));
            } else if let Some(a) = b.alert {
                next.push(a);
            }
        }
        // watch_budget only looks at a running Job: a paused one keeps its stop alert until it is resumed.
        for job in &state.jobs {
            if job.status == JobStatus::Paused {
                let stop = watch_budget(&Job { status: JobStatus::Running, ..job.clone() }).alert;
                if let Some(a) = stop.filter(|a| a.level == Level::Stop) {
                    next.push(a);
                }
            }
        }
        let moved: Vec<Job> = state.jobs.iter().enumerate().filter(|(i, j)| before.get(*i) != Some(*j)).map(|(_, j)| j.clone()).collect();
        let same = next == self.alerts;
        self.alerts = next;
        self.state = Some(state);
        if self.remember(host) || !moved.is_empty() {
            self.try_persist(host, &moved);
        } else if !same {
            self.changed();
        }
        let delivers = work.delivers.into_iter().map(|d| Effect::Deliver { person_id: d.person_id, text: d.text });
        self.run(host, delivers.chain(paused).collect());
    }
}
