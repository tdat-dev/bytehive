//! The hub over its real socket: a browser and an agent dial in over
//! WebSocket the way the native host and bytehive-mcp do, and a call goes
//! there and back. Nothing is published: the listener is this test's own.

use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use super::route::serve;
use super::Hub;

fn hub_on_a_port() -> (Arc<Hub>, u16) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let hub = Arc::new(Hub::for_test("tok", |_, _| "Work".into()));
    let h = hub.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let h = h.clone();
            std::thread::spawn(move || serve(h, stream));
        }
    });
    (hub, port)
}

fn dial(port: u16, hello: Value) -> WebSocket<TcpStream> {
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let (mut ws, _) = tungstenite::client(format!("ws://127.0.0.1:{port}/"), stream).unwrap();
    ws.send(Message::Text(hello.to_string())).unwrap();
    assert_eq!(next(&mut ws)["type"], "hello_ok");
    ws
}

fn next(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        match ws.read().expect("an answer within 5 s") {
            Message::Text(t) => return serde_json::from_str(&t).unwrap(),
            _ => continue,
        }
    }
}

fn wait_until(what: &str, ok: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_call_goes_over_the_socket_to_the_browser_and_back() {
    let (hub, port) = hub_on_a_port();
    let mut browser = dial(port, json!({ "type": "hello", "role": "browser", "token": "tok" }));
    browser.send(Message::Text(json!({ "type": "browser_info", "browser": "Google Chrome", "email": "me@x.com", "version": "1.0.0" }).to_string())).unwrap();
    wait_until("the browser to say who it is", || hub.browsers().first().is_some_and(|b| b.browser == "Google Chrome"));
    assert_eq!(hub.browsers()[0].label(), "Google Chrome · Work");

    let mut agent = dial(port, json!({ "type": "hello", "role": "agent", "token": "tok", "agent": "Ana" }));
    agent.send(Message::Text(json!({ "type": "call", "id": 41, "tool": "tabs", "args": {} }).to_string())).unwrap();
    let call = next(&mut browser);
    assert_eq!((call["type"].as_str(), call["agent"].as_str(), call["tool"].as_str()), (Some("call"), Some("Ana"), Some("tabs")));
    assert_eq!(call["args"]["_ask"], true, "risky clicks are held unless turned off");

    let answer = json!({ "type": "result", "id": call["id"], "result": { "content": [{ "type": "text", "text": "1 tab" }] } });
    browser.send(Message::Text(answer.to_string())).unwrap();
    let back = next(&mut agent);
    assert_eq!((back["type"].as_str(), back["id"].as_u64()), (Some("result"), Some(41)));
    assert_eq!(back["result"]["content"][0]["text"], "1 tab");
    assert_eq!(hub.activity()[0].tool, "tabs");

    // The browser leaves: the hub notices and agents hear there is none.
    drop(browser);
    wait_until("the browser to be gone", || hub.browsers().is_empty());
    agent.send(Message::Text(json!({ "type": "call", "id": 42, "tool": "tabs", "args": {} }).to_string())).unwrap();
    let none = next(&mut agent);
    assert_eq!(none["type"], "error");
    assert!(none["error"].as_str().unwrap().contains("No browser is connected"));
}

#[test]
fn a_peer_with_the_wrong_token_is_hung_up_on() {
    let (_hub, port) = hub_on_a_port();
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let (mut ws, _) = tungstenite::client(format!("ws://127.0.0.1:{port}/"), stream).unwrap();
    ws.send(Message::Text(json!({ "type": "hello", "role": "agent", "token": "nope" }).to_string())).unwrap();
    let closed = loop {
        match ws.read() {
            Ok(Message::Text(t)) => panic!("no answer for a stranger, got {t}"),
            Ok(_) => continue,
            Err(_) => break true,
        }
    };
    assert!(closed);
}
