//! An agent that hit its plan's usage limit, read from the bottom of its
//! screen, and when the limit lifts. Claude Code says "You've hit your limit
//! . resets 7pm (Asia/Saigon)", Codex "You've hit your usage limit ... try
//! again in 2 hours 13 minutes", Gemini "exhausted your daily quota". Only
//! the last lines count, so an agent talking about limits in an answer is
//! not taken for one that hit one. Ported from the web app's `limits.ts`.
//! Pure: the caller passes `now`.

use std::sync::LazyLock;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub struct LimitHit {
    /// The line that said so, trimmed.
    pub what: String,
    /// When it lifts, when the line says.
    pub resets_at: Option<DateTime<Local>>,
}

const TAIL_LINES: usize = 14;
const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

static HIT: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)you'?ve hit your (?:[\w-]+ )?(?:usage |spend |rate )?limit",
        r"(?i)\b(?:usage|session|weekly|5-hour|five-hour|daily|monthly) limit (?:reached|exceeded)",
        r"(?i)\bclaude (?:ai )?usage limit reached",
        r"(?i)\b(?:quota (?:exceeded|exhausted)|exhausted your (?:daily |free )?quota)",
    ]
    .iter()
    .map(|r| Regex::new(r).unwrap())
    .collect()
});
static DUR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(days?|d\b|hours?|hrs?|h\b|minutes?|mins?|m\b|seconds?|secs?|s\b)").unwrap());
static CLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b").unwrap());
static PARENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
static WAIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:in|after)\s+(.+)$").unwrap());
static RESET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:resets?|reset at|try again|available again)\b\s*(.*)$").unwrap());
static DECOR: LazyLock<Regex> = LazyLock::new(|| Regex::new("[\u{2502}\u{2503}|\u{256D}\u{256E}\u{2570}\u{256F}\u{2500}\u{2501}\u{25A0}\u{23BF}\u{25CF}\u{2219}\u{2022}\u{00B7}]+").unwrap());
static MONTH_DAY: LazyLock<Regex> = LazyLock::new(|| {
    let m = MONTHS.join("|");
    Regex::new(&format!(r"(?i)\b(?:({m})[a-z]*\.?\s+(\d{{1,2}})|(\d{{1,2}})\s+({m})[a-z]*)\b")).unwrap()
});
static WEEKDAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\b({})[a-z]*\b", DAYS.join("|"))).unwrap());

/// "2 hours 13 minutes", "3h 12m", "45 min", "1 day", or none.
pub fn duration_of(text: &str) -> Option<Duration> {
    let mut ms = 0f64;
    let mut any = false;
    for c in DUR.captures_iter(text) {
        let n: f64 = c[1].parse().unwrap_or(0.0);
        let u = c[2].to_lowercase();
        let unit = if u.starts_with('d') { 86_400_000.0 } else if u.starts_with('h') { 3_600_000.0 } else if u.starts_with('m') { 60_000.0 } else { 1000.0 };
        ms += n * unit;
        any = true;
    }
    any.then(|| Duration::milliseconds(ms as i64))
}

/// A clock time ("7pm", "7:30 pm", "19:05") on `day`.
fn at_clock(text: &str, day: NaiveDate) -> Option<DateTime<Local>> {
    let c = CLOCK.captures(text)?;
    let mut h: u32 = c[1].parse().ok()?;
    let min: u32 = c.get(2).map_or(Some(0), |m| m.as_str().parse().ok())?;
    let ap = c.get(3).map(|m| m.as_str().to_lowercase());
    if ap.is_none() && c.get(2).is_none() {
        return None; // a bare number is not a time
    }
    match ap.as_deref() {
        Some("pm") if h < 12 => h += 12,
        Some("am") if h == 12 => h = 0,
        _ => {}
    }
    Local.from_local_datetime(&day.and_hms_opt(h, min, 0)?).earliest()
}

/// When a limit lifts, from what follows "resets" or "try again": a wait,
/// a time (the next one), a weekday and time, or a month day and time.
pub fn parse_reset(text: &str, now: DateTime<Local>) -> Option<DateTime<Local>> {
    let t = PARENS.replace_all(text, " ").split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(w) = WAIT.captures(&t) {
        if let Some(d) = duration_of(&w[1]) {
            return Some(now + d);
        }
    }
    if let Some(md) = MONTH_DAY.captures(&t) {
        let name = md.get(1).or(md.get(4))?.as_str().to_lowercase();
        let mon = MONTHS.iter().position(|m| name.starts_with(m))? as u32 + 1;
        let day: u32 = md.get(2).or(md.get(3))?.as_str().parse().ok()?;
        let date = NaiveDate::from_ymd_opt(now.year(), mon, day)?;
        let rest = &t[md.get(0)?.end()..];
        let mut at = at_clock(rest, date).or_else(|| Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest())?;
        if at < now - Duration::days(1) {
            at = at.with_year(at.year() + 1)?; // December's reset seen in January
        }
        return Some(at);
    }
    if let Some(wd) = WEEKDAY.captures(&t) {
        let want = DAYS.iter().position(|d| wd[1].to_lowercase().starts_with(d))? as i64;
        let today = now.weekday().num_days_from_sunday() as i64;
        let date = now.date_naive() + Duration::days((want - today + 7) % 7);
        if let Some(mut at) = at_clock(&t[wd.get(0)?.end()..], date) {
            if at <= now {
                at += Duration::days(7);
            }
            return Some(at);
        }
    }
    let mut at = at_clock(&t, now.date_naive())?;
    if at <= now {
        at += Duration::days(1);
    }
    Some(at)
}

/// The limit an agent's screen says it hit, or none.
pub fn limit_on_screen(screen: &str, now: DateTime<Local>) -> Option<LimitHit> {
    let lines: Vec<String> = screen
        .lines()
        .map(|l| DECOR.replace_all(l, " ").split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    let lines = &lines[lines.len().saturating_sub(TAIL_LINES)..];
    for i in (0..lines.len()).rev() {
        if !HIT.iter().any(|re| re.is_match(&lines[i])) {
            continue;
        }
        let around = format!("{} {}", lines[i], lines.get(i + 1).map_or("", String::as_str));
        let resets_at = RESET.captures(&around).and_then(|r| parse_reset(&r[1], now));
        return Some(LimitHit { what: lines[i].chars().take(160).collect(), resets_at });
    }
    None
}

/// "19:00", or "Thu 01:00" when it is not today.
pub fn reset_label(at: DateTime<Local>, now: DateTime<Local>) -> String {
    if at.date_naive() == now.date_naive() {
        format!("{:02}:{:02}", at.hour(), at.minute())
    } else {
        at.format("%a %H:%M").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noon() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap() // a Wednesday
    }

    #[test]
    fn reads_waits() {
        assert_eq!(duration_of("2 hours 13 minutes"), Some(Duration::minutes(133)));
        assert_eq!(duration_of("3h 12m"), Some(Duration::minutes(192)));
        assert_eq!(duration_of("soon"), None);
        assert_eq!(parse_reset("in 2 hours", noon()), Some(noon() + Duration::hours(2)));
    }

    #[test]
    fn reads_clock_times_as_the_next_one() {
        assert_eq!(parse_reset("7pm (Asia/Saigon)", noon()).unwrap().hour(), 19);
        let morning = parse_reset("7am", noon()).unwrap();
        assert_eq!((morning.day(), morning.hour()), (15, 7), "7am already passed: tomorrow");
        assert_eq!(parse_reset("12", noon()), None, "a bare number is not a time");
    }

    #[test]
    fn reads_weekdays_and_dates() {
        let thu = parse_reset("Thu 1am", noon()).unwrap();
        assert_eq!((thu.day(), thu.hour()), (15, 1));
        let oct = parse_reset("Oct 20, 1am", noon()).unwrap();
        assert_eq!((oct.month(), oct.day(), oct.hour()), (10, 20, 1));
    }

    #[test]
    fn finds_a_limit_only_at_the_bottom() {
        let screen = "\u{25CF} working on it\n\u{2502} You've hit your limit \u{00B7} resets 7pm (Asia/Saigon) \u{2502}\n";
        let hit = limit_on_screen(screen, noon()).unwrap();
        assert!(hit.what.starts_with("You've hit your limit"));
        assert_eq!(hit.resets_at.unwrap().hour(), 19);
        let codex = "You've hit your usage limit. Upgrade or try again in 2 hours 13 minutes.";
        assert_eq!(limit_on_screen(codex, noon()).unwrap().resets_at, Some(noon() + Duration::minutes(133)));
        let old = format!("You've hit your limit\n{}", "line\n".repeat(20));
        assert!(limit_on_screen(&old, noon()).is_none(), "scrolled away: not a live limit");
        assert_eq!(reset_label(noon() + Duration::hours(7), noon()), "19:00");
    }
}
