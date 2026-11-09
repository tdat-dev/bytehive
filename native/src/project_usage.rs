//! What Claude has used in one project, for the Usage modal: today's
//! `claude_usage` command (every transcript under ~/.claude/projects/<slug>,
//! summed by model, biggest first) and the numbers usage.ts prints. Prices are
//! supervise::pricing, the same table as today's pricing.ts.

use agent_core::usage::{ModelUsage, file_usage};

/// Sum Claude Code's usage for a project folder, by model. Blocking: run it
/// off the UI thread. Empty when Claude has not run there.
pub fn claude_usage(dir: &str) -> Vec<ModelUsage> {
    let Some(proj) = agent_core::claude::project_dir(dir) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&proj) else { return Vec::new() };
    let mut total: Vec<ModelUsage> = Vec::new();
    for path in entries.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl")) {
        for u in file_usage(&path) {
            match total.iter_mut().find(|t| t.model == u.model) {
                Some(t) => {
                    t.input_tokens += u.input_tokens;
                    t.output_tokens += u.output_tokens;
                    t.cache_creation += u.cache_creation;
                    t.cache_read += u.cache_read;
                    t.messages += u.messages;
                }
                None => total.push(u),
            }
        }
    }
    total.sort_by_key(|u| std::cmp::Reverse(u.input_tokens + u.output_tokens));
    total
}

/// Estimated USD for one model's usage, or None when the model is not priced.
pub fn usage_cost(u: &ModelUsage) -> Option<f64> {
    supervise::pricing::cost(&u.model, u.input_tokens, u.output_tokens, u.cache_creation, u.cache_read)
}

/// pricing.ts money: "$0.004", "$1.23", "$12".
pub fn money(usd: f64) -> String {
    if usd > 0.0 && usd < 0.01 {
        format!("${}", to_fixed(usd, 3))
    } else if usd < 10.0 {
        format!("${}", to_fixed(usd, 2))
    } else {
        format!("${}", (usd + 0.5).floor() as i64)
    }
}

/// usage.ts fmtTokens: "950", "4.2k", "1.30M".
pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", to_fixed(n as f64 / 1e6, 2))
    } else if n >= 1_000 {
        format!("{}k", to_fixed(n as f64 / 1e3, 1))
    } else {
        n.to_string()
    }
}

/// JS Number.toFixed for a value at or above zero: the exact decimal value
/// of the double, a tie going up (Rust's own formatting sends ties to even).
pub fn to_fixed(x: f64, digits: usize) -> String {
    // Enough places to hold any double here exactly.
    let exact = format!("{:.*}", digits + 80, x.max(0.0));
    let (int, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut kept: Vec<u8> = int.bytes().chain(frac.bytes().take(digits)).collect();
    if frac.as_bytes().get(digits).is_some_and(|d| *d >= b'5') {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let split = kept.len() - digits;
    let (a, b) = kept.split_at(split);
    let a = String::from_utf8_lossy(a);
    if digits == 0 { a.into_owned() } else { format!("{a}.{}", String::from_utf8_lossy(b)) }
}

/// A model id for the table: no vendor prefix, no date suffix.
pub fn short_model(model: &str) -> String {
    let m = model.strip_prefix("claude-").unwrap_or(model);
    // -\d{6,}$
    if let Some(at) = m.rfind('-') {
        let tail = &m[at + 1..];
        if tail.len() >= 6 && tail.bytes().all(|b| b.is_ascii_digit()) {
            return m[..at].to_string();
        }
    }
    m.to_string()
}

#[cfg(test)]
mod tests {
    use agent_core::usage::ModelUsage;

    use super::{fmt_tokens, money, short_model, usage_cost};

    #[test]
    fn prints_tokens_like_usage_ts() {
        assert_eq!(fmt_tokens(950), "950");
        assert_eq!(fmt_tokens(4200), "4.2k");
        assert_eq!(fmt_tokens(38_400), "38.4k");
        assert_eq!(fmt_tokens(1_300_000), "1.30M");
        // toFixed rounds the double's exact value, a tie up.
        assert_eq!(fmt_tokens(4250), "4.3k");
        assert_eq!(fmt_tokens(4350), "4.3k");
        assert_eq!(fmt_tokens(999_960), "1000.0k");
        assert_eq!(super::to_fixed(9.995, 2), "9.99");
        assert_eq!(super::to_fixed(0.125, 2), "0.13");
    }

    #[test]
    fn shortens_model_ids() {
        assert_eq!(short_model("claude-haiku-4-5-20251001"), "haiku-4-5");
        assert_eq!(short_model("claude-opus-5-5"), "opus-5-5");
        assert_eq!(short_model("gpt-5"), "gpt-5");
    }

    #[test]
    fn prices_at_the_list_price() {
        let u = ModelUsage { model: "claude-opus-5-5".into(), input_tokens: 1_000_000, output_tokens: 1_000_000, cache_creation: 1_000_000, cache_read: 1_000_000, messages: 3 };
        assert!((usage_cost(&u).unwrap() - 29.2).abs() < 1e-9);
        assert_eq!(usage_cost(&ModelUsage { model: "gpt-5".into(), ..Default::default() }), None);
        assert_eq!(money(0.004), "$0.004");
        assert_eq!(money(1.234), "$1.23");
        assert_eq!(money(12.4), "$12");
    }
}
