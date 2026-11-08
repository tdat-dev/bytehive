//! What each agent has used so far, as today's src/agentcost.ts reads it:
//! Claude's session priced at API list prices, Codex and opencode from where
//! each keeps it, and for any CLI what its own status line says. Read again
//! only after the agent printed something new, and not more often than
//! REFRESH_MS.

use std::sync::LazyLock;

use regex::Regex;

pub const REFRESH_MS: u64 = 30_000;

#[derive(Clone, Default, PartialEq)]
pub struct Usage {
    /// USD so far: at API prices for Claude, the CLI's own figure otherwise.
    pub usd: Option<f64>,
    pub tokens: Option<u64>,
    /// How full its context is, 0 to 100.
    pub context_pct: Option<f64>,
}

static ACCOUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)Cost:\s*\$([\d,]*\.?\d+)\s*message,\s*\$([\d,]*\.?\d+)\s*session").unwrap());
static SPENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:session cost|total cost|spent|cost)\b[^$\n]{0,16}\$([\d,]*\.?\d+)").unwrap());
static LEFT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d{1,3})%\s*context left").unwrap());
static AUTO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)context left until auto-compact:\s*(\d{1,3})%").unwrap());

fn number(s: &str) -> Option<f64> {
    s.replace(',', "").parse().ok()
}

/// What a CLI's status line says it has spent and how full its context is.
pub fn on_screen(screen: &str) -> Usage {
    let mut out = Usage::default();
    if let Some(c) = ACCOUNT.captures(screen) {
        out.usd = number(&c[2]);
    } else if let Some(c) = SPENT.captures_iter(screen).last() {
        out.usd = number(&c[1]);
    }
    if let Some(c) = LEFT.captures_iter(screen).last() {
        out.context_pct = number(&c[1]).map(|left| (100.0 - left).max(0.0));
    }
    if out.context_pct.is_none() {
        if let Some(c) = AUTO.captures_iter(screen).last() {
            out.context_pct = number(&c[1]).map(|left| (100.0 - left).max(0.0));
        }
    }
    out
}

/// Read what the agent's CLI has on file. Blocking: run it off the UI thread.
pub fn read(cli: &str, dir: &str, session_id: Option<&str>, since_ms: u64, screen: &str) -> Usage {
    let mut u = Usage::default();
    match cli {
        "claude" => {
            if let Some(id) = session_id {
                let rows = agent_core::usage::claude_session_usage(dir, id);
                let usd: f64 = rows.iter().filter_map(|r| supervise::pricing::cost(&r.model, r.input_tokens, r.output_tokens, r.cache_creation, r.cache_read)).sum();
                let tokens: u64 = rows.iter().map(|r| r.input_tokens + r.output_tokens + r.cache_creation + r.cache_read).sum();
                u.usd = (usd > 0.0).then_some(usd);
                u.tokens = (tokens > 0).then_some(tokens);
            }
        }
        "codex" | "opencode" => {
            let since = since_ms.saturating_sub(5000);
            let found = if cli == "codex" { agent_core::usage::codex_usage(dir, since) } else { agent_core::usage::opencode_usage(dir, since) };
            if let Some(r) = found {
                u.usd = r.usd;
                u.tokens = r.tokens;
                u.context_pct = r.context_pct;
            }
        }
        _ => {}
    }
    // What the screen says fills what the files didn't.
    let s = on_screen(screen);
    u.usd = u.usd.or(s.usd);
    u.context_pct = u.context_pct.or(s.context_pct);
    u
}

/// "950", "4.2k", "38k", "1.3M".
pub fn tokens(n: u64) -> String {
    let trim = |s: String| s.trim_end_matches(".0").to_string();
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        if n < 10_000 { format!("{}k", trim(format!("{:.1}", n as f64 / 1000.0))) } else { format!("{}k", (n as f64 / 1000.0).round()) }
    } else {
        format!("{}M", trim(format!("{:.1}", n as f64 / 1_000_000.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_cli_prints() {
        assert_eq!(on_screen("Tokens: 2.1k sent. Cost: $0.01 message, $0.08 session.").usd, Some(0.08));
        assert_eq!(on_screen("›\n  42% context left").context_pct, Some(58.0));
        assert_eq!(on_screen("Context left until auto-compact: 12%").context_pct, Some(88.0));
    }

    #[test]
    fn counts_tokens_like_today() {
        assert_eq!(tokens(950), "950");
        assert_eq!(tokens(4200), "4.2k");
        assert_eq!(tokens(38_400), "38k");
        assert_eq!(tokens(1_300_000), "1.3M");
    }
}
