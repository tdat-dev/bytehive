//! How much of each plan's limits is used: the 5-hour window, the week, and
//! any model's own week. For Claude the numbers come from Anthropic's usage
//! endpoint (what Claude Code's /usage shows, nothing spent), asked with the
//! sign-in Claude Code keeps; the asking itself is up to the UI, which owns
//! an HTTP client. For Codex they are read from its newest rollout (what its
//! last request was told), nothing asked of anyone. Port of src-tauri's
//! `planusage.rs`, without Tauri.
//!
//! An expired sign-in is never refreshed here: refreshing rotates the tokens
//! Claude Code itself holds, and two writers of that file would log it out.

use serde_json::Value;

use crate::codex::{sessions_root, sorted_dirs};
use crate::usage::tail_lines;

pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// One limit of a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanLimit {
    /// "session" (5 hours), "weekly_all", "weekly_scoped" (one model's week),
    /// or "window_<minutes>".
    pub kind: String,
    /// 0 to 100.
    pub percent: f64,
    /// When it resets, ms since 1970, when known.
    pub resets_ms: Option<i64>,
    /// "normal", "warning", ... as the provider grades it.
    pub severity: String,
    /// The model a scoped limit is for.
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanUsage {
    /// Whose plan: "Claude", "Codex".
    pub provider: String,
    /// "max", "pro", "plus", ... when known.
    pub plan: Option<String>,
    pub limits: Vec<PlanLimit>,
    /// When these numbers were true, for ones read from a CLI's last run.
    pub as_of_ms: Option<i64>,
}

fn iso_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp_millis())
}

/// The limits in Anthropic's answer: its `limits` list when it has one, else
/// the older `five_hour` / `seven_day` objects.
pub fn parse_limits(v: &Value) -> Vec<PlanLimit> {
    let s = |x: &Value| x.as_str().map(str::to_string);
    if let Some(list) = v.get("limits").and_then(Value::as_array) {
        return list
            .iter()
            .filter_map(|l| {
                Some(PlanLimit {
                    kind: s(l.get("kind")?)?,
                    percent: l.get("percent").and_then(Value::as_f64).unwrap_or(0.0),
                    resets_ms: l.get("resets_at").and_then(Value::as_str).and_then(iso_ms),
                    severity: l.get("severity").and_then(s).unwrap_or_else(|| "normal".into()),
                    model: l.pointer("/scope/model/display_name").and_then(s),
                })
            })
            .collect();
    }
    [("five_hour", "session"), ("seven_day", "weekly_all")]
        .iter()
        .filter_map(|(key, kind)| {
            let w = v.get(*key)?;
            Some(PlanLimit {
                kind: kind.to_string(),
                percent: w.get("utilization").and_then(Value::as_f64)?,
                resets_ms: w.get("resets_at").and_then(Value::as_str).and_then(iso_ms),
                severity: "normal".into(),
                model: None,
            })
        })
        .collect()
}

/// Why Claude's limits can't be asked for right now.
#[derive(Debug, PartialEq)]
pub enum NoSignIn {
    /// An API key, or no Claude sign-in at all.
    NotSignedIn,
    /// Claude Code hasn't refreshed its sign-in yet.
    Expired,
}

/// Claude Code's sign-in from `<home>/.claude/.credentials.json`: the token
/// and the plan it is for.
pub fn claude_credentials(home: &std::path::Path, now_ms: i64) -> Result<(String, Option<String>), NoSignIn> {
    let text = std::fs::read_to_string(home.join(".claude").join(".credentials.json")).map_err(|_| NoSignIn::NotSignedIn)?;
    let v: Value = serde_json::from_str(&text).map_err(|_| NoSignIn::NotSignedIn)?;
    let o = v.get("claudeAiOauth").ok_or(NoSignIn::NotSignedIn)?;
    let token = o.get("accessToken").and_then(Value::as_str).ok_or(NoSignIn::NotSignedIn)?;
    if o.get("expiresAt").and_then(Value::as_i64).is_some_and(|at| at <= now_ms) {
        return Err(NoSignIn::Expired);
    }
    Ok((token.to_string(), o.get("subscriptionType").and_then(Value::as_str).map(str::to_string)))
}

/// Codex's plan limits as its last request reported them, from the newest
/// rollouts of the last two weeks. None when Codex hasn't run.
pub fn codex_plan_usage() -> Option<PlanUsage> {
    let root = sessions_root()?;
    let mut days = Vec::new();
    'outer: for y in sorted_dirs(&root).into_iter().rev() {
        for m in sorted_dirs(&y).into_iter().rev() {
            for d in sorted_dirs(&m).into_iter().rev() {
                days.push(d);
                if days.len() >= 14 {
                    break 'outer;
                }
            }
        }
    }
    let mut files: Vec<(i64, std::path::PathBuf)> = Vec::new();
    for d in days {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }
            let ms = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
            files.push((ms, p));
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().take(20).find_map(|(ms, p)| codex_limits_in(&tail_lines(&p, 512 * 1024)).map(|u| PlanUsage { as_of_ms: Some(ms), ..u }))
}

/// The newest `rate_limits` in these rollout lines.
pub fn codex_limits_in(lines: &[String]) -> Option<PlanUsage> {
    for line in lines.iter().rev() {
        if !line.contains("\"rate_limits\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(rl) = v.pointer("/payload/rate_limits").filter(|r| r.is_object()) else { continue };
        let limits: Vec<PlanLimit> = ["primary", "secondary"]
            .iter()
            .filter_map(|k| {
                let w = rl.get(*k).filter(|w| w.is_object())?;
                Some(PlanLimit {
                    kind: match w.get("window_minutes").and_then(Value::as_u64).unwrap_or(0) {
                        300 => "session".into(),
                        10080 => "weekly_all".into(),
                        m => format!("window_{m}"),
                    },
                    percent: w.get("used_percent").and_then(Value::as_f64).unwrap_or(0.0),
                    resets_ms: w.get("resets_at").and_then(Value::as_i64).map(|s| s * 1000),
                    severity: "normal".into(),
                    model: None,
                })
            })
            .collect();
        if !limits.is_empty() {
            let plan = rl.get("plan_type").and_then(Value::as_str).map(str::to_string);
            return Some(PlanUsage { provider: "Codex".into(), plan, limits, as_of_ms: None });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_limits_list_with_a_models_own_week() {
        let v = json!({
            "five_hour": { "utilization": 2.0 },
            "limits": [
                { "kind": "session", "percent": 2, "severity": "normal", "resets_at": "2026-09-27T12:09:59+00:00", "scope": null },
                { "kind": "weekly_all", "percent": 64.5, "severity": "warning", "resets_at": "2026-10-02T17:59:59+00:00" },
                { "kind": "weekly_scoped", "percent": 0, "severity": "normal", "resets_at": null, "scope": { "model": { "display_name": "Fable" } } },
                { "percent": 5 }
            ]
        });
        let l = parse_limits(&v);
        assert_eq!(l.len(), 3);
        assert_eq!((l[0].kind.as_str(), l[0].percent), ("session", 2.0));
        assert_eq!(l[0].resets_ms, iso_ms("2026-09-27T12:09:59+00:00"));
        assert!(l[0].resets_ms.is_some());
        assert_eq!((l[1].percent, l[1].severity.as_str()), (64.5, "warning"));
        assert_eq!(l[2].model.as_deref(), Some("Fable"));
    }

    #[test]
    fn falls_back_to_the_five_hour_and_seven_day_windows() {
        let v = json!({ "five_hour": { "utilization": 12.0, "resets_at": "a" }, "seven_day": { "utilization": 40.0 }, "seven_day_opus": null });
        let l = parse_limits(&v);
        assert_eq!(l.iter().map(|x| (x.kind.as_str(), x.percent)).collect::<Vec<_>>(), vec![("session", 12.0), ("weekly_all", 40.0)]);
    }

    #[test]
    fn reads_codex_windows_from_its_rollout() {
        let lines = vec![
            r#"{"type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":12.0,"window_minutes":300,"resets_at":1787300000},"secondary":{"used_percent":80,"window_minutes":10080,"resets_at":1787317252},"plan_type":"plus"}}}"#.to_string(),
            r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#.to_string(),
        ];
        let u = codex_limits_in(&lines).unwrap();
        assert_eq!(u.plan.as_deref(), Some("plus"));
        assert_eq!(u.limits.iter().map(|l| (l.kind.as_str(), l.percent, l.resets_ms)).collect::<Vec<_>>(), vec![("session", 12.0, Some(1787300000000)), ("weekly_all", 80.0, Some(1787317252000))]);
        assert!(codex_limits_in(&["{}".to_string()]).is_none());
    }

    #[test]
    fn reads_the_sign_in_and_says_why_not() {
        let home = std::env::temp_dir().join(format!("bytehive-cred-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        assert_eq!(claude_credentials(&home, 0), Err(NoSignIn::NotSignedIn));
        std::fs::write(home.join(".claude/.credentials.json"), r#"{"claudeAiOauth":{"accessToken":"t","expiresAt":100,"subscriptionType":"max"}}"#).unwrap();
        assert_eq!(claude_credentials(&home, 50), Ok(("t".to_string(), Some("max".to_string()))));
        assert_eq!(claude_credentials(&home, 100), Err(NoSignIn::Expired));
        std::fs::remove_dir_all(&home).ok();
    }
}
