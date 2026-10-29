//! Small text helpers every CLI parser shares.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::types::{ChatImage, DiffLine, DiffSign};

pub const MAX_OUTPUT: usize = 6000;
/// Pictures kept per step or message; a burst of screenshots keeps its last ones.
pub const MAX_IMAGES: usize = 8;
pub const MAX_DIFF_LINES: usize = 400;

/// A JSON string, or "" for anything else.
pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

pub fn s_at<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).map(s).unwrap_or("")
}

pub fn b_at(v: &Value, key: &str) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
}

pub fn num_at(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// An ISO timestamp as ms since the epoch, or 0 when it does not parse.
pub fn parse_ms(raw: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(raw).map(|d| d.timestamp_millis().max(0) as u64).unwrap_or(0)
}

/// The last part of a path.
pub fn base_name(p: &str) -> &str {
    let t = p.trim_end_matches(['/', '\\']);
    match t.rfind(['/', '\\']) {
        Some(i) => &t[i + 1..],
        None => {
            if t.is_empty() {
                p
            } else {
                t
            }
        }
    }
}

/// First line, trimmed, cut to `max` chars with an ellipsis.
pub fn first_line(s: &str, max: usize) -> String {
    let line = s.trim().split(['\r', '\n']).next().unwrap_or("");
    if line.chars().count() > max {
        let mut out: String = line.chars().take(max.saturating_sub(1)).collect();
        out.push('.');
        out
    } else {
        line.to_string()
    }
}

pub fn first_line_default(s: &str) -> String {
    first_line(s, 120)
}

/// Line diff by longest common subsequence; big inputs fall back to all-out, all-in.
pub fn line_diff(before: &str, after: &str) -> Vec<DiffLine> {
    // Split like JS: on \n with \r stripped, keeping a trailing "" when the
    // text ends in a newline.
    fn ts_lines(s: &str) -> Vec<String> {
        s.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect()
    }
    let a = ts_lines(before);
    let b = ts_lines(after);
    if a.len() * b.len() > 160_000 {
        let mut out: Vec<DiffLine> = a.into_iter().map(|text| DiffLine { sign: DiffSign::Removed, text }).collect();
        out.extend(b.into_iter().map(|text| DiffLine { sign: DiffSign::Added, text }));
        return out;
    }
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffLine { sign: DiffSign::Same, text: a[i].clone() });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push(DiffLine { sign: DiffSign::Removed, text: a[i].clone() });
            i += 1;
        } else {
            out.push(DiffLine { sign: DiffSign::Added, text: b[j].clone() });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffLine { sign: DiffSign::Removed, text: a[i].clone() });
        i += 1;
    }
    while j < m {
        out.push(DiffLine { sign: DiffSign::Added, text: b[j].clone() });
        j += 1;
    }
    out
}

pub fn count_diff(d: &[DiffLine]) -> (u32, u32) {
    let mut added = 0u32;
    let mut removed = 0u32;
    for l in d {
        match l.sign {
            DiffSign::Added => added += 1,
            DiffSign::Removed => removed += 1,
            DiffSign::Same => {}
        }
    }
    (added, removed)
}

/// Text of a tool_result's content, whatever shape it came in.
pub fn result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|c| {
                if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                    let t = s_at(c, "text");
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub fn trim_output(s: &str) -> String {
    let t = s.trim_end();
    if t.chars().count() > MAX_OUTPUT {
        let mut out: String = t.chars().take(MAX_OUTPUT).collect();
        out.push_str("\n.");
        out
    } else {
        t.to_string()
    }
}

fn image_data_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9+/=\s]+$").unwrap())
}

/// The pictures in a message's or a tool result's content.
pub fn images_of(content: &Value) -> Vec<ChatImage> {
    let Value::Array(parts) = content else { return Vec::new() };
    let mut out = Vec::new();
    for c in parts {
        if c.get("type").and_then(|t| t.as_str()) != Some("image") {
            continue;
        }
        let src = c.get("source").cloned().unwrap_or(Value::Null);
        let media = s_at(&src, "media_type").to_lowercase();
        let data = s_at(&src, "data");
        let head: String = data.chars().take(256).collect();
        let ok_media = matches!(media.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp");
        if s_at(&src, "type") == "base64" && ok_media && image_data_re().is_match(&head) {
            out.push(ChatImage { media, data: data.to_string() });
        }
    }
    if out.len() > MAX_IMAGES {
        out.drain(..out.len() - MAX_IMAGES);
    }
    out
}

fn reminder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<system-reminder>.*?</system-reminder>").unwrap())
}

/// What you typed, without the reminders and hook text the CLI adds around it.
pub fn clean_user_text(s: &str) -> String {
    reminder_re().replace_all(s, "").trim().to_string()
}

fn answer_pair_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""((?:[^"\\]|\\.)*)"="((?:[^"\\]|\\.)*)""#).unwrap())
}

/// The answers in an AskUserQuestion result: `"question"="answer"` pairs, or
/// the structured ones. Values stay raw, escapes and all, as the parser keeps.
pub fn ask_answers(text: &str, extra: &Value) -> Vec<(String, String)> {
    if let Some(answers) = extra.get("answers").and_then(|a| a.as_object()) {
        let out: Vec<(String, String)> = answers.iter().filter_map(|(q, a)| a.as_str().map(|s| (q.clone(), s.to_string()))).collect();
        if !out.is_empty() {
            return out;
        }
    }
    answer_pair_re()
        .captures_iter(text)
        .map(|m| (m[1].to_string(), m[2].to_string()))
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trim_output_strips_trailing_and_caps_chars() {
        let long = "x".repeat(7000);
        let out = trim_output(&long);
        assert!(out.ends_with("\n."));
        assert_eq!(trim_output("  ok  "), "  ok");
        assert_eq!(trim_output("ok"), "ok");
    }

    #[test]
    fn clean_user_text_strips_reminders() {
        let s = "do it <system-reminder>hidden</system-reminder> now";
        assert_eq!(clean_user_text(s), "do it  now");
    }

    #[test]
    fn parse_ms_reads_iso_only_like_date_parse() {
        assert_eq!(parse_ms("2024-01-02T03:04:05.000Z"), 1704164645000);
        assert_eq!(parse_ms("1700000000000"), 0);
        assert_eq!(parse_ms(""), 0);
        assert_eq!(parse_ms("bogus"), 0);
    }

    #[test]
    fn line_diff_marks_changed_lines() {
        let d = line_diff("a\nb\nc", "a\nB\nc");
        let signs: Vec<(&str, &str)> = d.iter().map(|l| {
            let sign = match l.sign {
                DiffSign::Same => " ",
                DiffSign::Added => "+",
                DiffSign::Removed => "-",
            };
            (sign, l.text.as_str())
        }).collect();
        assert_eq!(signs, vec![(" ", "a"), ("-", "b"), ("+", "B"), (" ", "c")]);
        assert_eq!(line_diff("a", "b").len(), 2);
        assert!(line_diff("", "").iter().all(|l| l.sign == DiffSign::Same));
    }

    #[test]
    fn ask_answers_prefers_structured() {
        let extra = json!({"answers": {"color?": "blue"}});
        assert_eq!(ask_answers("ignored", &extra), vec![("color?".to_string(), "blue".to_string())]);
        let none = json!({});
        assert_eq!(ask_answers(r#""a"="b""#, &none), vec![("a".to_string(), "b".to_string())]);
    }
}

/// A URL's host, or the URL itself when it has no scheme to parse.
pub fn url_host(url: &str) -> String {
    let after = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => return url.to_string(),
    };
    let host = after.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = match host.rsplit_once(':') {
        // A port is digits; anything else keeps the colon (IPv6, garbage).
        Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    };
    if host.is_empty() {
        url.to_string()
    } else {
        host.to_string()
    }
}
