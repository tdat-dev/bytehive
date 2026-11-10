//! Agents do not report back on their own, and managers loop. These checks are
//! what stops a Job from quietly stalling or burning tokens (watch.ts).

use super::alerts::{Alert, Level};
use super::types::{Job, JobStatus, Limits};
use crate::board::{Card, List};
use std::collections::HashMap;

/// Which list a watched card sits in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchList {
    Doing,
    Todo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchedCard {
    pub id: String,
    pub title: String,
    pub job: String,
    pub assignee_id: String,
    /// When the person went idle, None while working.
    pub quiet_since: Option<i64>,
    pub returns: u32,
    pub list: WatchList,
}

/// A message for a person: the watcher's nudge, delivered as the reducer's deliver effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deliver {
    pub person_id: String,
    pub text: String,
}

pub struct CardsIn<'a> {
    /// One board's lists.
    pub lists: &'a [List],
    pub jobs: &'a [Job],
    /// Who a To do card with nobody on it falls to: the floor's contact.
    pub contact: Option<&'a str>,
    pub is_working: &'a dyn Fn(&str) -> bool,
    pub now: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Watched {
    pub cards: Vec<WatchedCard>,
    /// Every quiet-time key read, `doing:<id>` and `todo:<id>`, for clearing old ones.
    pub seen: Vec<String>,
}

fn cards_of<'a>(lists: &'a [List], title: &str) -> &'a [Card] {
    lists.iter().find(|l| l.title.trim().to_lowercase() == title).map_or(&[], |l| l.cards.as_slice())
}

/// A field the board left empty counts as missing, as JavaScript's `!value` does.
fn given(s: Option<&str>) -> Option<&str> {
    s.filter(|s| !s.is_empty())
}

fn returns(c: &Card) -> u32 {
    c.returns.unwrap_or(0.0) as u32
}

/// The cards of running Jobs on one board that could go quiet. A To do card is lost
/// when nobody has it: a Job with a card in Doing is moving, and its To do cards are
/// only waiting their turn. Times are kept per list in `quiet`, so moving on starts
/// them over.
pub fn cards_to_watch(i: &CardsIn, quiet: &mut HashMap<String, i64>) -> Watched {
    let mut cards = Vec::new();
    let mut seen = Vec::new();
    let running = |job: &str| i.jobs.iter().find(|j| j.id == job).is_some_and(|j| j.status == JobStatus::Running);
    let doing = cards_of(i.lists, "doing");
    for c in doing {
        let key = format!("doing:{}", c.id);
        seen.push(key.clone());
        let (Some(job), Some(who)) = (given(c.job.as_deref()), given(c.assignee_id.as_deref())) else { continue };
        if !running(job) {
            quiet.remove(&key);
            continue;
        }
        if (i.is_working)(who) {
            quiet.remove(&key);
        } else {
            quiet.entry(key.clone()).or_insert(i.now);
        }
        let quiet_since = quiet.get(&key).copied();
        cards.push(WatchedCard { id: c.id.clone(), title: c.title.clone(), job: job.into(), assignee_id: who.into(), quiet_since, returns: returns(c), list: WatchList::Doing });
    }
    for c in cards_of(i.lists, "to do") {
        let key = format!("todo:{}", c.id);
        seen.push(key.clone());
        let who = c.assignee_id.as_deref().or(i.contact);
        let (Some(job), Some(who)) = (given(c.job.as_deref()), given(who)) else { continue };
        if !running(job) || doing.iter().any(|d| d.job.as_deref() == Some(job)) {
            quiet.remove(&key);
            continue;
        }
        let quiet_since = *quiet.entry(key).or_insert(i.now);
        cards.push(WatchedCard { id: c.id.clone(), title: c.title.clone(), job: job.into(), assignee_id: who.into(), quiet_since: Some(quiet_since), returns: returns(c), list: WatchList::Todo });
    }
    Watched { cards, seen }
}

pub struct WorkIn<'a> {
    pub cards: &'a [WatchedCard],
    /// Card id to the nudges sent in its current quiet stretch.
    pub nudges: &'a HashMap<String, u32>,
    pub limits: &'a Limits,
    pub boss_of: &'a dyn Fn(&str) -> Option<String>,
    pub now: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct WorkOut {
    pub alerts: Vec<Alert>,
    pub delivers: Vec<Deliver>,
    /// The nudge counts to keep: only cards still quiet are in it.
    pub nudges: HashMap<String, u32>,
}

fn warn(key: String, c: &WatchedCard, text: String) -> Alert {
    Alert { key, job: Some(c.job.clone()), person_id: Some(c.assignee_id.clone()), text, level: Level::Warn, standing: false }
}

/// A quiet card gets its person's boss a nudge first, then the owner an alert;
/// another nudge only after another stretch as long. A card sent back too often
/// goes to the owner.
pub fn watch_work(i: &WorkIn) -> WorkOut {
    let mut alerts = Vec::new();
    let mut delivers = Vec::new();
    let mut nudges = HashMap::new();
    let span = i64::from(i.limits.stall_minutes) * 60_000;
    let minutes = i.limits.stall_minutes;
    for c in i.cards {
        if c.returns >= i.limits.max_returns {
            alerts.push(warn(format!("returns-{}", c.id), c, format!("\"{}\" went back {} times and needs you.", c.title, c.returns)));
        }
        let Some(since) = c.quiet_since else { continue };
        let quiet = i.now.saturating_sub(since);
        let had = i.nudges.get(&c.id).copied().unwrap_or(0);
        nudges.insert(c.id.clone(), had);
        if quiet < span.saturating_mul(i64::from(had) + 1) {
            continue;
        }
        let waiting = c.list == WatchList::Todo;
        let boss = if had == 0 { (i.boss_of)(&c.assignee_id) } else { None };
        match boss {
            Some(boss) => {
                nudges.insert(c.id.clone(), 1);
                let text = if waiting {
                    format!("\"{}\" has waited in To do for {minutes} minutes. Hand it out or start it.", c.title)
                } else {
                    format!("\"{}\" has gone quiet for {minutes} minutes. Check on it.", c.title)
                };
                delivers.push(Deliver { person_id: boss, text });
            }
            None => {
                let text = if waiting { format!("\"{}\" has not been picked up.", c.title) } else { format!("\"{}\" is stuck.", c.title) };
                alerts.push(warn(format!("stall-{}", c.id), c, text));
            }
        }
    }
    WorkOut { alerts, delivers, nudges }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Budget {
    pub alert: Option<Alert>,
    pub pause: bool,
}

/// A running Job warns at 80% of its token cap and is paused at 100%.
pub fn watch_budget(job: &Job) -> Budget {
    if job.status != JobStatus::Running {
        return Budget { alert: None, pause: false };
    }
    let share = job.tokens_used as f64 / job.token_cap as f64;
    let alert = |level, text| Some(Alert { key: format!("cap-{}", job.id), job: Some(job.id.clone()), person_id: None, text, level, standing: false });
    if share >= 1.0 {
        return Budget { pause: true, alert: alert(Level::Stop, format!("{} reached its token cap and is paused.", job.id)) };
    }
    if share >= 0.8 {
        return Budget { pause: false, alert: alert(Level::Warn, format!("{} has used {}% of its tokens.", job.id, (share * 100.0).round())) };
    }
    Budget { alert: None, pause: false }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
