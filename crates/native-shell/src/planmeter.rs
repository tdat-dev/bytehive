//! The plans' limits at the foot of the Queue: for Claude and for Codex, the
//! 5-hour window and the week (and a model's own week once used), how much of
//! each is used and when it resets. Claude is asked of Anthropic with Claude
//! Code's own sign-in (agent_core::planusage), Codex is read from its last
//! rollout. Asked a few seconds after Maestro opens, then every two minutes,
//! off the UI thread. Port of the web app's `planmeter.ts`.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use agent_core::planusage::{claude_credentials, codex_plan_usage, parse_limits, NoSignIn, PlanLimit, PlanUsage, CLAUDE_USAGE_URL};
use chrono::{Local, TimeZone};
use eframe::egui::{self, Color32, RichText};

use crate::sessions_ui::Cli;
use crate::theme::{ADD, AMBER, DEL, INK3, TEXT};

const FIRST: Duration = Duration::from_secs(5);
const EVERY: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Ok,
    High,
    Full,
}

#[derive(Debug, PartialEq)]
pub struct PlanRow {
    pub label: String,
    pub percent: u32,
    /// "in 3h 12m", or the day and time when further off.
    pub resets: String,
    pub level: Level,
}

/// "in 3h 12m", "in 40m"; the day and time when a day or more off.
pub fn reset_words(at: Option<i64>, now: i64) -> String {
    let Some(at) = at else { return String::new() };
    let mins = ((at - now).max(0) + 30_000) / 60_000;
    if mins >= 24 * 60 {
        return Local.timestamp_millis_opt(at).single().map(|t| t.format("%a %H:%M").to_string()).unwrap_or_default();
    }
    let h = mins / 60;
    if h > 0 { format!("in {h}h {}m", mins % 60) } else { format!("in {mins}m") }
}

/// Every window has reset since these numbers were true: 0% everywhere is noise.
pub fn all_reset(u: &PlanUsage, now: i64) -> bool {
    !u.limits.is_empty() && u.limits.iter().all(|l| l.resets_ms.is_some_and(|r| r <= now))
}

fn label_of(l: &PlanLimit) -> String {
    match l.kind.as_str() {
        "session" => "5 hours".into(),
        "weekly_all" => "This week".into(),
        _ if l.model.is_some() => format!("{}, this week", l.model.as_deref().unwrap_or("")),
        k => match k.strip_prefix("window_").and_then(|m| m.parse::<u64>().ok()) {
            Some(m) if m % 1440 == 0 => format!("{} days", m / 1440),
            Some(m) => format!("{} hours", (m + 30) / 60),
            None => String::new(),
        },
    }
}

/// The rows to show: the 5 hours, the week, then any model's own week in use.
pub fn plan_rows(u: &PlanUsage, now: i64) -> Vec<PlanRow> {
    let order = |k: &str| match k {
        "session" => 0,
        "weekly_all" => 1,
        _ => 2,
    };
    let mut limits: Vec<&PlanLimit> = u.limits.iter().collect();
    limits.sort_by_key(|l| order(&l.kind));
    limits
        .into_iter()
        .filter_map(|l| {
            let label = label_of(l);
            if label.is_empty() || (l.kind == "weekly_scoped" && l.percent <= 0.0) {
                return None;
            }
            let past = l.resets_ms.is_some_and(|r| r <= now);
            let percent = if past { 0 } else { l.percent.round().max(0.0) as u32 };
            let sev = if past { "" } else { l.severity.as_str() };
            let sev = sev.to_ascii_lowercase();
            let level = if percent >= 90 || ["critical", "exceeded", "rejected"].iter().any(|w| sev.contains(w)) {
                Level::Full
            } else if percent >= 70 || sev.contains("warn") {
                Level::High
            } else {
                Level::Ok
            };
            Some(PlanRow { label, percent, resets: if past { String::new() } else { reset_words(l.resets_ms, now) }, level })
        })
        .collect()
}

#[derive(Default)]
pub struct Source {
    pub usage: Option<PlanUsage>,
    /// Why the last ask failed: "not signed in", "expired", or the error.
    pub problem: Option<String>,
}

enum Answer {
    Claude(Result<PlanUsage, String>),
    Codex(Option<PlanUsage>),
}

pub struct PlanWatch {
    pub claude: Source,
    pub codex: Source,
    tx: Sender<Answer>,
    rx: Receiver<Answer>,
    started: Instant,
    last: Option<Instant>,
}

impl Default for PlanWatch {
    fn default() -> Self {
        let (tx, rx) = channel();
        PlanWatch { claude: Source::default(), codex: Source::default(), tx, rx, started: Instant::now(), last: None }
    }
}

fn now_ms() -> i64 {
    crate::app::now_ms() as i64
}

/// Ask Anthropic for the plan's limits right now (blocking; off the UI thread).
fn ask_claude() -> Result<PlanUsage, String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|_| "no home folder".to_string())?;
    let (token, plan) = claude_credentials(std::path::Path::new(&home), now_ms()).map_err(|e| match e {
        NoSignIn::NotSignedIn => "not signed in".to_string(),
        NoSignIn::Expired => "expired".to_string(),
    })?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).build().into();
    let mut res = agent
        .get(CLAUDE_USAGE_URL)
        .header("Authorization", &format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .call()
        .map_err(|e| format!("couldn't reach Anthropic: {e}"))?;
    match res.status().as_u16() {
        401 => return Err("expired".into()),
        s if !(200..300).contains(&s) => return Err(format!("Anthropic answered {s}")),
        _ => {}
    }
    let body = res.body_mut().read_to_string().map_err(|e| format!("unreadable answer: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("unreadable answer: {e}"))?;
    Ok(PlanUsage { provider: "Claude".into(), plan, limits: parse_limits(&v), as_of_ms: None })
}

impl PlanWatch {
    /// Ask when due and take in the answers.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(a) = self.rx.try_recv() {
            match a {
                Answer::Claude(Ok(u)) => {
                    self.claude.usage = Some(u);
                    self.claude.problem = None;
                }
                Answer::Claude(Err(why)) => {
                    if why == "not signed in" {
                        self.claude.usage = None;
                    }
                    self.claude.problem = Some(why);
                }
                Answer::Codex(u) => {
                    if u.is_some() {
                        self.codex.usage = u;
                    }
                }
            }
            ctx.request_repaint();
        }
        let due = match self.last {
            None => self.started.elapsed() >= FIRST,
            Some(t) => t.elapsed() >= EVERY,
        };
        if due && !cfg!(test) {
            self.last = Some(Instant::now());
            let (tx, repaint) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(Answer::Codex(codex_plan_usage()));
                let _ = tx.send(Answer::Claude(ask_claude()));
                repaint.request_repaint();
            });
        }
        ctx.request_repaint_after(EVERY);
    }

    /// The meter at the foot of the Queue; nothing when no plan has a say.
    pub fn show(&self, ui: &mut egui::Ui) {
        let now = now_ms();
        if self.claude.usage.is_none() && self.claude.problem.as_deref() == Some("expired") {
            ui.add_space(10.0);
            ui.label(RichText::new("Claude's sign-in renews the next time an agent runs; its limits show then.").size(11.5).color(INK3));
        }
        for src in [&self.claude, &self.codex] {
            let Some(u) = src.usage.as_ref().filter(|u| !all_reset(u, now)) else { continue };
            let rows = plan_rows(u, now);
            if rows.is_empty() {
                continue;
            }
            ui.add_space(10.0);
            let plan = u.plan.as_deref().map(|p| format!("{}{}", p[..1].to_uppercase(), &p[1..])).unwrap_or_else(|| "plan".into());
            let stale = u.as_of_ms.filter(|at| now - at > 10 * 60_000).or(if src.problem.is_some() { Some(now) } else { None });
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} {plan} limits", u.provider)).size(12.0).strong().color(TEXT));
                if let Some(at) = stale {
                    let when = Local.timestamp_millis_opt(at).single().map(|t| t.format("%b %-d, %H:%M").to_string()).unwrap_or_default();
                    ui.label(RichText::new(format!("as of {when}")).size(11.0).color(INK3)).on_hover_text(src.problem.clone().unwrap_or_else(|| format!("What {} was told on its last request", u.provider)));
                }
            });
            for r in rows {
                meter_row(ui, &r);
            }
        }
    }
}

/// The fullest window of a provider's plan that hasn't reset: (percent, its
/// name in words, when it resets).
fn fullest(u: &PlanUsage, now: i64) -> Option<(f64, String, Option<i64>)> {
    u.limits
        .iter()
        .filter(|l| !l.resets_ms.is_some_and(|r| r <= now))
        .max_by(|a, b| a.percent.total_cmp(&b.percent))
        .map(|l| {
            let name = match (l.kind.as_str(), &l.model) {
                ("session", _) => "5-hour limit".to_string(),
                ("weekly_all", _) => "weekly limit".to_string(),
                (_, Some(m)) => format!("{m} weekly limit"),
                _ => "limit".to_string(),
            };
            (l.percent, name, l.resets_ms)
        })
}

/// For the New task sheet: how much room `cli`'s plan has left when little
/// (80% or more used), and another CLI that has room. Empty otherwise.
pub fn room_note(plans: &[&PlanUsage], cli: Cli, now: i64) -> String {
    let full = |name: &str| plans.iter().find(|u| u.provider == name).filter(|u| !all_reset(u, now)).and_then(|u| fullest(u, now));
    let Some((pct, name, at)) = full(cli.name()) else { return String::new() };
    if pct < 80.0 {
        return String::new();
    }
    let at = at.and_then(|t| Local.timestamp_millis_opt(t).single()).map(|t| t.format("%H:%M").to_string());
    let head = if pct >= 99.0 {
        format!("{} is out of its {name}{}.", cli.name(), at.map(|a| format!(" until {a}")).unwrap_or_default())
    } else {
        format!("{} has used {}% of its {name}{}.", cli.name(), pct.round(), at.map(|a| format!(" \u{00B7} resets {a}")).unwrap_or_default())
    };
    match Cli::all().into_iter().find(|o| *o != cli && matches!(o, Cli::Claude | Cli::Codex) && full(o.name()).is_none_or(|(p, _, _)| p < 80.0)) {
        Some(o) => format!("{head} {} has room.", o.name()),
        None => head,
    }
}

impl PlanWatch {
    pub fn room_note(&self, cli: Cli, now: i64) -> String {
        let plans: Vec<&PlanUsage> = [&self.claude.usage, &self.codex.usage].into_iter().flatten().collect();
        room_note(&plans, cli, now)
    }
}

fn meter_row(ui: &mut egui::Ui, r: &PlanRow) {
    let color = match r.level {
        Level::Ok => ADD,
        Level::High => AMBER,
        Level::Full => DEL,
    };
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(86.0, 16.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(86.0);
            ui.label(RichText::new(&r.label).size(11.5).color(INK3));
        });
        let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 6.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, Color32::from_white_alpha(18));
        let mut fill = rect;
        fill.set_width(rect.width() * (r.percent.min(100) as f32 / 100.0));
        ui.painter().rect_filled(fill, 3.0, color);
        ui.label(RichText::new(format!("{}%", r.percent)).size(11.5).monospace().color(TEXT));
        if !r.resets.is_empty() {
            ui.label(RichText::new(format!("resets {}", r.resets)).size(11.0).color(INK3));
        }
    })
    .response
    .on_hover_text(format!("{}: {}% used", r.label, r.percent));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lim(kind: &str, percent: f64, resets_ms: Option<i64>, severity: &str, model: Option<&str>) -> PlanLimit {
        PlanLimit { kind: kind.into(), percent, resets_ms, severity: severity.into(), model: model.map(str::to_string) }
    }

    fn usage(limits: Vec<PlanLimit>) -> PlanUsage {
        PlanUsage { provider: "Claude".into(), plan: Some("max".into()), limits, as_of_ms: None }
    }

    #[test]
    fn rows_in_order_with_levels_and_resets() {
        let now = 1_000_000_000_000;
        let u = usage(vec![
            lim("weekly_all", 72.4, Some(now + 3 * 86_400_000), "normal", None),
            lim("session", 12.0, Some(now + (3 * 60 + 12) * 60_000), "normal", None),
            lim("weekly_scoped", 0.0, None, "normal", Some("Fable")),
            lim("weekly_scoped", 95.0, None, "normal", Some("Opus")),
            lim("window_2880", 10.0, None, "critical", None),
        ]);
        let rows = plan_rows(&u, now);
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["5 hours", "This week", "Opus, this week", "2 days"]);
        assert_eq!((rows[0].percent, rows[0].level, rows[0].resets.as_str()), (12, Level::Ok, "in 3h 12m"));
        assert_eq!(rows[1].level, Level::High);
        assert_eq!(rows[2].level, Level::Full);
        assert_eq!(rows[3].level, Level::Full, "a critical grade is full whatever the number");
    }

    #[test]
    fn a_window_that_reset_starts_from_nothing() {
        let now = 1_000_000_000_000;
        let u = usage(vec![lim("session", 99.0, Some(now - 1), "exceeded", None)]);
        let rows = plan_rows(&u, now);
        assert_eq!((rows[0].percent, rows[0].level, rows[0].resets.as_str()), (0, Level::Ok, ""));
        assert!(all_reset(&u, now));
        assert!(!all_reset(&usage(vec![lim("session", 9.0, None, "", None)]), now));
        assert_eq!(reset_words(Some(now + 40 * 60_000), now), "in 40m");
        assert_eq!(reset_words(None, now), "");
    }

    #[test]
    fn says_when_a_plan_is_nearly_out_and_who_has_room() {
        let now = 1_000_000_000_000;
        let claude = usage(vec![lim("session", 99.5, Some(now + 3_600_000), "", None), lim("weekly_all", 20.0, None, "", None)]);
        let codex = PlanUsage { provider: "Codex".into(), ..usage(vec![lim("session", 10.0, None, "", None)]) };
        let note = room_note(&[&claude, &codex], Cli::Claude, now);
        assert!(note.starts_with("Claude is out of its 5-hour limit until "), "{note}");
        assert!(note.ends_with("Codex has room."), "{note}");
        assert_eq!(room_note(&[&claude, &codex], Cli::Codex, now), "");
        assert_eq!(room_note(&[], Cli::Claude, now), "", "unknown plan: nothing to say");
    }
}
