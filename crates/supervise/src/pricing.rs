//! What Claude usage costs, estimated from transcript token counts at
//! Anthropic's API list prices (USD per million tokens). A subscription does
//! not bill per token, so this is what the same work would cost on the API,
//! not a bill. Ported from the web app's `pricing.ts`. Pure.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

const fn p(input: f64, output: f64, cache_write: f64, cache_read: f64) -> Price {
    Price { input, output, cache_write, cache_read }
}

/// The list price of a Claude model id, or none. Newer and more specific
/// ids come before the family, first match wins.
pub fn price_for(model: &str) -> Option<Price> {
    let m = model.to_lowercase();
    let has = |s: &str| m.contains(s);
    let opus_new = ["opus-5", "opus-4-5", "opus-4-6", "opus-4-7", "opus-4-8", "opus-4-9"].iter().any(|s| has(s));
    Some(if has("fable-5-1") || has("mythos-5-1") {
        p(10.0, 50.0, 12.5, 0.25)
    } else if has("fable") || has("mythos") {
        p(10.0, 50.0, 12.5, 1.0)
    } else if has("opus-5-5") {
        p(4.0, 20.0, 5.0, 0.2)
    } else if opus_new {
        p(5.0, 25.0, 6.25, 0.5)
    } else if has("opus") {
        p(15.0, 75.0, 18.75, 1.5)
    } else if has("sonnet-5") {
        p(2.0, 10.0, 2.5, 0.2)
    } else if has("sonnet") {
        p(3.0, 15.0, 3.75, 0.3)
    } else if has("haiku") {
        p(1.0, 5.0, 1.25, 0.1)
    } else {
        return None;
    })
}

/// Estimated USD for token counts of one model, or none when it is not priced.
pub fn cost(model: &str, input: u64, output: u64, cache_write: u64, cache_read: u64) -> Option<f64> {
    let p = price_for(model)?;
    Some((input as f64 * p.input + output as f64 * p.output + cache_write as f64 * p.cache_write + cache_read as f64 * p.cache_read) / 1e6)
}

/// "$0.004", "$1.23", "$12".
pub fn money(usd: f64) -> String {
    if usd > 0.0 && usd < 0.01 {
        format!("${usd:.3}")
    } else if usd < 10.0 {
        format!("${usd:.2}")
    } else {
        format!("${}", usd.round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_ids_win_over_the_family() {
        assert_eq!(price_for("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(price_for("claude-opus-4-6").unwrap().input, 5.0);
        assert_eq!(price_for("claude-opus-4-1").unwrap().input, 15.0);
        assert_eq!(price_for("claude-haiku-4-5-20251001").unwrap().output, 5.0);
        assert!(price_for("gpt-5").is_none());
    }

    #[test]
    fn costs_a_million_tokens_at_list_price() {
        assert_eq!(cost("claude-sonnet-4-5", 1_000_000, 0, 0, 0), Some(3.0));
        assert_eq!(cost("claude-haiku-4-5", 0, 1_000_000, 0, 1_000_000), Some(5.1));
    }

    #[test]
    fn money_reads_short() {
        assert_eq!(money(0.0042), "$0.004");
        assert_eq!(money(1.234), "$1.23");
        assert_eq!(money(12.6), "$13");
    }
}
