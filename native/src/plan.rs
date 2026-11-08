//! The plans' limits at the foot of the agent list, as today's
//! src/planmeter.ts shows them: Claude asked from Anthropic (the numbers of
//! Claude Code's /usage, nothing spent), Codex read from its newest rollout.

use agent_core::planusage::{CLAUDE_USAGE_URL, NoSignIn, PlanLimit, PlanUsage, claude_credentials, codex_plan_usage, parse_limits};
use chrono::{Local, TimeZone};

/// Asked a few seconds after start, then every few minutes.
pub const FIRST_MS: u64 = 5_000;
pub const EVERY_MS: u64 = 2 * 60_000;

#[derive(Clone, Default)]
pub struct Source {
    pub usage: Option<PlanUsage>,
    /// "expired", "not signed in", or why asking failed.
    pub problem: Option<String>,
    pub asked_at: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    High,
    Full,
}

pub struct Row {
    pub label: String,
    pub percent: u32,
    pub resets: String,
    pub level: Level,
}

/// Ask Anthropic for Claude's limits. Blocking: run it off the UI thread.
pub fn ask_claude() -> Result<PlanUsage, String> {
    let home = std::env::var("USERPROFILE").map_err(|_| "no home folder".to_string())?;
    let now = chrono::Utc::now().timestamp_millis();
    let (token, plan) = claude_credentials(std::path::Path::new(&home), now).map_err(|e| match e {
        NoSignIn::NotSignedIn => "not signed in".to_string(),
        NoSignIn::Expired => "expired".to_string(),
    })?;
    let agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).http_status_as_error(false).build().new_agent();
    let mut res = agent
        .get(CLAUDE_USAGE_URL)
        .header("Authorization", &format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .call()
        .map_err(|e| format!("couldn't reach Anthropic: {e}"))?;
    match res.status().as_u16() {
        401 => return Err("expired".to_string()),
        s if !(200..300).contains(&s) => return Err(format!("Anthropic answered {s}")),
        _ => {}
    }
    let body: serde_json::Value = res.body_mut().read_json().map_err(|e| format!("unreadable answer: {e}"))?;
    Ok(PlanUsage { provider: "Claude".into(), plan, limits: parse_limits(&body), as_of_ms: None })
}

pub fn ask_codex() -> Option<PlanUsage> {
    codex_plan_usage()
}

/// "in 3h 12m", "in 40m"; the day and time when it is further off.
fn reset_words(at_ms: i64, now: i64) -> String {
    let mins = ((at_ms - now) as f64 / 60_000.0).round().max(0.0) as i64;
    if mins >= 24 * 60 {
        return Local.timestamp_millis_opt(at_ms).single().map(|d| d.format("%a %I:%M %p").to_string()).unwrap_or_default();
    }
    let h = mins / 60;
    if h > 0 { format!("in {h}h {}m", mins % 60) } else { format!("in {mins}m") }
}

/// Every window has reset since these numbers were true: 0% everywhere is noise.
pub fn all_reset(u: &PlanUsage, now: i64) -> bool {
    !u.limits.is_empty() && u.limits.iter().all(|l| l.resets_ms.is_some_and(|r| r <= now))
}

/// The rows to show: the 5 hours, the week, then any model's own week in use.
pub fn rows(u: &PlanUsage, now: i64) -> Vec<Row> {
    let order = |k: &str| match k {
        "session" => 0,
        "weekly_all" => 1,
        _ => 2,
    };
    let mut limits: Vec<&PlanLimit> = u.limits.iter().collect();
    limits.sort_by_key(|l| order(&l.kind));
    let mut out = Vec::new();
    for l in limits {
        let label = match l.kind.as_str() {
            "session" => "5 hours".to_string(),
            "weekly_all" => "This week".to_string(),
            _ if l.model.is_some() => format!("{}, this week", l.model.clone().unwrap_or_default()),
            k => match k.strip_prefix("window_").and_then(|m| m.parse::<i64>().ok()) {
                Some(m) if m % 1440 == 0 => format!("{} days", m / 1440),
                Some(m) => format!("{} hours", (m as f64 / 60.0).round()),
                None => String::new(),
            },
        };
        // a model's own week only once it is used; empty it is noise
        if label.is_empty() || (l.kind == "weekly_scoped" && l.percent <= 0.0) {
            continue;
        }
        let past = l.resets_ms.is_some_and(|r| r <= now);
        let percent = if past { 0 } else { l.percent.round() as u32 };
        let severity = if past { "" } else { l.severity.as_str() };
        let sev = severity.to_ascii_lowercase();
        let level = if percent >= 90 || ["critical", "exceeded", "rejected"].iter().any(|w| sev.contains(w)) {
            Level::Full
        } else if percent >= 70 || sev.contains("warn") {
            Level::High
        } else {
            Level::Ok
        };
        let resets = match l.resets_ms {
            Some(r) if !past => reset_words(r, now),
            _ => String::new(),
        };
        out.push(Row { label, percent, resets, level });
    }
    out
}

/// "Claude Max limits".
pub fn title(u: &PlanUsage) -> String {
    let plan = u.plan.as_deref().map(|p| {
        let mut c = p.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    });
    format!("{} {} limits", u.provider, plan.unwrap_or_else(|| "plan".to_string()))
}

/// "Nov 8, 12:52 AM".
pub fn clock(ms: i64) -> String {
    Local.timestamp_millis_opt(ms).single().map(|d| d.format("%b %-d, %I:%M %p").to_string()).unwrap_or_default()
}
