//! What the desktop path tells an agent, and which steps wait for the user.

use std::sync::OnceLock;

use regex::Regex;

/// Names of buttons that install, remove, turn off, pay or send: on a page
/// Chrome guards (extensions, the Web Store, settings), such a click waits for
/// the user's OK in ByteHive, like a risky click anywhere else.
pub fn risky(name: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(remove|uninstall|delete|add to chrome|add extension|add app|install|reset|clear|turn off|disable|sign out|log out|block|report|pay|buy|purchase|send|post|submit|publish|extension enabled)\b").expect("regex")
    })
    .is_match(name)
}

/// The answer that holds a step for the user.
pub fn hold(what: &str, url: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": format!("Waiting for the user to allow: {what}.") }],
        "needsConfirm": { "what": what, "url": url },
    })
}

pub fn text(s: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "content": [{ "type": "text", "text": s.into() }] })
}

/// One line of a page outline: `[d3] button "Load unpacked" @120,240`.
pub fn line(n: usize, kind: &str, name: &str, x: i32, y: i32, extra: &str) -> String {
    let name = name.replace('\n', " ");
    let name = if name.chars().count() > 120 { format!("{}…", name.chars().take(120).collect::<String>()) } else { name };
    format!("[d{n}] {kind} \"{name}\" @{x},{y}{extra}")
}

/// `d12` to the index 11.
pub fn parse_ref(r: &str) -> Result<usize, String> {
    let n = r.trim().strip_prefix('d').and_then(|n| n.parse::<usize>().ok()).filter(|n| *n > 0);
    n.map(|n| n - 1).ok_or_else(|| format!("\"{r}\" is not a ref of this page: here refs look like d12, from read_page or find."))
}

pub const HEAD_NOTE: &str = "Chrome keeps this page from extensions, so ByteHive reads and drives it from Windows: refs start with d, coordinates are pixels of a screenshot of the whole window, and the window is brought to the front while it works.";

pub const NO_SCRIPT: &str = "Chrome runs no scripts for extensions on this page. Use read_page, find, page_text, form_input and computer here.";

pub const NO_UPLOAD: &str = "On this page, click the button that opens the file or folder chooser, then read_page: the chooser is listed, so fill its name box with form_input and press its button (Open, Select Folder).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_that_change_chrome_wait_and_the_rest_go() {
        for name in ["Remove", "Add to Chrome", "Uninstall", "On, extension enabled", "Reset settings", "Add extension"] {
            assert!(risky(name), "{name}");
        }
        for name in ["Load unpacked", "Details", "Developer mode", "Search extensions", "Keyboard shortcuts", "Select Folder", "Removed"] {
            assert!(!risky(name), "{name}");
        }
    }

    #[test]
    fn a_ref_is_d_and_a_number() {
        assert_eq!(parse_ref("d12"), Ok(11));
        assert!(parse_ref("ref_12").is_err());
        assert!(parse_ref("d0").is_err());
    }

    #[test]
    fn a_line_names_the_element_where_it_is() {
        assert_eq!(line(3, "button", "Load unpacked", 120, 240, ""), "[d3] button \"Load unpacked\" @120,240");
        assert!(line(1, "text", &"x".repeat(200), 0, 0, "").ends_with("…\" @0,0"));
    }
}
