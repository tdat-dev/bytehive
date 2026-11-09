//! ByteHive Browser, the pure part: what an agent's last browser step was in
//! words, when the live view shows, how a held click reads, and the frame in
//! a peek answer. Port of browserview.ts's helpers, its exact words.

use browser_hub::{Activity, Ask};

/// The card shows while the agent browsed within this long, or is stopped.
pub const RECENT_MS: u64 = 90_000;
/// A frame about once a second while its tab is in front.
pub const FRAME_MS: u64 = 1200;
/// A tab behind another one costs Chrome a paint per frame: fewer of them.
pub const BACK_FRAME_MS: u64 = 3000;
/// Same as the hub's HOLD_MS: after this the agent has given up waiting.
pub const HOLD_MS: u64 = browser_hub::hub::HOLD_MS;

pub const BEHIND: &str = "Behind another tab in Chrome, so it updates every few seconds";
pub const NO_PICTURE: &str = "No picture right now";

/// What an agent's last browser step was, in words.
pub fn describe(tool: &str, action: &str) -> &'static str {
    if tool == "computer" {
        return match action {
            "screenshot" => "Took a screenshot",
            "left_click" | "triple_click" => "Clicked",
            "double_click" => "Double-clicked",
            "right_click" => "Right-clicked",
            "hover" => "Pointed at something",
            "scroll" => "Scrolled",
            "type" => "Typed",
            "key" => "Pressed a key",
            "wait" => "Waited",
            _ => "Used the page",
        };
    }
    match tool {
        "tab_new" => "Opened a tab",
        "tab_adopt" => "Took over a tab",
        "tab_close" => "Closed a tab",
        "tabs" => "Checked its tabs",
        "navigate" => "Went to a page",
        "read_page" => "Read the page",
        "find" => "Looked for something",
        "page_text" => "Read the text",
        "form_input" => "Filled in a field",
        "javascript" => "Ran a script on the page",
        "console" => "Read the console",
        "network" => "Read the network log",
        "dialog" => "Answered a dialog",
        _ => "Used the browser",
    }
}

/// The card is up when the agent browsed recently, or you stopped it.
pub fn shows(a: Option<&Activity>, now: u64) -> bool {
    a.is_some_and(|a| a.paused || (a.browser > 0 && now.saturating_sub(a.at) < RECENT_MS))
}

/// An ask still waits: the agent has not given up on it.
pub fn live(a: &Ask, now: u64) -> bool {
    a.at == 0 || now.saturating_sub(a.at) < HOLD_MS
}

/// "Ana wants to click "Send"", from the hub's `Click "Send"`.
pub fn ask_line(agent: &str, what: &str) -> String {
    let mut c = what.chars();
    let first = c.next().map(|f| f.to_lowercase().collect::<String>()).unwrap_or_default();
    format!("{agent} wants to {first}{}", c.as_str())
}

/// "facebook.com" from a URL, without the www.
pub fn host(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return String::new() };
    let h = rest.split(['/', '?', '#']).next().unwrap_or("");
    let h = h.rsplit_once('@').map_or(h, |(_, h)| h);
    h.strip_prefix("www.").unwrap_or(h).to_string()
}

/// "just now", "12s ago", "3m ago".
pub fn ago(ms: u64) -> String {
    let s = (ms + 500) / 1000;
    if s < 5 {
        "just now".into()
    } else if s < 60 {
        format!("{s}s ago")
    } else {
        format!("{}m ago", (s + 30) / 60)
    }
}

/// The line under the frame: what it did and when, or that you stopped it.
pub fn what_line(a: &Activity, now: u64) -> String {
    if a.paused {
        return "Stopped by you".into();
    }
    format!("{} \u{00B7} {}", describe(&a.tool, &a.action), ago(now.saturating_sub(a.at)))
}

/// The note on the frame after a failed peek.
pub fn failed_note(error: &str) -> &'static str {
    if error.contains("isn't showing") { BEHIND } else { NO_PICTURE }
}

/// The base64 JPEG in a peek answer, and whether its tab is in front.
pub fn frame_of(answer: &serde_json::Value) -> (Option<String>, bool) {
    let data = answer["content"].as_array().into_iter().flatten().find(|c| c["type"] == "image").and_then(|c| c["data"].as_str()).map(str::to_string);
    let front = answer["tab"]["active"].as_bool() != Some(false);
    (data, front)
}

/// What a page that wants a person says (onBlocker): the note and, for a
/// window you are away from, the notice.
pub fn blocker_note(agent: &str, kind: &str, url: &str) -> String {
    let what = if kind == "captcha" { "a CAPTCHA" } else { "a login page" };
    let at = host(url);
    let where_ = if at.is_empty() { String::new() } else { format!(" on {at}") };
    format!("{agent} reached {what}{where_}. Handle it in Chrome, then tell it to carry on.")
}

#[cfg(test)]
mod tests {
    use browser_hub::{Activity, Ask};

    use super::{ago, ask_line, blocker_note, describe, failed_note, frame_of, host, live, shows, what_line, BEHIND, HOLD_MS, NO_PICTURE, RECENT_MS};

    fn act(browser: u64, at: u64, paused: bool) -> Activity {
        Activity { agent: "Ana".into(), browser, label: "Chrome".into(), tool: "computer".into(), action: "left_click".into(), at, paused }
    }

    #[test]
    fn says_what_it_did() {
        assert_eq!(describe("computer", "left_click"), "Clicked");
        assert_eq!(describe("computer", "zoom"), "Used the page");
        assert_eq!(describe("navigate", ""), "Went to a page");
        assert_eq!(describe("new_thing", ""), "Used the browser");
    }

    #[test]
    fn shows_while_recent_or_stopped() {
        assert!(shows(Some(&act(1, 1000, false)), 2000));
        assert!(!shows(Some(&act(1, 1000, false)), 1000 + RECENT_MS));
        assert!(shows(Some(&act(0, 0, true)), 10_000_000));
        assert!(!shows(Some(&act(0, 1000, false)), 1500), "no browser yet");
        assert!(!shows(None, 0));
    }

    #[test]
    fn reads_asks_hosts_and_times() {
        let a = Ask { id: 1, agent: "Ana".into(), what: "Click \"Send\"".into(), url: "https://www.facebook.com/x".into(), at: 10 };
        assert_eq!(ask_line(&a.agent, &a.what), "Ana wants to click \"Send\"");
        assert!(live(&a, 10 + HOLD_MS - 1) && !live(&a, 10 + HOLD_MS));
        assert_eq!(host(&a.url), "facebook.com");
        assert_eq!(host("http://user@localhost:3000/a?b"), "localhost:3000");
        assert_eq!(host("not a url"), "");
        assert_eq!(ago(1200), "just now");
        assert_eq!(ago(12_000), "12s ago");
        assert_eq!(ago(180_000), "3m ago");
        assert_eq!(what_line(&act(1, 0, true), 5), "Stopped by you");
        assert_eq!(what_line(&act(1, 0, false), 12_000), "Clicked \u{00B7} 12s ago");
    }

    #[test]
    fn finds_the_frame_in_a_peek() {
        let v = serde_json::json!({ "content": [{ "type": "text", "text": "x" }, { "type": "image", "data": "AAA" }], "tab": { "active": false } });
        assert_eq!(frame_of(&v), (Some("AAA".to_string()), false));
        assert_eq!(frame_of(&serde_json::json!({})), (None, true));
        assert_eq!(failed_note("The tab isn't showing"), BEHIND);
        assert_eq!(failed_note("The browser took too long."), NO_PICTURE);
    }

    #[test]
    fn a_page_that_wants_a_person_says_where() {
        assert_eq!(blocker_note("Ana", "captcha", "https://www.x.com/a"), "Ana reached a CAPTCHA on x.com. Handle it in Chrome, then tell it to carry on.");
        assert_eq!(blocker_note("Ana", "login", ""), "Ana reached a login page. Handle it in Chrome, then tell it to carry on.");
    }
}
