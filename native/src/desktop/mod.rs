//! Driving the pages Chrome keeps from every extension (chrome://extensions,
//! the Web Store, settings, other extensions' pages) and the dialogs Chrome
//! opens (the folder chooser of "Load unpacked"), from Windows instead: read
//! through UI Automation, the way a screen reader does, and acted on through
//! its patterns or real input. Chrome takes both as a person's. The browser
//! hub calls `run` when the extension answers that a tab is such a page
//! (crates/browser-hub/src/hub/desktop.rs); agents use the same browser tools.
//! Measured 2026-11-21 on chrome://extensions: 291 elements read in 146 ms,
//! the search box filled and "Keyboard shortcuts" pressed, no flag and no restart.

mod act;
mod computer;
mod idle;
mod keys;
mod shot;
mod tree;
mod words;
mod worker;

use serde_json::Value;

/// One step on a guarded page: the agent, the tool and its args, and what the
/// extension said about the tab (title, url, window bounds).
pub fn run(agent: &str, tool: &str, args: &Value, desk: &Value) -> Result<Value, String> {
    worker::ask(agent, tool, args, desk)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    /// A hub of its own with this desktop path, for a throwaway Chrome whose
    /// native host reads `<BYTEHIVE_CHAIN_HOME>/.bytehive/browser-hub.json`
    /// (Chrome started with USERPROFILE set to that folder). Stays up 5 minutes.
    #[test]
    #[ignore]
    fn chain_hub() {
        let home = std::path::PathBuf::from(std::env::var("BYTEHIVE_CHAIN_HOME").expect("BYTEHIVE_CHAIN_HOME"));
        let hub = browser_hub::Hub::start(Box::new(|_| {}), Box::new(|| None)).unwrap();
        hub.set_ask_risky(true);
        hub.set_desktop(Box::new(super::run));
        let file = home.join(".bytehive").join("browser-hub.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let own = browser_hub::hub::own_hub_file().unwrap();
        std::fs::copy(&own, &file).unwrap();
        println!("hub on {} ({})", hub.port, file.display());
        std::thread::sleep(std::time::Duration::from_secs(300));
    }

    /// What the owner asked for: load an unpacked extension with no flag and no
    /// restart, through chrome://extensions and its folder chooser. Needs one
    /// Chrome window titled "Extensions" open on chrome://extensions, and the
    /// folder in BYTEHIVE_LOAD_DIR.
    #[test]
    #[ignore]
    fn load_unpacked_on_this_machine() {
        let dir = std::env::var("BYTEHIVE_LOAD_DIR").expect("BYTEHIVE_LOAD_DIR");
        // A new window shows New Tab (Chrome opens no chrome:// page from its
        // command line): type the address into Chrome's own address bar first.
        let start = json!({ "title": "New Tab", "url": "chrome://newtab/" });
        if let Ok(r) = super::run("Probe", "find", &json!({ "query": "Address and search bar" }), &start) {
            let t = r["content"][0]["text"].as_str().unwrap_or("").to_string();
            let bar = t.split(']').next().unwrap().trim_start_matches('[').to_string();
            super::run("Probe", "form_input", &json!({ "ref": bar, "value": "chrome://extensions" }), &start).unwrap();
            super::run("Probe", "computer", &json!({ "action": "key", "text": "Enter" }), &start).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let desk = json!({ "title": "Extensions", "url": "chrome://extensions/" });
        let go = |tool: &str, args: serde_json::Value| {
            let r = super::run("Probe", tool, &args, &desk).unwrap_or_else(|e| panic!("{tool} {args}: {e}"));
            let t = r["content"][0]["text"].as_str().unwrap_or("").to_string();
            println!("--- {tool} {args}\n{}", t.lines().take(12).collect::<Vec<_>>().join("\n"));
            t
        };
        let first_ref = |t: &str, name: &str| -> String {
            let l = t.lines().find(|l| l.contains(&format!("\"{name}\"")) && !l.contains("] text ")).unwrap_or_else(|| panic!("no {name} in:\n{t}"));
            l.split(']').next().unwrap().trim_start_matches('[').to_string()
        };
        let t = go("find", json!({ "query": "Developer mode" }));
        go("form_input", json!({ "ref": first_ref(&t, "Developer mode"), "value": true }));
        std::thread::sleep(std::time::Duration::from_millis(800));
        let t = go("find", json!({ "query": "Load unpacked" }));
        go("computer", json!({ "action": "left_click", "ref": first_ref(&t, "Load unpacked") }));
        std::thread::sleep(std::time::Duration::from_secs(2));
        let t = go("read_page", json!({}));
        let folder = t.lines().find(|l| l.contains("textbox") && l.contains("Folder")).map(|l| l.split(']').next().unwrap().trim_start_matches('[').to_string()).expect("the folder chooser's name box");
        go("form_input", json!({ "ref": folder, "value": dir }));
        go("computer", json!({ "action": "left_click", "ref": first_ref(&t, "Select Folder") }));
        std::thread::sleep(std::time::Duration::from_secs(2));
        let t = go("find", json!({ "query": "ByteHive" }));
        assert!(t.contains("ByteHive"), "the extension is loaded");
    }

    /// Switches Developer mode off and on again in the window titled
    /// "Extensions", and checks Load unpacked follows it.
    #[test]
    #[ignore]
    fn switch_on_this_machine() {
        let desk = json!({ "title": "Extensions" });
        let outline = || super::run("Probe", "read_page", &json!({}), &desk).unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        let dev = |t: &str| t.lines().find(|l| l.contains("button \"Developer mode\"")).unwrap().split(']').next().unwrap().trim_start_matches('[').to_string();
        for want in [false, true] {
            let r = dev(&outline());
            let said = super::run("Probe", "form_input", &json!({ "ref": r, "value": want }), &desk).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1200));
            let shown = outline().contains("\"Load unpacked\"");
            println!("{want}: {} / Load unpacked shown: {shown}", said["content"][0]["text"]);
            assert_eq!(shown, want);
        }
    }

    /// With Chrome behind another window: steps by UI Automation must leave the
    /// window in front alone, and a step that borrows real input must give it
    /// back. Needs one Chrome window titled "New Tab" that is not in front.
    #[test]
    #[ignore]
    fn behind_on_this_machine() {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let front = || unsafe { GetForegroundWindow() } as isize;
        let before = front();
        let go = |title: &str, tool: &str, args: serde_json::Value| {
            let r = super::run("Probe", tool, &args, &json!({ "title": title })).unwrap_or_else(|e| panic!("{tool} {args}: {e}"));
            let t = r["content"][0]["text"].as_str().unwrap_or("").to_string();
            let kept = front() == before;
            if !kept {
                super::keys::give_back(before);
            }
            println!("--- {tool} {args} (front kept: {kept})\n{}", t.lines().take(6).collect::<Vec<_>>().join("\n"));
            t
        };
        let r_of = |t: &str, pat: &str| t.lines().find(|l| l.contains(pat)).unwrap_or_else(|| panic!("no {pat} in {t}")).split(']').next().unwrap().trim_start_matches('[').to_string();
        let start = std::env::var("BYTEHIVE_START_TITLE").unwrap_or_else(|_| "New Tab".into());
        let t = go(&start, "find", json!({ "query": "Address and search bar" }));
        go(&start, "form_input", json!({ "ref": r_of(&t, "textbox"), "value": "chrome://extensions" }));
        go(&start, "computer", json!({ "action": "key", "text": "Enter" }));
        std::thread::sleep(std::time::Duration::from_secs(2));
        let t = go("Extensions", "find", json!({ "query": "Search extensions" }));
        go("Extensions", "computer", json!({ "action": "left_click", "ref": r_of(&t, "\"Search extensions\"") }));
        let t = go("Extensions", "find", json!({ "query": "Search extensions" }));
        if let Some(l) = t.lines().find(|l| l.contains("textbox")) {
            let r = l.split(']').next().unwrap().trim_start_matches('[');
            go("Extensions", "computer", json!({ "action": "left_click", "ref": r }));
            go("Extensions", "computer", json!({ "action": "type", "text": "Docs" }));
            assert!(go("Extensions", "find", json!({ "query": "Search extensions" })).contains("value=\"Docs\""), "typed without keys");
        }
        go("Extensions", "computer", json!({ "action": "scroll", "scroll_direction": "down", "scroll_amount": 2 }));
        let t = go("Extensions", "find", json!({ "query": "Developer mode" }));
        let at = t.lines().find(|l| l.contains("button \"Developer mode\"")).unwrap().rsplit('@').next().unwrap().split(' ').next().unwrap().to_string();
        let (x, y) = at.split_once(',').unwrap();
        go("Extensions", "computer", json!({ "action": "left_click", "coordinate": [x.parse::<i32>().unwrap(), y.parse::<i32>().unwrap()] }));
        go("Extensions", "computer", json!({ "action": "key", "text": "Escape" }));
    }

    /// Prints how long the person has been away from the real keyboard and mouse.
    #[test]
    #[ignore]
    fn idle_on_this_machine() {
        super::idle::watch();
        let seen: Vec<u32> = (0..12).map(|_| { std::thread::sleep(std::time::Duration::from_millis(500)); super::idle::idle_ms() }).collect();
        println!("real idle ms: {seen:?}");
    }

    /// Prints the outline of the Chrome window titled BYTEHIVE_READ_TITLE.
    #[test]
    #[ignore]
    fn read_on_this_machine() {
        let title = std::env::var("BYTEHIVE_READ_TITLE").expect("BYTEHIVE_READ_TITLE");
        let r = super::run("Probe", "read_page", &json!({}), &json!({ "title": title })).unwrap();
        println!("{}", r["content"][0]["text"].as_str().unwrap());
        if let Ok(out) = std::env::var("BYTEHIVE_SHOT_OUT") {
            use base64::Engine;
            let s = super::run("Probe", "computer", &json!({ "action": "screenshot" }), &json!({ "title": title })).unwrap();
            std::fs::write(out, base64::engine::general_purpose::STANDARD.decode(s["content"][0]["data"].as_str().unwrap()).unwrap()).unwrap();
        }
    }

    /// Drives a real chrome://extensions tab on this machine: open one first.
    /// `cargo test desktop::tests::on_this_machine -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn on_this_machine() {
        let desk = json!({ "title": "Extensions", "url": "chrome://extensions/" });
        let read = super::run("Probe", "read_page", &json!({}), &desk).unwrap();
        let text = read["content"][0]["text"].as_str().unwrap().to_string();
        println!("{}", text.lines().take(14).collect::<Vec<_>>().join("\n"));
        let mine = text.lines().find(|l| l.contains("\"My extensions\"")).expect("My extensions listed");
        let r = mine.split(']').next().unwrap().trim_start_matches('[');
        let done = super::run("Probe", "computer", &json!({ "action": "left_click", "ref": r, "_ask": true }), &desk).unwrap();
        println!("click: {}", done["content"][0]["text"]);
        let held = super::run("Probe", "find", &json!({ "query": "Remove" }), &desk).unwrap();
        let remove = held["content"][0]["text"].as_str().unwrap().lines().next().unwrap().to_string();
        let r = remove.split(']').next().unwrap().trim_start_matches('[');
        let ask = super::run("Probe", "computer", &json!({ "action": "left_click", "ref": r, "_ask": true }), &desk).unwrap();
        assert!(ask["needsConfirm"].is_object(), "Remove must wait for the user: {ask}");
        let shot = super::run("Probe", "computer", &json!({ "action": "screenshot" }), &desk).unwrap();
        let data = shot["content"][0]["data"].as_str().unwrap();
        println!("screenshot: {} bytes of base64, {}", data.len(), shot["content"][1]["text"]);
        if let Ok(out) = std::env::var("BYTEHIVE_SHOT_OUT") {
            use base64::Engine;
            std::fs::write(out, base64::engine::general_purpose::STANDARD.decode(data).unwrap()).unwrap();
        }
    }
}
