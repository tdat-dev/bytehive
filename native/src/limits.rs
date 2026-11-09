//! What the Limit Guard says and decides (limitguard.ts, handoff.ts,
//! convdigest.ts), pure so it can be tested. The Inbox side that watches the
//! agents is inbox/limits.rs.

use agent_core::planusage::PlanUsage;

/// A little after the reset, so the plan has surely turned over.
pub const GRACE_MS: u64 = 90_000;
/// Never tell the same agent to carry on twice within this.
pub const RESUME_GAP_MS: u64 = 20 * 60_000;

/// A plan past this share of a limit gets a warning.
pub const WARN_AT: f64 = 90.0;

/// A limit's name in a sentence (planmeter.ts limitName).
pub fn limit_name(kind: &str, model: Option<&str>) -> String {
    match (kind, model) {
        ("session", _) => "5-hour limit".to_string(),
        ("weekly_all", _) => "weekly limit".to_string(),
        (_, Some(m)) => format!("{m} weekly limit"),
        _ => "limit".to_string(),
    }
}

pub const CARRY_ON: &str = "Your usage limit has reset. Carry on with the task where you left off.";
pub const LIMIT_WHY: &str = "it hit its plan's usage limit";

const ASK_CHARS: usize = 700;
const LAST_WORDS_CHARS: usize = 1500;
const MAX_ASKS: usize = 6;

/// Whose plan an agent's CLI runs on, in the plan meter's words.
pub fn plan_of(cli: &str) -> &'static str {
    match cli {
        "claude" => "Claude",
        "codex" => "Codex",
        _ => "",
    }
}

/// planResetFor: when a plan that is out (a limit at 99% or more) comes back,
/// the soonest of its full windows; none when it has room.
pub fn plan_reset_for(usage: Option<&PlanUsage>, now: i64) -> Option<i64> {
    usage?.limits.iter().filter(|l| l.percent >= 99.0).filter_map(|l| l.resets_ms).filter(|t| *t > now).min()
}

/// "Claude Code" for "claude".
pub fn cli_label(cli: &str) -> String {
    crate::presets::preset(cli).map_or_else(|| cli.to_string(), |p| p.label.to_string())
}

/// What an open agent is told when it takes a task over: one line, the rest
/// it reads itself with the bytehive MCP.
pub fn take_over_line(name: &str, cli: &str, why: &str) -> String {
    format!(
        "Take over the unfinished task of {name} ({cli}), which stopped because {why}. Call the bytehive agent_context tool with who set to {name} to read its task, plan, changed files and last words, check git status, then finish what is left. Say in one line what you picked up first."
    )
}

/// Its row says who has its task now.
pub fn handed_title(to: &str, title: Option<&str>) -> String {
    let rest = title.unwrap_or("");
    let rest = match rest.strip_prefix("Handed to ") {
        Some(r) => r.split_once(": ").map_or("", |(_, t)| t),
        None => rest,
    };
    if rest.is_empty() { format!("Handed to {to}") } else { format!("Handed to {to}: {rest}") }
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n { format!("{}…", s.chars().take(n - 1).collect::<String>().trim_end()) } else { s.to_string() }
}

fn one_block(s: &str) -> String {
    let s = s.replace("\r\n", "\n");
    let mut out = String::new();
    let mut blank = 0;
    for line in s.split('\n') {
        blank = if line.trim().is_empty() { blank + 1 } else { 0 };
        if blank < 2 {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// The brief a new agent starts with (handoffBrief): what it was asked and
/// its last words when its conversation was read, else what ByteHive knows.
/// The native transcript reader keeps no plan or file list yet, so those
/// parts of today's brief are left out.
pub fn handoff_brief(name: &str, cli: &str, why: &str, title: Option<&str>, asks: &[String], last_words: &str) -> String {
    let mut parts = vec![format!("You're taking over a task from {name} ({cli}), another coding agent working in this same folder. It stopped because {why}. Its work so far is already in the folder.")];
    let asks: Vec<String> = asks.iter().map(|a| one_block(a)).filter(|a| !a.is_empty()).collect();
    if !asks.is_empty() {
        let shown: Vec<&String> = if asks.len() > MAX_ASKS { std::iter::once(&asks[0]).chain(&asks[asks.len() - (MAX_ASKS - 1)..]).collect() } else { asks.iter().collect() };
        let note = if asks.len() > shown.len() { format!(" (the first, then the latest {} of {})", shown.len() - 1, asks.len()) } else { String::new() };
        let lines: Vec<String> = shown.iter().enumerate().map(|(k, a)| format!("{}. {}", k + 1, clip(a, ASK_CHARS).replace('\n', "\n   "))).collect();
        parts.push(format!("What it was asked, in order{note}:\n{}", lines.join("\n")));
    } else if let Some(t) = title.filter(|t| !t.is_empty()) {
        parts.push(format!("Its task: {t}"));
    }
    if !last_words.trim().is_empty() {
        let quoted: Vec<String> = clip(&one_block(last_words), LAST_WORDS_CHARS).lines().map(|l| format!("> {l}")).collect();
        parts.push(format!("Its last words:\n{}", quoted.join("\n")));
    }
    parts.push(if !asks.is_empty() {
        "Look at the current state of the files first (git status and the files above), then carry on and finish what's left. Don't redo what's already done, and say briefly what you picked up from before you start.".to_string()
    } else {
        "Its conversation couldn't be read, so look at the folder first (git status, recent changes) to see where it got to, then carry on and finish the task.".to_string()
    });
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::{handed_title, handoff_brief, limit_name, plan_reset_for, take_over_line};
    use agent_core::planusage::{PlanLimit, PlanUsage};

    #[test]
    fn names_limits_as_the_planmeter_does() {
        assert_eq!(limit_name("session", None), "5-hour limit");
        assert_eq!(limit_name("weekly_all", None), "weekly limit");
        assert_eq!(limit_name("weekly_scoped", Some("Opus")), "Opus weekly limit");
        assert_eq!(limit_name("window_300", None), "limit");
    }

    #[test]
    fn a_full_plan_comes_back_at_its_soonest_reset() {
        let lim = |percent: f64, resets_ms| PlanLimit { kind: "session".into(), percent, resets_ms, severity: String::new(), model: None };
        let u = PlanUsage { provider: "Claude".into(), plan: None, limits: vec![lim(100.0, Some(500)), lim(99.5, Some(300)), lim(40.0, Some(100))], as_of_ms: None };
        assert_eq!(plan_reset_for(Some(&u), 0), Some(300));
        assert_eq!(plan_reset_for(Some(&u), 400), Some(500));
        assert_eq!(plan_reset_for(None, 0), None);
    }

    #[test]
    fn says_who_has_the_task() {
        assert_eq!(handed_title("Bob", Some("Fix it")), "Handed to Bob: Fix it");
        assert_eq!(handed_title("Cid", Some("Handed to Bob: Fix it")), "Handed to Cid: Fix it");
        assert_eq!(handed_title("Bob", None), "Handed to Bob");
        assert!(take_over_line("Ana", "Claude Code", "it hit its plan's usage limit").starts_with("Take over the unfinished task of Ana (Claude Code), which stopped because it hit"));
    }

    #[test]
    fn briefs_from_the_conversation_or_the_title() {
        let b = handoff_brief("Ana", "Claude Code", "it ran out", Some("Fix it"), &["Fix the upload".into()], "Half done.");
        assert!(b.contains("What it was asked, in order:\n1. Fix the upload"));
        assert!(b.contains("Its last words:\n> Half done."));
        let none = handoff_brief("Ana", "Codex", "it ran out", Some("Fix it"), &[], "");
        assert!(none.contains("Its task: Fix it") && none.ends_with("then carry on and finish the task."));
    }
}
