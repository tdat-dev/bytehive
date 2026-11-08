// Screen blocks and usage limits: what stops an agent that only you clear,
// and when a plan limit lifts.

use chrono::{Local, TimeZone};
use supervise::screens::{blocked_on_screen, duration_ms, limit_on_screen, parse_reset, BlockedKind};

// Saturday 27 Sep 2026, 14:00 local time
fn now_ms() -> u64 {
    Local.with_ymd_and_hms(2026, 9, 27, 14, 0, 0).single().unwrap().timestamp_millis() as u64
}

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> u64 {
    Local.with_ymd_and_hms(y, mo, d, h, mi, 0).single().unwrap().timestamp_millis() as u64
}

#[test]
fn catches_sign_in_and_login_screens() {
    assert_eq!(blocked_on_screen("Please sign in to continue").map(|h| h.kind), Some(BlockedKind::Auth));
    assert_eq!(blocked_on_screen("You must log in to use Claude Code").map(|h| h.kind), Some(BlockedKind::Auth));
    assert_eq!(blocked_on_screen("Authentication expired. Please log in again.").map(|h| h.kind), Some(BlockedKind::Auth));
    assert_eq!(blocked_on_screen("Error: Not logged in. Run /login to authenticate.").map(|h| h.kind), Some(BlockedKind::Auth));
}

#[test]
fn catches_captcha_and_human_verification() {
    assert_eq!(blocked_on_screen("Please complete the CAPTCHA to continue").map(|h| h.kind), Some(BlockedKind::Captcha));
    assert_eq!(blocked_on_screen("Verify you are human to proceed").map(|h| h.kind), Some(BlockedKind::Captcha));
}

#[test]
fn catches_missing_or_invalid_api_keys() {
    assert_eq!(blocked_on_screen("Error: Invalid API key. Set a valid key.").map(|h| h.kind), Some(BlockedKind::Key));
    assert_eq!(blocked_on_screen("API key not configured").map(|h| h.kind), Some(BlockedKind::Key));
    assert_eq!(blocked_on_screen("Set your api key to continue").map(|h| h.kind), Some(BlockedKind::Key));
}

#[test]
fn keeps_the_line_that_said_so() {
    let hit = blocked_on_screen("Please sign in to continue").unwrap();
    assert_eq!(hit.kind, BlockedKind::Auth);
    assert_eq!(hit.what, "Please sign in to continue");
}

#[test]
fn sees_through_box_drawing() {
    assert_eq!(blocked_on_screen("\u{2502} Please sign in to continue \u{2502}").map(|h| h.kind), Some(BlockedKind::Auth));
}

#[test]
fn ignores_a_notice_scrolled_out_of_tail() {
    let mut lines = vec!["Please sign in to continue".to_string()];
    lines.extend((0..20).map(|i| format!("working line {i}")));
    assert_eq!(blocked_on_screen(&lines.join("\n")), None);
}

#[test]
fn says_nothing_on_a_normal_screen() {
    assert_eq!(blocked_on_screen("\u{280B} Reading src/app.ts\n> fix the login flow"), None);
    assert_eq!(blocked_on_screen(""), None);
}

#[test]
fn reads_a_wait() {
    assert_eq!(duration_ms("2 hours 13 minutes"), Some(2.0 * 3_600_000.0 + 13.0 * 60_000.0));
    let now = now_ms();
    assert_eq!(parse_reset("in 2 hours 13 minutes.", now), Some(now + 2 * 3_600_000 + 13 * 60_000));
    assert_eq!(parse_reset("in 3h 12m", now), Some(now + 3 * 3_600_000 + 12 * 60_000));
}

#[test]
fn reads_a_time_as_the_next_one() {
    let now = now_ms();
    assert_eq!(parse_reset("7pm (Asia/Saigon)", now), Some(at(2026, 9, 27, 19, 0)));
    assert_eq!(parse_reset("at 7:30 pm", now), Some(at(2026, 9, 27, 19, 30)));
    assert_eq!(parse_reset("1am", now), Some(at(2026, 9, 28, 1, 0)));
    assert_eq!(parse_reset("12:10am", now), Some(at(2026, 9, 28, 0, 10)));
}

#[test]
fn reads_a_day_and_a_time() {
    let now = now_ms();
    assert_eq!(parse_reset("Thu 1am", now), Some(at(2026, 10, 1, 1, 0)));
    assert_eq!(parse_reset("Oct 2, 1am", now), Some(at(2026, 10, 2, 1, 0)));
    assert_eq!(parse_reset("October 2 at 5:00 PM", now), Some(at(2026, 10, 2, 17, 0)));
    assert_eq!(parse_reset("soon", now), None);
    assert_eq!(parse_reset("42", now), None);
}

#[test]
fn sees_claude_limits_with_when_they_lift() {
    let now = now_ms();
    let screen = "\u{25CF} Done with the header.\n\n  \u{23BF}  You've hit your limit \u{00B7} resets 7pm (Asia/Saigon)\n     /upgrade to increase your usage limit.\n\n\u{256D}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256E}\n\u{2502} >      \u{2502}\n\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256F}\n  ? for shortcuts";
    let hit = limit_on_screen(screen, now).unwrap();
    assert!(hit.what.contains("You've hit your limit"));
    assert_eq!(hit.resets_at, Some(at(2026, 9, 27, 19, 0)));
    assert_eq!(limit_on_screen("5-hour limit reached \u{2219} resets 7pm\n> ", now).unwrap().resets_at, Some(at(2026, 9, 27, 19, 0)));
    assert_eq!(
        limit_on_screen("You've hit your weekly limit \u{00B7} resets Oct 2, 1am", now).unwrap().resets_at,
        Some(at(2026, 10, 2, 1, 0))
    );
}

#[test]
fn sees_codex_limits() {
    let now = now_ms();
    let hit = limit_on_screen("\u{25A0} You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again in 2 hours 13 minutes.\n\n\u{203A} ", now).unwrap();
    assert_eq!(hit.resets_at, Some(now + 2 * 3_600_000 + 13 * 60_000));
}

#[test]
fn sees_gemini_limits_even_without_a_time() {
    let hit = limit_on_screen("\u{2715} You have exhausted your daily quota on this model.\n> ", now_ms()).unwrap();
    assert_eq!(hit.resets_at, None);
}

#[test]
fn ignores_old_lines_and_talk_about_limits() {
    let mut lines = vec!["You've hit your limit \u{00B7} resets 7pm".to_string()];
    lines.extend((0..20).map(|i| format!("line {i}")));
    assert_eq!(limit_on_screen(&lines.join("\n"), now_ms()), None);
    assert_eq!(limit_on_screen("The API returns 429 when the rate limit is exceeded; we retry with backoff.\n> ", now_ms()), None);
}
