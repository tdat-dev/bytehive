//! Live terminals: the WebSocket and the SSE stream want a session and end
//! with it.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tungstenite::client::IntoClientRequest;

use std::net::{IpAddr, Ipv4Addr};

use crate::auth::{DashAuth, Login};
use crate::testkit::*;

/// An open terminal socket, read with a short timeout. Returns how long
/// after `end()` the server closed it (panics if it stays open 5 s).
fn socket_closes_after(srv: &TestServer, token: &str, end: impl FnOnce()) -> Duration {
    let url = format!("ws://127.0.0.1:{}/term?id=a", srv.ws_port);
    let mut req = url.as_str().into_client_request().unwrap();
    req.headers_mut().insert("Cookie", format!("maestro_dash={token}").parse().unwrap());
    let (mut ws, _) = tungstenite::connect(req).expect("a signed-in browser gets its terminal");
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_mut() {
        s.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    }
    let timed_out = |e: &tungstenite::Error| {
        matches!(e, tungstenite::Error::Io(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
    };
    // live first: the agent's output arrives
    let t = Instant::now();
    loop {
        match ws.read() {
            Ok(tungstenite::Message::Binary(_)) => break,
            Ok(_) => {}
            Err(e) if timed_out(&e) => {}
            Err(e) => panic!("closed before the session ended: {e}"),
        }
        assert!(t.elapsed() < Duration::from_secs(10), "no output from the agent");
    }
    end();
    let t = Instant::now();
    loop {
        match ws.read() {
            Ok(_) => {}
            Err(e) if timed_out(&e) => {}
            Err(_) => return t.elapsed(),
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the terminal stayed open");
    }
}

/// An open /term SSE stream. Returns how long after `end()` it ended.
fn stream_ends_after(srv: &TestServer, token: &str, end: impl FnOnce()) -> Duration {
    let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    s.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let req = format!(
        "GET /term?id=a HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\nX-Maestro: 1\r\nCookie: maestro_dash={token}\r\n\r\n",
        srv.port
    );
    s.write_all(req.as_bytes()).unwrap();
    // tiny_http holds a chunked body until 8 KB pile up, so the stream's
    // bytes may only show up when it ends: give the server time to open it
    let mut seen = Vec::new();
    let mut buf = [0u8; 4096];
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(1500) {
        match s.read(&mut buf) {
            Ok(0) => panic!("ended before the session did: {}", String::from_utf8_lossy(&seen)),
            Ok(n) => seen.extend_from_slice(&buf[..n]),
            Err(_) => {}
        }
    }
    end();
    let t = Instant::now();
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => seen.extend_from_slice(&buf[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the stream stayed open");
    }
    let took = t.elapsed();
    let seen = String::from_utf8_lossy(&seen);
    assert!(seen.starts_with("HTTP/1.1 200"), "it was open: {seen}");
    assert!(seen.contains("data: "), "with the agent's output");
    took
}

#[test]
fn signing_out_closes_an_open_terminal() {
    let auth = locked("correct horse");
    let reg = one_agent();
    let srv = start_with(auth.clone(), reg.clone());
    let token = session(&auth);
    let took = socket_closes_after(&srv, &token, || auth.logout(&token));
    assert!(took < Duration::from_millis(1500), "closed after {took:?}");
    let token = session(&auth);
    let took = stream_ends_after(&srv, &token, || auth.logout(&token));
    assert!(took < Duration::from_millis(1500), "ended after {took:?}");
    reg.lock().unwrap().clear();
}

#[test]
fn a_new_or_removed_password_closes_open_terminals() {
    let auth = locked("correct horse");
    let reg = one_agent();
    let srv = start_with(auth.clone(), reg.clone());
    let token = session(&auth);
    let took = stream_ends_after(&srv, &token, || auth.set_password("battery staple").unwrap());
    assert!(took < Duration::from_millis(1500), "ended after {took:?}");
    auth.set_password("correct horse").unwrap();
    let token = session(&auth);
    // with no password the dashboard is localhost-only and open, but a
    // terminal opened behind the old password still closes
    let took = socket_closes_after(&srv, &token, || auth.clear_password().unwrap());
    assert!(took < Duration::from_millis(1500), "closed after {took:?}");
    reg.lock().unwrap().clear();
}

#[test]
fn the_terminal_socket_wants_a_session_too() {
    let auth = locked("correct horse");
    let srv = start(auth.clone());
    let url = format!("ws://127.0.0.1:{}/term?id=a", srv.ws_port);
    match tungstenite::connect(url.as_str()) {
        Err(tungstenite::Error::Http(resp)) => assert_eq!(resp.status(), 401),
        other => panic!("expected 401, got {:?}", other.map(|_| ())),
    }
    let Login::Ok(token) = auth.login(IpAddr::V4(Ipv4Addr::LOCALHOST), "correct horse", Instant::now()) else { panic!() };
    let mut req = url.as_str().into_client_request().unwrap();
    req.headers_mut().insert("Cookie", format!("maestro_dash={token}").parse().unwrap());
    assert!(tungstenite::connect(req).is_ok(), "a signed-in browser gets its terminal");
}

#[test]
fn the_terminal_socket_checks_host_and_origin() {
    let srv = start(Arc::new(DashAuth::default()));
    let url = format!("ws://127.0.0.1:{}/term?id=a", srv.ws_port);
    let status = |req: tungstenite::handshake::client::Request| match tungstenite::connect(req) {
        Err(tungstenite::Error::Http(resp)) => resp.status().as_u16(),
        Ok(_) => 101,
        Err(e) => panic!("{e}"),
    };
    let mut req = url.as_str().into_client_request().unwrap();
    req.headers_mut().insert("Host", format!("evil.com:{}", srv.ws_port).parse().unwrap());
    assert_eq!(status(req), 403, "a rebound host");
    for origin in ["http://evil.com", "http://127.0.0.1:3000", "null"] {
        let mut req = url.as_str().into_client_request().unwrap();
        req.headers_mut().insert("Origin", origin.parse().unwrap());
        assert_eq!(status(req), 403, "{origin}");
    }
    let mut req = url.as_str().into_client_request().unwrap();
    req.headers_mut().insert("Origin", format!("http://127.0.0.1:{}", srv.port).parse().unwrap());
    assert_eq!(status(req), 101, "the dashboard page's own origin");
}
