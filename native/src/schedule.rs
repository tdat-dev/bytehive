//! Scheduled agents (schedule.ts): start a saved preset at a time of day,
//! once or daily. Pure engine, the caller passes `now`; the Inbox owns the
//! tick and the launching. Kept in schedules.json in the data folder, in the
//! shape today's app keeps under localStorage "maestro.schedules".

use std::path::PathBuf;

use chrono::{DateTime, Days, Local, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Once,
    Daily,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: String,
    /// Which saved preset to start. Native presets have no id of their own,
    /// so this is the preset's name.
    pub template_id: String,
    /// "HH:MM" 24h, local.
    pub time: String,
    pub repeat: Repeat,
    pub enabled: bool,
    /// ms epoch of the last fire (per-slot dedup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fired: Option<i64>,
}

/// Parse "HH:MM", or None if malformed.
pub fn parse_time(t: &str) -> Option<(u32, u32)> {
    let (h, m) = t.trim().split_once(':')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !(1..=2).contains(&h.len()) || m.len() != 2 || !digits(h) || !digits(m) {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h <= 23 && m <= 59).then_some((h, m))
}

/// Today's occurrence of a schedule's time, or None if the time is malformed.
pub fn slot_today(s: &Schedule, now: DateTime<Local>) -> Option<DateTime<Local>> {
    let (h, m) = parse_time(&s.time)?;
    local_at(now.date_naive().and_time(NaiveTime::from_hms_opt(h, m, 0)?))
}

fn local_at(at: chrono::NaiveDateTime) -> Option<DateTime<Local>> {
    // A time the clock skips (spring forward) lands like JS Date does, after the gap.
    Local.from_local_datetime(&at).earliest().or_else(|| Local.from_local_datetime(&(at + chrono::Duration::hours(1))).earliest())
}

/// Due once per HH:MM slot: now has reached today's slot and it has not
/// fired for that slot yet. Disabled or malformed schedules are never due.
pub fn is_due(s: &Schedule, now: DateTime<Local>) -> bool {
    if !s.enabled {
        return false;
    }
    let Some(slot) = slot_today(s, now) else { return false };
    if now < slot {
        return false;
    }
    s.last_fired.unwrap_or(0) < slot.timestamp_millis()
}

/// Every schedule that should fire now, in list order.
pub fn due_schedules(list: &[Schedule], now: DateTime<Local>) -> Vec<Schedule> {
    list.iter().filter(|s| is_due(s, now)).cloned().collect()
}

/// The schedule after it fires at `now`: lastFired stamped; a once schedule
/// also turns itself off so it cannot run again.
pub fn after_fire(s: &Schedule, now: DateTime<Local>) -> Schedule {
    Schedule { last_fired: Some(now.timestamp_millis()), enabled: if s.repeat == Repeat::Once { false } else { s.enabled }, ..s.clone() }
}

/// Next run time for display, or None (off, malformed, spent once).
pub fn next_run(s: &Schedule, now: DateTime<Local>) -> Option<DateTime<Local>> {
    if !s.enabled {
        return None;
    }
    let slot = slot_today(s, now)?;
    let fired_this_slot = s.last_fired.unwrap_or(0) >= slot.timestamp_millis();
    let passed = now >= slot;
    if !passed && !fired_this_slot {
        return Some(slot); // later today
    }
    if s.repeat == Repeat::Once {
        return if fired_this_slot || passed { None } else { Some(slot) };
    }
    // Daily rolls to tomorrow, same wall-clock time.
    local_at(slot.naive_local().checked_add_days(Days::new(1))?)
}

/// The words under "next" on a row: paused, done, or when it runs next
/// (toLocaleString weekday short, hour and minute 2-digit).
pub fn next_label(s: &Schedule, now: DateTime<Local>) -> String {
    if !s.enabled {
        return "paused".to_string();
    }
    match next_run(s, now) {
        Some(at) => at.format("%a %I:%M %p").to_string(),
        None => "done".to_string(),
    }
}

/// A fresh id, "sc-" and seven characters as today's app makes them.
pub fn new_id() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("sc-{}", &hex[..7])
}

fn store() -> Option<PathBuf> {
    Some(crate::fleet::data_dir()?.join("schedules.json"))
}

/// Every schedule, as saved; a missing or broken file reads as none.
pub fn load() -> Vec<Schedule> {
    store().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Best effort, like today's localStorage write.
pub fn save(list: &[Schedule]) {
    let Some(path) = store() else { return };
    if let (Some(dir), Ok(json)) = (path.parent(), serde_json::to_vec_pretty(list)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Local, TimeZone, Timelike};

    use super::{Repeat, Schedule, after_fire, due_schedules, is_due, next_run, parse_time};

    /// 2026-07-10 local, as schedule.test.ts takes it.
    fn at(h: u32, m: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2026, 7, 10, h, m, 0).unwrap()
    }

    fn sched() -> Schedule {
        Schedule { id: "s1".into(), template_id: "t1".into(), time: "09:30".into(), repeat: Repeat::Daily, enabled: true, last_fired: None }
    }

    #[test]
    fn parse_time_accepts_valid_and_rejects_junk() {
        assert_eq!(parse_time("09:30"), Some((9, 30)));
        assert_eq!(parse_time("23:59"), Some((23, 59)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("9:5"), None);
        assert_eq!(parse_time("nope"), None);
    }

    #[test]
    fn not_due_before_the_slot() {
        assert!(!is_due(&sched(), at(9, 29)));
    }

    #[test]
    fn due_at_or_after_the_slot_when_not_yet_fired() {
        assert!(is_due(&sched(), at(9, 30)));
        assert!(is_due(&sched(), at(10, 0)));
    }

    #[test]
    fn not_due_again_once_fired_for_this_slot() {
        let fired = Schedule { last_fired: Some(at(9, 30).timestamp_millis()), ..sched() };
        assert!(!is_due(&fired, at(9, 45)));
    }

    #[test]
    fn disabled_schedules_are_never_due() {
        assert!(!is_due(&Schedule { enabled: false, ..sched() }, at(10, 0)));
    }

    #[test]
    fn after_fire_stamps_and_keeps_daily_enabled() {
        let s = after_fire(&sched(), at(9, 30));
        assert_eq!(s.last_fired, Some(at(9, 30).timestamp_millis()));
        assert!(s.enabled);
    }

    #[test]
    fn after_fire_disables_a_once_schedule() {
        let s = after_fire(&Schedule { repeat: Repeat::Once, ..sched() }, at(9, 30));
        assert!(!s.enabled);
    }

    #[test]
    fn due_schedules_returns_only_the_due_enabled_ones() {
        let list = vec![
            Schedule { id: "a".into(), time: "09:00".into(), ..sched() },
            Schedule { id: "b".into(), time: "10:00".into(), ..sched() },
            Schedule { id: "c".into(), time: "08:00".into(), enabled: false, ..sched() },
        ];
        let ids: Vec<String> = due_schedules(&list, at(9, 30)).into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["a".to_string()]);
    }

    #[test]
    fn next_run_shows_later_today_before_the_slot() {
        assert_eq!(next_run(&sched(), at(8, 0)).map(|d| d.hour()), Some(9));
    }

    #[test]
    fn next_run_daily_rolls_to_tomorrow_once_fired() {
        let fired = Schedule { last_fired: Some(at(9, 30).timestamp_millis()), ..sched() };
        assert_eq!(next_run(&fired, at(10, 0)).map(|d| d.day()), Some(11));
    }

    #[test]
    fn next_run_once_is_none_after_it_has_passed() {
        let fired = Schedule { repeat: Repeat::Once, last_fired: Some(at(9, 30).timestamp_millis()), ..sched() };
        assert_eq!(next_run(&fired, at(10, 0)), None);
    }

    #[test]
    fn keeps_the_shape_of_todays_storage() {
        let json = r#"[{"id":"sc-abc1234","templateId":"tpl-x","time":"09:00","repeat":"once","enabled":false,"lastFired":1752130200000}]"#;
        let list: Vec<Schedule> = serde_json::from_str(json).unwrap();
        assert_eq!(list[0].repeat, Repeat::Once);
        assert_eq!(list[0].last_fired, Some(1_752_130_200_000));
        assert_eq!(serde_json::to_string(&list).unwrap(), json);
    }
}
