//! What each agent has used and whether it hit its plan's limit, kept on
//! the pane for the grid header, the Queue and the Board. Usage is read off
//! the UI thread at most every 30 s, and only after the agent printed
//! something new (Claude priced at API list prices; Codex and opencode from
//! their own files; any CLI's status line fills the gaps). Limits are read
//! off the screen every 2 s. With auto-resume on, an agent stopped by a
//! limit is told to carry on once the limit has lifted, one time per limit.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use agent_core::usage::{claude_session_usage, codex_usage, opencode_usage};
use chrono::Local;
use regex::Regex;
use supervise::limits::limit_on_screen;
use supervise::pricing::cost;

use crate::app::{now_ms, ShellApp};
use crate::deliver::Delivery;
use crate::sessions_ui::Cli;
use crate::states::pane_cwd;

const READ_EVERY_MS: u64 = 30_000;
const SCAN_EVERY: Duration = Duration::from_secs(2);
/// What an agent is told once its limit lifts.
pub const RESUME_TEXT: &str = "Your usage limit has reset. Carry on where you left off.";

/// What an agent used so far.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageFig {
    pub usd: Option<f64>,
    pub tokens: Option<u64>,
    pub context_pct: Option<f64>,
}

static AIDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)Cost:\s*\$([\d,]*\.?\d+)\s*message,\s*\$([\d,]*\.?\d+)\s*session").unwrap());
static SPENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:session cost|total cost|spent|cost)\b[^$\n]{0,16}\$([\d,]*\.?\d+)").unwrap());
static LEFT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d{1,3})%\s*context left").unwrap());
static AUTO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)context left until auto-compact:\s*(\d{1,3})%").unwrap());

fn num(s: &str) -> Option<f64> {
    s.replace(',', "").parse().ok()
}

/// What a CLI's own status line says it spent and how full its context is.
pub fn usage_on_screen(screen: &str) -> UsageFig {
    let mut out = UsageFig::default();
    if let Some(c) = AIDER.captures(screen) {
        out.usd = num(&c[2]);
    } else if let Some(c) = SPENT.captures_iter(screen).last() {
        out.usd = num(&c[1]);
    }
    if let Some(c) = LEFT.captures_iter(screen).last() {
        out.context_pct = num(&c[1]).map(|n| (100.0 - n).max(0.0));
    } else if let Some(c) = AUTO.captures_iter(screen).last() {
        out.context_pct = num(&c[1]).map(|n| (100.0 - n).max(0.0));
    }
    out
}

/// Read one agent's usage from its CLI's own files. Runs off the UI thread.
fn read_files(cli: Cli, dir: &str, session: Option<&str>, since_ms: u64) -> UsageFig {
    match (cli, session) {
        (Cli::Claude, Some(sid)) => {
            let rows = claude_session_usage(dir, sid);
            let usd = rows.iter().filter_map(|r| cost(&r.model, r.input_tokens, r.output_tokens, r.cache_creation, r.cache_read)).reduce(|a, b| a + b);
            let tokens: u64 = rows.iter().map(|r| r.tokens()).sum();
            UsageFig { usd, tokens: (tokens > 0).then_some(tokens), context_pct: None }
        }
        (Cli::Codex, _) => codex_usage(dir, since_ms).map(|u| UsageFig { usd: u.usd, tokens: u.tokens, context_pct: u.context_pct }).unwrap_or_default(),
        (Cli::Opencode, _) => opencode_usage(dir, since_ms).map(|u| UsageFig { usd: u.usd, tokens: u.tokens, context_pct: u.context_pct }).unwrap_or_default(),
        _ => UsageFig::default(),
    }
}

pub struct UsageWatch {
    /// When each pane was last read, and whether a read is in flight.
    read_at: HashMap<String, (u64, bool)>,
    /// The limit reset each pane was already told to resume after.
    resumed: HashMap<String, i64>,
    last_scan: Instant,
    tx: Sender<(String, UsageFig)>,
    rx: Receiver<(String, UsageFig)>,
}

impl Default for UsageWatch {
    fn default() -> Self {
        let (tx, rx) = channel();
        UsageWatch { read_at: HashMap::new(), resumed: HashMap::new(), last_scan: Instant::now() - SCAN_EVERY, tx, rx }
    }
}

impl ShellApp {
    pub(crate) fn poll_usage(&mut self) {
        while let Ok((id, mut fig)) = self.usage.rx.try_recv() {
            if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                // What the screen says fills what the files did not.
                let s = usage_on_screen(&p.screen_text());
                fig.usd = fig.usd.or(s.usd);
                fig.context_pct = fig.context_pct.or(s.context_pct);
                p.usage = Some(fig);
            }
            if let Some(e) = self.usage.read_at.get_mut(&id) {
                e.1 = false;
            }
        }
        if self.usage.last_scan.elapsed() < SCAN_EVERY {
            return;
        }
        self.usage.last_scan = Instant::now();
        let now = now_ms();
        let clock = Local::now();
        let mut resume = Vec::new();
        for p in &mut self.panes {
            let Some(cli) = p.cli else { continue };
            p.limit = limit_on_screen(&p.screen_text(), clock);
            if let Some(at) = p.limit.as_ref().and_then(|l| l.resets_at) {
                let lifted = clock >= at + chrono::Duration::seconds(60);
                let waits = self.prefs.handover_to.is_none_or(|to| to == cli);
                if lifted && waits && p.exit_code.is_none() && self.usage.resumed.get(&p.id) != Some(&at.timestamp()) {
                    self.usage.resumed.insert(p.id.clone(), at.timestamp());
                    resume.push(p.id.clone());
                }
            }
            let e = self.usage.read_at.entry(p.id.clone()).or_insert((0, false));
            let moved = p.last_output_ms > e.0;
            if e.1 || !(e.0 == 0 || (moved && now - e.0 > READ_EVERY_MS)) {
                continue;
            }
            *e = (now, true);
            let (id, dir, sid, since, tx) = (p.id.clone(), pane_cwd(p), p.session_id.clone(), p.started_ms.saturating_sub(5000), self.usage.tx.clone());
            std::thread::spawn(move || {
                let _ = tx.send((id, read_files(cli, &dir, sid.as_deref(), since)));
            });
        }
        if self.prefs.auto_resume {
            for id in resume {
                self.deliveries.push(Delivery::new(id, RESUME_TEXT.to_string(), now, 0));
            }
        }
    }

    /// Everything the agents on show used, for the top bar.
    pub(crate) fn total_usd(&self, hidden: &std::collections::HashSet<String>) -> Option<f64> {
        self.panes.iter().filter(|p| !hidden.contains(&p.id)).filter_map(|p| p.usage.as_ref().and_then(|u| u.usd)).reduce(|a, b| a + b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limit_on_screen_shows_in_the_queue() {
        let mut app = ShellApp::new().unwrap();
        app.panes[0].cli = Some(Cli::Claude);
        app.panes[0].feed_bytes(b"\r\nYou've hit your limit \xC2\xB7 resets 11:59pm\r\n");
        app.poll_usage();
        assert!(app.panes[0].limit.is_some(), "limit read off the screen");
        let rows = crate::queue_ui::queue_rows(&app.panes, now_ms(), &Default::default(), &Default::default(), &Default::default());
        let line = &rows.iter().find(|r| r.pane_id == app.panes[0].id).unwrap().line;
        assert!(line.contains("hit its usage limit, back at 23:59"), "{line}");
    }

    #[test]
    fn reads_what_status_lines_say() {
        let aider = usage_on_screen("Tokens: 2.1k sent, 250 received. Cost: $0.01 message, $0.08 session.");
        assert_eq!(aider.usd, Some(0.08));
        let codex = usage_on_screen("  gpt-5 high \u{00B7} 42% context left");
        assert_eq!(codex.context_pct, Some(58.0));
        let claude = usage_on_screen("Context left until auto-compact: 12%");
        assert_eq!(claude.context_pct, Some(88.0));
        assert_eq!(usage_on_screen("nothing here"), UsageFig::default());
    }
}
