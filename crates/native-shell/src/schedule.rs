//! Scheduled tasks: a New task set to start at a time of day, once or
//! daily (the approved design folds Schedule into each task). The engine is
//! the web app's `schedule.ts`, pure: the caller passes `now`. A schedule is
//! due once per HH:MM slot, after the slot and not yet fired for it; a "once"
//! schedule turns itself off when it fires.

use chrono::{DateTime, Duration, Local, TimeZone};
use serde_json::{json, Value};

use crate::newtask::NewTask;
use crate::prefs::cli_from_key;
use crate::taskopts::{Flow, Perm};

#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    pub id: String,
    pub task: NewTask,
    /// "HH:MM", 24 h, local time.
    pub time: String,
    pub daily: bool,
    pub enabled: bool,
    /// When it last fired (ms), for once-per-slot.
    pub last_fired: u64,
}

/// "HH:MM" to (h, m), or none when malformed.
pub fn parse_time(t: &str) -> Option<(u32, u32)> {
    let (h, m) = t.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60 && t.trim().len() >= 4).then_some((h, m))
}

impl Schedule {
    /// Today's slot, or none when the time is malformed.
    fn slot_today(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let (h, m) = parse_time(&self.time)?;
        Local.from_local_datetime(&now.date_naive().and_hms_opt(h, m, 0)?).earliest()
    }

    pub fn is_due(&self, now: DateTime<Local>) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(slot) = self.slot_today(now) else { return false };
        now >= slot && (self.last_fired as i64) < slot.timestamp_millis()
    }

    /// Stamp the fire; a once schedule switches itself off.
    pub fn fired(&mut self, now: DateTime<Local>) {
        self.last_fired = now.timestamp_millis() as u64;
        if !self.daily {
            self.enabled = false;
        }
    }

    /// When it runs next, for display; none when off or spent.
    pub fn next_run(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        if !self.enabled {
            return None;
        }
        let slot = self.slot_today(now)?;
        let fired_today = self.last_fired as i64 >= slot.timestamp_millis();
        if now < slot && !fired_today {
            return Some(slot);
        }
        if !self.daily {
            return None;
        }
        Some(slot + Duration::days(1))
    }

    /// "Daily at 09:00" / "Once at 09:00".
    pub fn when(&self) -> String {
        format!("{} at {}", if self.daily { "Daily" } else { "Once" }, self.time.trim())
    }

    pub fn to_json(&self) -> Value {
        let t = &self.task;
        json!({
            "id": self.id, "time": self.time, "daily": self.daily, "enabled": self.enabled, "lastFired": self.last_fired,
            "task": { "prompt": t.prompt, "dir": t.dir, "cli": t.cli.program(), "count": t.count, "worktree": t.worktree, "perm": t.perm.key(), "flow": t.flow.key() }
        })
    }

    pub fn from_json(v: &Value) -> Option<Schedule> {
        let t = &v["task"];
        let mut task = NewTask::new(t["dir"].as_str()?.to_string());
        task.prompt = t["prompt"].as_str()?.to_string();
        task.cli = cli_from_key(t["cli"].as_str()?)?;
        task.set_count(t["count"].as_u64().unwrap_or(1) as usize);
        task.worktree = t["worktree"].as_bool().unwrap_or(true);
        task.perm = Perm::from_key(t["perm"].as_str().unwrap_or(""));
        task.flow = Flow::from_key(t["flow"].as_str().unwrap_or(""));
        let time = v["time"].as_str()?.to_string();
        parse_time(&time)?;
        Some(Schedule {
            id: v["id"].as_str().unwrap_or("s").to_string(),
            task,
            time,
            daily: v["daily"].as_bool().unwrap_or(false),
            enabled: v["enabled"].as_bool().unwrap_or(true),
            last_fired: v["lastFired"].as_u64().unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions_ui::Cli;

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 14, h, m, 0).unwrap()
    }

    fn sched(time: &str, daily: bool) -> Schedule {
        let mut task = NewTask::new(r"D:\maestro".into());
        task.prompt = "Run the nightly checks".into();
        task.cli = Cli::Codex;
        Schedule { id: "s1".into(), task, time: time.into(), daily, enabled: true, last_fired: 0 }
    }

    #[test]
    fn reads_times() {
        assert_eq!(parse_time("09:05"), Some((9, 5)));
        assert_eq!(parse_time("9:05"), Some((9, 5)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("9"), None);
    }

    #[test]
    fn due_once_per_slot() {
        let mut s = sched("09:00", true);
        assert!(!s.is_due(at(8, 59)));
        assert!(s.is_due(at(9, 0)));
        s.fired(at(9, 0));
        assert!(!s.is_due(at(9, 30)), "already fired this slot");
        assert!(s.enabled, "daily stays on");
        assert_eq!(s.next_run(at(9, 30)), Some(at(9, 0) + Duration::days(1)));
    }

    #[test]
    fn a_once_schedule_turns_itself_off() {
        let mut s = sched("09:00", false);
        assert_eq!(s.next_run(at(8, 0)), Some(at(9, 0)));
        s.fired(at(9, 0));
        assert!(!s.enabled);
        assert_eq!(s.next_run(at(10, 0)), None);
    }

    #[test]
    fn round_trips_through_json() {
        let s = sched("21:30", true);
        assert_eq!(Schedule::from_json(&s.to_json()), Some(s));
        assert_eq!(Schedule::from_json(&json!({ "time": "nope", "task": {} })), None);
    }
}
