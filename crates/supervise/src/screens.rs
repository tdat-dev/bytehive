//! Two things read from the bottom of an agent's screen: what blocks it
//! that only you can clear (sign-in, CAPTCHA, API key), and whether it hit
//! its plan's usage limit with when that lifts. Only the last lines count, so
//! an agent talking about these isn't taken for one stopped by them. Pure.

use std::sync::OnceLock;

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Lines at the bottom of the screen looked at.
const TAIL_LINES: usize = 14;

fn screen_tail(screen: &str) -> Vec<String> {
    static CLEAN: OnceLock<Regex> = OnceLock::new();
    // │┃|╭╮╰╯─━■⎿●∙•·
    let clean = CLEAN.get_or_init(|| Regex::new("[\u{2502}\u{2503}|\u{256D}\u{256E}\u{2570}\u{256F}\u{2500}\u{2501}\u{25A0}\u{23BF}\u{25CF}\u{2219}\u{2022}\u{00B7}]+").unwrap());
    static SPACES: OnceLock<Regex> = OnceLock::new();
    let spaces = SPACES.get_or_init(|| Regex::new(r"\s+").unwrap());
    let lines: Vec<String> = screen
        .split('\n')
        .map(|l| {
            let l = l.strip_suffix('\r').unwrap_or(l);
            spaces.replace_all(&clean.replace_all(l, " "), " ").trim().to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();
    let skip = lines.len().saturating_sub(TAIL_LINES);
    lines[skip..].to_vec()
}

fn cap_chars(text: &str, n: usize) -> String {
    if text.chars().count() > n {
        text.chars().take(n).collect()
    } else {
        text.to_string()
    }
}

// ---- blocked: what only you can clear ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockedKind {
    Auth,
    Captcha,
    Key,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedHit {
    pub kind: BlockedKind,
    /// The line that said so, trimmed.
    pub what: String,
}

fn captcha_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| [r"(?i)captcha", r"(?i)verify (that )?you('re| are) (a )?human", r"(?i)human verification"].iter().map(|p| Regex::new(p).unwrap()).collect())
}

fn key_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [r"(?i)(invalid|missing|expired).*api key", r"(?i)api key.*(invalid|missing|expired|not set|not configured|not found|required)", r"(?i)set (your|the|an) \S*api[_ ]?key"]
            .iter()
            .map(|p| Regex::new(p).unwrap())
            .collect()
    })
}

fn auth_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"(?i)please (log|sign) in\b",
            r"(?i)\b(sign|log) in to continue\b",
            r"(?i)you (need to|must) (log|sign) in\b",
            r"(?i)\bnot (logged|signed) in\b",
            r"(?i)authentication (failed|expired|required)",
            r"(?i)session expired",
            r"(?i)\blog ?in required\b",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    })
}

/// What blocks the agent's screen that only you can clear, or none. The
/// bottom-most notice wins; a line matching several reads as the most
/// specific (a CAPTCHA or key problem before a generic sign-in).
pub fn blocked_on_screen(screen: &str) -> Option<BlockedHit> {
    let lines = screen_tail(screen);
    for line in lines.iter().rev() {
        let kind = if captcha_res().iter().any(|re| re.is_match(line)) {
            Some(BlockedKind::Captcha)
        } else if key_res().iter().any(|re| re.is_match(line)) {
            Some(BlockedKind::Key)
        } else if auth_res().iter().any(|re| re.is_match(line)) {
            Some(BlockedKind::Auth)
        } else {
            None
        };
        if let Some(kind) = kind {
            return Some(BlockedHit { kind, what: cap_chars(line, 160) });
        }
    }
    None
}

// ---- limits: usage limits and when they lift ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitHit {
    /// The line that said so, trimmed.
    pub what: String,
    /// When it lifts (ms), when the line says; else none.
    pub resets_at: Option<u64>,
}

fn hit_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"(?i)you'?ve hit your (?:[A-Za-z0-9_-]+ )?(?:usage |spend |rate )?limit",
            r"(?i)\b(?:usage|session|weekly|5-hour|five-hour|daily|monthly) limit (?:reached|exceeded)",
            r"(?i)\bclaude (?:ai )?usage limit reached",
            r"(?i)\b(?:quota (?:exceeded|exhausted)|exhausted your (?:daily |free )?quota)",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    })
}

fn duration_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(days?|d\b|hours?|hrs?|h\b|minutes?|mins?|m\b|seconds?|secs?|s\b)").unwrap())
}

/// "2 hours 13 minutes", "3h 12m", "45 min", "1 day" to ms; none when none.
pub fn duration_ms(text: &str) -> Option<f64> {
    let mut ms = 0.0;
    let mut any = false;
    for m in duration_re().captures_iter(text) {
        let n: f64 = m[1].parse().unwrap_or(0.0);
        let unit = m[2].to_lowercase();
        ms += n * if unit.starts_with('d') { 86_400_000.0 } else if unit.starts_with('h') { 3_600_000.0 } else if unit.starts_with('m') { 60_000.0 } else { 1000.0 };
        any = true;
    }
    if any { Some(ms) } else { None }
}

fn clock_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b").unwrap())
}

/// A clock time ("7pm", "7:30 pm", "19:05") on the day `base` is in.
fn at_clock(text: &str, base: DateTime<Local>) -> Option<DateTime<Local>> {
    let m = clock_re().captures(text)?;
    let mut h: u32 = m[1].parse().ok()?;
    let min: u32 = m.get(2).map(|g| g.as_str().parse().unwrap_or(0)).unwrap_or(0);
    let ap = m.get(3).map(|g| g.as_str().to_lowercase());
    if ap.is_none() && m.get(2).is_none() {
        return None; // a bare number isn't a time
    }
    if ap.as_deref() == Some("pm") && h < 12 {
        h += 12;
    }
    if ap.as_deref() == Some("am") && h == 12 {
        h = 0;
    }
    if h > 23 || min > 59 {
        return None;
    }
    base.with_hour(h)?.with_minute(min)?.with_second(0)?.with_nanosecond(0)
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

fn month_day_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let months = MONTHS.join("|");
        Regex::new(&format!(r"(?i)\b(?:({months})[a-z]*\.?\s+(\d{{1,2}})|(\d{{1,2}})\s+({months})[a-z]*)\b")).unwrap()
    })
}

fn weekday_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let days = DAYS.join("|");
        Regex::new(&format!(r"(?i)\b({days})[a-z]*\b")).unwrap()
    })
}

fn wait_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:in|after)\s+(.+)$").unwrap())
}

fn iso_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(?::\d{2})?(?:Z|[+-]\d{2}:?\d{2})?)").unwrap())
}

fn paren_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\([^)]*\)").unwrap())
}

fn spaces_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").unwrap())
}

fn short3(s: &str) -> &str {
    if s.len() >= 3 { &s[..3] } else { s }
}

/// When a limit lifts, from what the CLI says after "resets" or "try again":
/// a wait ("in 2 hours 13 minutes"), a time ("7pm": the next 7pm), a day and
/// time ("Thu 1am", "Oct 2, 1am"). None when it can't be told.
pub fn parse_reset(text: &str, now: u64) -> Option<u64> {
    let t = spaces_re().replace_all(&paren_re().replace_all(text, " "), " ").trim().to_string();
    // a wait
    if let Some(m) = wait_re().captures(&t) {
        if let Some(ms) = duration_ms(&m[1]) {
            if ms > 0.0 {
                return Some((now as f64 + ms) as u64);
            }
        }
    }
    if let Some(m) = iso_re().captures(&t) {
        if let Some(ms) = parse_iso_ms(&m[1]) {
            return Some(ms);
        }
    }
    let base = Local.timestamp_millis_opt(now as i64).single()?;
    // "Oct 2, 1am", "October 2 at 1am", "2 Oct 1am"
    if let Some(m) = month_day_re().captures(&t) {
        let mon_name = m.get(1).map(|g| g.as_str()).unwrap_or_else(|| m.get(4).map(|g| g.as_str()).unwrap_or(""));
        let mon = MONTHS.iter().position(|mo| *mo == short3(&mon_name.to_lowercase()))?;
        let day: u32 = m.get(2).map(|g| g.as_str()).unwrap_or_else(|| m.get(3).map(|g| g.as_str()).unwrap_or("0")).parse().ok()?;
        let d = Local.with_ymd_and_hms(base.year(), (mon + 1) as u32, day, 0, 0, 0).single()?;
        let rest = &t[m.get(0).unwrap().start() + m.get(0).unwrap().len()..];
        let mut at = at_clock(rest, d).unwrap_or(d);
        // December's reset seen in January
        if (at.timestamp_millis() as u64) < now.saturating_sub(86_400_000) {
            at = at.with_year(at.year() + 1)?;
        }
        return Some(at.timestamp_millis().max(0) as u64);
    }
    // "Thu 1am", "Thursday at 1am"
    if let Some(m) = weekday_re().captures(&t) {
        let want = DAYS.iter().position(|d| *d == short3(&m[1].to_lowercase()))? as i64;
        let shift = (want - base.weekday().num_days_from_sunday() as i64 + 7) % 7;
        let d = base + chrono::Duration::days(shift);
        let rest = &t[m.get(0).unwrap().start() + m.get(0).unwrap().len()..];
        if let Some(mut at) = at_clock(rest, d) {
            if (at.timestamp_millis().max(0) as u64) <= now {
                at = at + chrono::Duration::days(7);
            }
            return Some(at.timestamp_millis().max(0) as u64);
        }
    }
    // "7pm": the next one
    if let Some(mut at) = at_clock(&t, base) {
        if (at.timestamp_millis().max(0) as u64) <= now {
            at = at + chrono::Duration::days(1);
        }
        return Some(at.timestamp_millis().max(0) as u64);
    }
    None
}

fn parse_iso_ms(text: &str) -> Option<u64> {
    if let Ok(d) = DateTime::parse_from_rfc3339(text) {
        return Some(d.timestamp_millis().max(0) as u64);
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(text, fmt) {
            if let Some(local) = Local.from_local_datetime(&naive).single() {
                return Some(local.timestamp_millis().max(0) as u64);
            }
        }
    }
    None
}

fn reset_word_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:resets?|reset at|try again|available again)\b\s*(.*)$").unwrap())
}

/// The limit an agent's screen says it hit, or none.
pub fn limit_on_screen(screen: &str, now: u64) -> Option<LimitHit> {
    let lines = screen_tail(screen);
    for (i, line) in lines.iter().enumerate().rev() {
        if !hit_res().iter().any(|re| re.is_match(line)) {
            continue;
        }
        // the reset is on the same line or the next
        let next = lines.get(i + 1).map(String::as_str).unwrap_or("");
        let around = format!("{line} {next}");
        let resets_at = reset_word_re().captures(&around).and_then(|m| parse_reset(&m[1], now));
        return Some(LimitHit { what: cap_chars(line, 160), resets_at });
    }
    None
}
