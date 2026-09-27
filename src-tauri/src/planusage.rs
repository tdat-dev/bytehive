//! How much of the Claude plan's limits is used: the 5-hour session window,
//! the week, and any per-model weekly limit. Asked the way Claude Code's own
//! /usage asks, with the sign-in Claude Code keeps in ~/.claude/.credentials.json.
//! No model is called and nothing is spent. The token only goes to
//! api.anthropic.com, and only the percentages come back to the page.
//!
//! An expired sign-in is never refreshed here: refreshing rotates the tokens
//! Claude Code itself holds, and two writers of that file would log it out.
//! Claude Code refreshes it the next time an agent runs.

use crate::error::CommandError;
use serde::Serialize;
use serde_json::Value;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// One limit of the plan.
#[derive(Serialize, Debug, PartialEq)]
pub struct PlanLimit {
    /// "session" (5 hours), "weekly_all", "weekly_scoped" (one model's week).
    pub kind: String,
    /// 0–100.
    pub percent: f64,
    /// ISO time it resets, when the plan says.
    pub resets_at: Option<String>,
    /// "normal", "warning", … as Anthropic grades it.
    pub severity: String,
    /// The model a scoped limit is for ("Fable").
    pub model: Option<String>,
    /// When it resets, as ms since 1970, for a CLI that says it that way.
    pub resets_ms: Option<i64>,
}

#[derive(Serialize, Debug)]
pub struct PlanUsage {
    /// Whose plan: "Claude", "Codex".
    pub provider: String,
    /// "max", "pro", "plus", … when the sign-in says.
    pub plan: Option<String>,
    pub limits: Vec<PlanLimit>,
    /// When these numbers were true, for ones read from a CLI's last run
    /// rather than asked just now.
    pub as_of_ms: Option<i64>,
}

/// The limits out of the usage answer: its `limits` list when it has one,
/// else the older `five_hour` / `seven_day` objects.
pub fn parse_limits(v: &Value) -> Vec<PlanLimit> {
    let s = |x: &Value| x.as_str().map(str::to_string);
    if let Some(list) = v.get("limits").and_then(Value::as_array) {
        return list
            .iter()
            .filter_map(|l| {
                Some(PlanLimit {
                    kind: s(l.get("kind")?)?,
                    percent: l.get("percent").and_then(Value::as_f64).unwrap_or(0.0),
                    resets_at: l.get("resets_at").and_then(s),
                    severity: l.get("severity").and_then(s).unwrap_or_else(|| "normal".into()),
                    model: l.pointer("/scope/model/display_name").and_then(s),
                    resets_ms: None,
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
                resets_at: w.get("resets_at").and_then(s),
                severity: "normal".into(),
                model: None,
                resets_ms: None,
            })
        })
        .collect()
}

fn credentials() -> Result<(String, Option<String>), CommandError> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|_| CommandError::Failed("no home folder".into()))?;
    let text = std::fs::read_to_string(std::path::Path::new(&home).join(".claude").join(".credentials.json"))
        .map_err(|_| CommandError::Failed("not signed in".into()))?;
    let v: Value = serde_json::from_str(&text).map_err(|_| CommandError::Failed("not signed in".into()))?;
    let o = v.get("claudeAiOauth").ok_or_else(|| CommandError::Failed("not signed in".into()))?;
    let token = o.get("accessToken").and_then(Value::as_str).ok_or_else(|| CommandError::Failed("not signed in".into()))?;
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    if o.get("expiresAt").and_then(Value::as_i64).is_some_and(|at| at <= now_ms) {
        return Err(CommandError::Failed("expired".into()));
    }
    Ok((token.to_string(), o.get("subscriptionType").and_then(Value::as_str).map(str::to_string)))
}

/// The plan's limits right now. Errors say "not signed in" (an API key, or no
/// Claude sign-in), "expired" (Claude Code hasn't refreshed it yet), or why
/// the request failed.
#[tauri::command]
pub async fn claude_plan_usage() -> Result<PlanUsage, CommandError> {
    let (token, plan) = credentials()?;
    // the same TLS the updater uses; installing it twice is harmless
    let _ = rustls::crypto::ring::default_provider().install_default();
    // one client for the app's life: its connections are kept and reused
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let res = CLIENT
        .get_or_init(reqwest::Client::new)
        .get(USAGE_URL)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| CommandError::Failed(format!("couldn't reach Anthropic: {e}")))?;
    let status = res.status();
    if status.as_u16() == 401 {
        return Err(CommandError::Failed("expired".into()));
    }
    if !status.is_success() {
        return Err(CommandError::Failed(format!("Anthropic answered {status}")));
    }
    let body: Value = res.json().await.map_err(|e| CommandError::Failed(format!("unreadable answer: {e}")))?;
    Ok(PlanUsage { provider: "Claude".into(), plan, limits: parse_limits(&body), as_of_ms: None })
}

/// Codex's plan limits, as its last request reported them: every
/// `token_count` event in a rollout carries `rate_limits` (a 5-hour and a
/// weekly window, how much is used, when each resets). Read from the end of
/// the newest rollouts, nothing asked of OpenAI. Null when Codex hasn't run.
#[tauri::command]
pub async fn codex_plan_usage() -> Result<Option<PlanUsage>, CommandError> {
    crate::error::run_blocking(|| Ok(codex_plan_usage_now())).await
}

fn codex_plan_usage_now() -> Option<PlanUsage> {
    use crate::transcripts::codex::{sessions_root, sorted_dirs};
    let root = sessions_root()?;
    let mut days = Vec::new();
    for y in sorted_dirs(&root).into_iter().rev() {
        for m in sorted_dirs(&y).into_iter().rev() {
            for d in sorted_dirs(&m).into_iter().rev() {
                days.push(d);
                if days.len() >= 14 {
                    break;
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
    for (ms, p) in files.into_iter().take(20) {
        let lines = crate::agentusage::tail_lines(&p, 512 * 1024);
        if let Some(u) = codex_limits_in(&lines) {
            return Some(PlanUsage { as_of_ms: Some(ms), ..u });
        }
    }
    None
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
                let minutes = w.get("window_minutes").and_then(Value::as_u64).unwrap_or(0);
                Some(PlanLimit {
                    kind: match minutes {
                        300 => "session".into(),
                        10080 => "weekly_all".into(),
                        m => format!("window_{m}"),
                    },
                    percent: w.get("used_percent").and_then(Value::as_f64).unwrap_or(0.0),
                    resets_at: None,
                    severity: "normal".into(),
                    model: None,
                    resets_ms: w.get("resets_at").and_then(Value::as_i64).map(|s| s * 1000),
                })
            })
            .collect();
        if limits.is_empty() {
            continue;
        }
        let plan = rl.get("plan_type").and_then(Value::as_str).map(str::to_string);
        return Some(PlanUsage { provider: "Codex".into(), plan, limits, as_of_ms: None });
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
        assert_eq!((l[0].kind.as_str(), l[0].percent, l[0].resets_at.as_deref()), ("session", 2.0, Some("2026-09-27T12:09:59+00:00")));
        assert_eq!((l[1].percent, l[1].severity.as_str()), (64.5, "warning"));
        assert_eq!(l[2].model.as_deref(), Some("Fable"));
    }

    /// Asks Anthropic for real: `cargo test --lib planusage -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn asks_anthropic() {
        let u = tauri::async_runtime::block_on(claude_plan_usage()).map_err(|e| e.to_string());
        println!("{u:?}");
        assert!(u.is_ok());
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
    fn falls_back_to_the_five_hour_and_seven_day_windows() {
        let v = json!({ "five_hour": { "utilization": 12.0, "resets_at": "a" }, "seven_day": { "utilization": 40.0, "resets_at": "b" }, "seven_day_opus": null });
        let l = parse_limits(&v);
        assert_eq!(l.iter().map(|x| (x.kind.as_str(), x.percent)).collect::<Vec<_>>(), vec![("session", 12.0), ("weekly_all", 40.0)]);
    }
}
