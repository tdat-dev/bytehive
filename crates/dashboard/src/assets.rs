//! What the server hands a browser: the dashboard page, the sign-in page,
//! and xterm.js served same-origin so the web terminal works offline.

pub(crate) const PAGE: &str = include_str!("../assets/dashboard.html");
pub(crate) const LOGIN_PAGE: &str = include_str!("../assets/dashboard-login.html");
pub(crate) const XTERM_JS: &str = include_str!("../../../node_modules/@xterm/xterm/lib/xterm.js");
pub(crate) const XTERM_CSS: &str = include_str!("../../../node_modules/@xterm/xterm/css/xterm.css");
pub(crate) const ADDON_FIT_JS: &str = include_str!("../../../node_modules/@xterm/addon-fit/lib/addon-fit.js");

#[cfg(test)]
mod tests {
    use super::*;

    /// Every fetch the embedded pages make carries what the server now wants.
    #[test]
    fn the_pages_send_json_and_the_maestro_header() {
        assert!(PAGE.contains(r#"const API = { "Content-Type": "application/json", "X-Maestro": "1" };"#));
        for (name, page) in [("dashboard", PAGE), ("login", LOGIN_PAGE)] {
            let calls: Vec<&str> = page.split("fetch(").skip(1).collect();
            assert!(!calls.is_empty(), "{name}");
            for call in calls {
                let call = &call[..call.find(';').unwrap_or(call.len())];
                assert!(call.contains("headers: API") || call.contains(r#""X-Maestro": "1""#), "{name}: fetch({call}");
            }
        }
    }
}
