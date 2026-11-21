use super::file::*;
use super::*;

#[test]
fn a_test_build_keeps_off_the_installed_bytehives_hub_file() {
    // debug_assertions: this is a test build
    std::env::remove_var("BYTEHIVE_DEV_BROWSER");
    assert!(!owns_browser());
    assert!(own_hub_file().unwrap().ends_with("browser-hub.dev.json"));
    assert!(hub_file().unwrap().ends_with("browser-hub.json"), "the host always reads the installed one");
    // asked for on purpose: it takes the browser like the installed app
    std::env::set_var("BYTEHIVE_DEV_BROWSER", "1");
    assert!(owns_browser());
    assert_eq!(own_hub_file(), hub_file());
    std::env::remove_var("BYTEHIVE_DEV_BROWSER");
}

#[test]
fn a_hub_file_naming_a_hub_that_left_is_taken_back() {
    let own = 6967;
    let alive = |p: u16| p == 4000;
    // missing or unreadable: publish
    assert!(should_claim(None, own, alive));
    assert!(should_claim(Some("not json"), own, alive));
    // already ours: leave it
    assert!(!should_claim(Some(r#"{"port":6967,"token":"t"}"#), own, alive));
    // another ByteHive that still answers (the one opened last): leave it
    assert!(!should_claim(Some(r#"{"port":4000,"token":"t"}"#), own, alive));
    // one that closed and left its file behind: take it back
    assert!(should_claim(Some(r#"{"port":3710,"token":"t"}"#), own, alive));
}

fn peer(hub: &Hub) -> (u64, mpsc::Receiver<String>) {
    let (tx, rx) = mpsc::channel();
    (hub.add(tx), rx)
}
fn last(rx: &mpsc::Receiver<String>) -> Value {
    let mut v = Value::Null;
    while let Ok(s) = rx.try_recv() {
        v = serde_json::from_str(&s).unwrap();
    }
    v
}

#[test]
fn a_peer_without_the_token_is_dropped() {
    let hub = Hub::for_test("t", |_, _| String::new());
    let (a, _rx) = peer(&hub);
    assert!(!hub.on_message(a, r#"{"type":"hello","role":"agent","token":"nope"}"#));
}

#[test]
fn a_call_reaches_the_only_browser_and_the_answer_comes_back() {
    let hub = Hub::for_test("t", |b, e| if b == "Google Chrome" && e == "me@x.com" { "GravityCare".into() } else { String::new() });
    let (br, brx) = peer(&hub);
    let (ag, arx) = peer(&hub);
    assert!(hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#));
    assert!(hub.on_message(br, r#"{"type":"browser_info","browser":"Google Chrome","email":"me@x.com","version":"0.1.0"}"#));
    assert_eq!(hub.browsers()[0].label(), "Google Chrome · GravityCare");
    assert!(hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#));
    last(&arx);
    last(&brx);

    hub.on_message(ag, r#"{"type":"call","id":7,"tool":"tabs","args":{}}"#);
    let fwd = last(&brx);
    assert_eq!(fwd["agent"], "Ana");
    assert_eq!(fwd["tool"], "tabs");
    let hid = fwd["id"].as_u64().unwrap();

    hub.on_message(br, &json!({ "type": "result", "id": hid, "result": { "content": [] } }).to_string());
    let back = last(&arx);
    assert_eq!(back["type"], "result");
    assert_eq!(back["id"], 7);
}

#[test]
fn with_two_browsers_the_agent_must_pick_one() {
    let hub = Hub::for_test("t", |_, e| if e == "a@x.com" { "Work".into() } else { "GravityCare".into() });
    let (b1, _r1) = peer(&hub);
    let (b2, r2) = peer(&hub);
    let (ag, arx) = peer(&hub);
    for (b, e) in [(b1, "a@x.com"), (b2, "b@x.com")] {
        hub.on_message(b, r#"{"type":"hello","role":"browser","token":"t"}"#);
        hub.on_message(b, &json!({ "type": "browser_info", "browser": "Google Chrome", "email": e }).to_string());
    }
    hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#);
    hub.on_message(ag, r#"{"type":"call","id":1,"tool":"tabs","args":{}}"#);
    let e = last(&arx);
    assert_eq!(e["type"], "error");
    assert!(e["error"].as_str().unwrap().contains("browser_select"));

    hub.on_message(ag, r#"{"type":"call","id":2,"tool":"select_browser","args":{"browser":"gravitycare"}}"#);
    assert_eq!(last(&arx)["type"], "result");
    hub.on_message(ag, r#"{"type":"call","id":3,"tool":"tabs","args":{}}"#);
    assert_eq!(last(&r2)["tool"], "tabs");
}

#[test]
fn a_stopped_agent_is_refused_until_let_go_on() {
    let hub = Hub::for_test("t", |_, _| "P".into());
    let (br, brx) = peer(&hub);
    let (ag, arx) = peer(&hub);
    hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
    hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#);
    hub.on_message(ag, r#"{"type":"call","id":1,"tool":"computer","args":{"action":"left_click"}}"#);
    let a = &hub.activity()[0];
    assert_eq!((a.agent.as_str(), a.tool.as_str(), a.action.as_str(), a.paused), ("Ana", "computer", "left_click", false));
    last(&brx);

    hub.set_paused("Ana", true);
    hub.on_message(ag, r#"{"type":"call","id":2,"tool":"navigate","args":{"url":"x"}}"#);
    let e = last(&arx);
    assert_eq!((e["type"].as_str(), e["id"].as_u64()), (Some("error"), Some(2)));
    assert!(e["error"].as_str().unwrap().contains("wait"));
    assert_eq!(last(&brx), Value::Null, "nothing reached the browser");

    hub.set_paused("Ana", false);
    hub.on_message(ag, r#"{"type":"call","id":3,"tool":"navigate","args":{"url":"x"}}"#);
    assert_eq!(last(&brx)["tool"], "navigate");
}

#[test]
fn bytehive_can_ask_a_browser_for_an_agent() {
    let hub = std::sync::Arc::new(Hub::for_test("t", |_, _| "P".into()));
    let (br, brx) = peer(&hub);
    hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
    last(&brx);
    let h = hub.clone();
    let t = std::thread::spawn(move || h.app_call("Ana", "peek", json!({}), Duration::from_secs(2)));
    let fwd = loop {
        if let Ok(s) = brx.recv_timeout(Duration::from_secs(2)) { break serde_json::from_str::<Value>(&s).unwrap(); }
    };
    assert_eq!((fwd["tool"].as_str(), fwd["agent"].as_str()), (Some("peek"), Some("Ana")));
    hub.on_message(br, &json!({ "type": "result", "id": fwd["id"], "result": { "content": [] } }).to_string());
    assert_eq!(t.join().unwrap().unwrap(), json!({ "content": [] }));
}

#[test]
fn a_browser_on_an_old_extension_is_told_to_reload_once() {
    let mut hub = Hub::for_test("t", |_, _| "P".into());
    hub.wanted_version = Box::new(|| Some("0.2.0".into()));
    let (br, brx) = peer(&hub);
    hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
    last(&brx);
    hub.on_message(br, r#"{"type":"browser_info","browser":"Google Chrome","email":"a@x.com","version":"0.1.0"}"#);
    assert_eq!(last(&brx)["tool"], "reload");
    hub.on_message(br, r#"{"type":"browser_info","browser":"Google Chrome","email":"a@x.com","version":"0.1.0"}"#);
    assert_eq!(last(&brx), Value::Null, "only once, so a folder that never updates can't loop");
}

#[test]
fn a_call_to_an_out_of_date_browser_updates_it_first() {
    let mut hub = Hub::for_test("t", |_, _| "P".into());
    let want = std::sync::Arc::new(Mutex::new("0.1.0".to_string()));
    let w = want.clone();
    hub.wanted_version = Box::new(move || Some(w.lock().unwrap_or_else(|p| p.into_inner()).clone()));
    let (br, brx) = peer(&hub);
    let (ag, arx) = peer(&hub);
    hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
    hub.on_message(br, r#"{"type":"browser_info","browser":"Google Chrome","email":"a@x.com","version":"0.1.0"}"#);
    hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#);
    last(&brx);
    last(&arx);
    *want.lock().unwrap_or_else(|p| p.into_inner()) = "0.1.1".into(); // the extension on disk was just edited
    hub.on_message(ag, r#"{"type":"call","id":1,"tool":"tabs","args":{}}"#);
    assert_eq!(last(&brx)["tool"], "reload");
    assert!(last(&arx)["error"].as_str().unwrap().contains("Try again"));
    hub.on_message(ag, r#"{"type":"call","id":2,"tool":"tabs","args":{}}"#);
    assert_eq!(last(&brx)["tool"], "tabs", "only one reload per version");
}

#[test]
fn a_risky_click_waits_for_the_user() {
    let hub = Hub::for_test("t", |_, _| "P".into());
    let (br, brx) = peer(&hub);
    let (ag, arx) = peer(&hub);
    hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
    hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#);
    last(&arx);
    hub.on_message(ag, r#"{"type":"call","id":5,"tool":"computer","args":{"action":"left_click","ref":"ref_3"}}"#);
    let fwd = last(&brx);
    assert_eq!(fwd["args"]["_ask"], true);
    let held = json!({ "type": "result", "id": fwd["id"], "result": { "content": [], "needsConfirm": { "what": "Click \"Đăng\"", "url": "https://facebook.com" } } });
    hub.on_message(br, &held.to_string());
    assert_eq!(last(&arx), json!({ "type": "hold", "id": 5 }));
    let a = hub.asks();
    assert_eq!((a.len(), a[0].agent.as_str(), a[0].what.as_str()), (1, "Ana", "Click \"Đăng\""));

    hub.answer(a[0].id, true);
    let again = last(&brx);
    assert_eq!((again["args"]["confirmed"].as_bool(), again["args"]["ref"].as_str()), (Some(true), Some("ref_3")));
    hub.on_message(br, &json!({ "type": "result", "id": again["id"], "result": { "content": [] } }).to_string());
    assert_eq!((last(&arx)["type"].as_str(), hub.asks().len()), (Some("result"), 0));

    // An OK that comes after the agent gave up clicks nothing.
    hub.on_message(ag, r#"{"type":"call","id":9,"tool":"computer","args":{"action":"left_click","ref":"ref_9"}}"#);
    let fwd = last(&brx);
    hub.on_message(br, &json!({ "type": "result", "id": fwd["id"], "result": { "content": [], "needsConfirm": { "what": "Click \"Mua\"" } } }).to_string());
    let late = hub.asks()[0].id;
    hub.inner.lock().unwrap_or_else(|p| p.into_inner()).held.get_mut(&late).unwrap().at -= HOLD_MS;
    assert!(hub.asks().is_empty(), "an expired ask is no longer shown");
    hub.answer(late, true);
    assert_eq!(last(&brx), Value::Null, "a late OK never reaches the page");
    last(&arx);

    hub.on_message(ag, r#"{"type":"call","id":6,"tool":"computer","args":{"action":"left_click","ref":"ref_4"}}"#);
    let fwd = last(&brx);
    hub.on_message(br, &json!({ "type": "result", "id": fwd["id"], "result": { "content": [], "needsConfirm": { "what": "Click \"Gửi\"" } } }).to_string());
    hub.answer(hub.asks()[0].id, false);
    let no = last(&arx);
    assert_eq!((no["type"].as_str(), no["id"].as_u64()), (Some("error"), Some(6)));
    assert_eq!(last(&brx), Value::Null, "a refused click never reaches the page");
}

#[test]
fn a_browser_that_leaves_fails_its_pending_calls() {
    let hub = Hub::for_test("t", |_, _| "P".into());
    let (br, _brx) = peer(&hub);
    let (ag, arx) = peer(&hub);
    hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
    hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#);
    hub.on_message(ag, r#"{"type":"call","id":"x","tool":"tabs","args":{}}"#);
    hub.remove(br);
    let e = last(&arx);
    assert_eq!(e["type"], "error");
    assert_eq!(e["id"], "x");
}

mod desktop {
    use super::*;
    use std::time::Duration;

    /// A hub with one browser and one agent called Ana, the call already forwarded.
    fn ready(hub: &Hub) -> (u64, mpsc::Receiver<String>, mpsc::Receiver<String>, u64) {
        let (br, brx) = peer(hub);
        let (ag, arx) = peer(hub);
        hub.on_message(br, r#"{"type":"hello","role":"browser","token":"t"}"#);
        hub.on_message(ag, r#"{"type":"hello","role":"agent","token":"t","agent":"Ana"}"#);
        last(&arx);
        last(&brx);
        hub.on_message(ag, r#"{"type":"call","id":7,"tool":"computer","args":{"action":"left_click","ref":"d3"}}"#);
        let hid = last(&brx)["id"].as_u64().unwrap();
        (br, brx, arx, hid)
    }

    fn wait(rx: &mpsc::Receiver<String>) -> Value {
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).expect("an answer")).unwrap()
    }

    const DESK: &str = r#"{"desktop":{"title":"Extensions","url":"chrome://extensions/"}}"#;

    #[test]
    fn a_locked_page_is_done_from_windows_and_answered_as_usual() {
        let hub = Arc::new(Hub::for_test("t", |_, _| String::new()));
        hub.set_me();
        hub.set_desktop(Box::new(|agent, tool, args, desk| {
            assert_eq!((agent, tool, args["ref"].as_str(), desk["url"].as_str()), ("Ana", "computer", Some("d3"), Some("chrome://extensions/")));
            Ok(json!({ "content": [{ "type": "text", "text": "Clicked." }] }))
        }));
        let (br, _brx, arx, hid) = ready(&hub);
        hub.on_message(br, &format!(r#"{{"type":"result","id":{hid},"result":{DESK}}}"#));
        let back = wait(&arx);
        assert_eq!(back["type"], "result");
        assert_eq!(back["id"], 7);
        assert_eq!(back["result"]["content"][0]["text"], "Clicked.");
    }

    #[test]
    fn a_risky_desktop_step_waits_for_the_user_like_any_click() {
        let hub = Arc::new(Hub::for_test("t", |_, _| String::new()));
        hub.set_me();
        hub.set_desktop(Box::new(|_, _, args, _| {
            if args["confirmed"] == true {
                return Ok(json!({ "content": [{ "type": "text", "text": "Removed." }] }));
            }
            Ok(json!({ "content": [], "needsConfirm": { "what": "Click \"Remove\"", "url": "chrome://extensions/" } }))
        }));
        let (br, brx, arx, hid) = ready(&hub);
        hub.on_message(br, &format!(r#"{{"type":"result","id":{hid},"result":{DESK}}}"#));
        assert_eq!(wait(&arx)["type"], "hold");
        let ask = hub.asks();
        assert_eq!(ask[0].what, "Click \"Remove\"");
        hub.answer(ask[0].id, true);
        let again = last(&brx);
        assert_eq!(again["args"]["confirmed"], true);
        hub.on_message(br, &json!({ "type": "result", "id": again["id"], "result": serde_json::from_str::<Value>(DESK).unwrap() }).to_string());
        assert_eq!(wait(&arx)["result"]["content"][0]["text"], "Removed.");
    }

    #[test]
    fn without_the_apps_hook_the_agent_is_told_to_ask_the_user() {
        let hub = Hub::for_test("t", |_, _| String::new());
        let (br, _brx, arx, hid) = ready(&hub);
        hub.on_message(br, &format!(r#"{{"type":"result","id":{hid},"result":{DESK}}}"#));
        let back = last(&arx);
        assert_eq!(back["type"], "error");
        assert_eq!(back["error"], super::super::desktop::NO_DESKTOP);
    }
}
