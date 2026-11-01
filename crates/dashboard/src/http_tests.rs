//! The server end to end: the guard, the password gate, sign-in and out,
//! and restarts on the same port.

use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_core::registry::Registry;

use crate::assets::LOGIN_PAGE;
use crate::auth::DashAuth;
use crate::guard::Guard;
use crate::http::{serve, Ctx};
use crate::stream::serve_ws;
use crate::testkit::*;
use crate::{bind_both, finish, Dashboard, Inner};

#[test]
fn without_a_password_it_works_as_before() {
    let srv = start(Arc::new(DashAuth::default()));
    let page = http(srv.port, "GET", "/", None, "");
    assert_eq!(page.status, 200);
    assert!(page.body.contains("id=\"list\""), "the dashboard itself");
    assert!(!page.body.contains("<body data-auth>"));
    let fleet = http(srv.port, "GET", "/api/fleet", None, "");
    assert_eq!(fleet.status, 200);
    assert!(fleet.body.contains("Ana"));
}

#[test]
fn with_a_password_nothing_opens_without_a_session() {
    let srv = start(locked("correct horse"));
    let page = http(srv.port, "GET", "/", None, "");
    assert_eq!(page.status, 200);
    assert!(page.body.contains("id=\"pw\""), "the sign-in page");
    assert!(!page.body.contains("id=\"list\""));
    assert_eq!(http(srv.port, "GET", "/api/fleet", None, "").status, 401);
    assert_eq!(http(srv.port, "GET", "/api/fleet", Some("maestro_dash=forged"), "").status, 401);
    assert_eq!(http(srv.port, "POST", "/api/send", None, r#"{"paneId":"a","message":"rm -rf"}"#).status, 401);
    assert_eq!(http(srv.port, "POST", "/term/input", None, r#"{"id":"a","data":"eA=="}"#).status, 401);
    assert_eq!(http(srv.port, "POST", "/term/resize", None, r#"{"id":"a"}"#).status, 401);
    assert_eq!(http(srv.port, "GET", "/term?id=a", None, "").status, 401);
    assert!(srv.sent.lock().unwrap().is_empty(), "nothing reached an agent");
}

#[test]
fn a_wrong_password_is_refused_slowly() {
    let srv = start(locked("correct horse"));
    let t = Instant::now();
    let r = http(srv.port, "POST", "/api/login", None, r#"{"password":"wrong horse"}"#);
    assert_eq!(r.status, 401);
    assert!(t.elapsed() >= crate::auth::FAIL_DELAY);
    assert!(!r.head.to_ascii_lowercase().contains("set-cookie"));
    assert_eq!(http(srv.port, "POST", "/api/login", None, "not json").status, 401);
}

/// A burst of logins at once is not a way around the backoff: most are
/// told to wait (429) and only the free tries get checked.
#[test]
fn a_login_burst_gets_429s() {
    let auth = locked("correct horse");
    let srv = start(auth.clone());
    let port = srv.port;
    let gate = Arc::new(std::sync::Barrier::new(12));
    let codes: Vec<u16> = (0..12)
        .map(|_| {
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                http(port, "POST", "/api/login", None, r#"{"password":"nope"}"#).status
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    let checked = codes.iter().filter(|c| **c == 401).count();
    assert!(checked <= 3, "{checked} guesses checked: {codes:?}");
    assert_eq!(codes.iter().filter(|c| **c == 429).count(), 12 - checked, "{codes:?}");
    assert!(auth.peak_checks() <= 2);
    assert!(LOGIN_PAGE.contains("Too many attempts, try again in a moment."), "the page says so");
}

#[test]
fn signing_in_opens_everything_until_signing_out() {
    let srv = start(locked("correct horse"));
    let r = http(srv.port, "POST", "/api/login", None, r#"{"password":"correct horse"}"#);
    assert_eq!(r.status, 200);
    assert!(r.head.contains("HttpOnly; SameSite=Strict; Path=/; Max-Age=43200"));
    let cookie = cookie_from(&r);
    assert!(cookie.starts_with("maestro_dash=") && cookie.len() == "maestro_dash=".len() + 64);

    let page = http(srv.port, "GET", "/", Some(&cookie), "");
    assert!(page.body.contains("id=\"list\"") && page.body.contains("<body data-auth>"));
    let fleet = http(srv.port, "GET", "/api/fleet", Some(&format!("theme=dark; {cookie}")), "");
    assert_eq!(fleet.status, 200);
    assert!(fleet.body.contains("Ana"));
    assert_eq!(http(srv.port, "POST", "/api/send", Some(&cookie), r#"{"paneId":"a","message":"hi"}"#).status, 200);
    assert_eq!(srv.sent.lock().unwrap().len(), 1);

    let out = http(srv.port, "POST", "/api/logout", Some(&cookie), "");
    assert_eq!(out.status, 200);
    assert!(out.head.contains("Max-Age=0"));
    assert_eq!(http(srv.port, "GET", "/api/fleet", Some(&cookie), "").status, 401);
}

/// The same walk with the real curl (skipped where there is none).
#[test]
fn curl_sees_401_then_200_after_login() {
    let curl = |args: &[&str]| std::process::Command::new("curl").args(args).output();
    if curl(&["--version"]).is_err() {
        return;
    }
    let srv = start(locked("correct horse"));
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("jar.txt");
    let jar = jar.to_str().unwrap();
    let base = format!("http://127.0.0.1:{}", srv.port);
    let code = |out: std::io::Result<std::process::Output>| String::from_utf8(out.unwrap().stdout).unwrap();
    let fleet = format!("{base}/api/fleet");
    let mx = "X-Maestro: 1";
    assert_eq!(code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", "-H", mx, &fleet])), "401");
    let login = format!("{base}/api/login");
    let ok = code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", "-c", jar, "-H", "Content-Type: application/json",
        "-H", mx, "-d", r#"{"password":"correct horse"}"#, &login]));
    assert_eq!(ok, "200");
    assert_eq!(code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", "-b", jar, "-H", mx, &fleet])), "200");
    // a plain form post, the kind any page can send, is turned away
    let form = code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", "-d", "password=x", &login]));
    assert_eq!(form, "415");
}

/// Turning "let other devices in" on restarts the server on the same
/// port at once: the restart waits for the old one to let go of its ports
/// instead of failing with "only one usage of each socket address".
#[test]
fn a_restart_finds_the_old_servers_ports_free() {
    // Two free ports side by side, above the range Windows hands out to
    // other tests' sockets (which takes port + 1 of an OS-picked port).
    let free = |p: u16| TcpListener::bind(("127.0.0.1", p)).is_ok();
    let port = (0..400u16)
        .map(|k| 47_000 + (std::process::id() as u16 % 50) * 40 + k * 2)
        .find(|&p| free(p) && free(p + 1))
        .expect("two free ports");
    for round in 0..3 {
        let (server, ws) = bind_both(Ipv4Addr::LOCALHOST, port, Duration::ZERO).unwrap_or_else(|e| panic!("round {round}: {e:?}"));
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = Ctx {
            snapshot: Arc::new(Mutex::new("{}".into())),
            registry: Arc::new(Mutex::new(Registry::default())),
            auth: locked("correct horse"),
            send: Arc::new(|_| {}),
            guard: Guard { port, ws_port: port + 1, lan_ip: None, others: false },
            stop: stop.clone(),
        };
        let (ctx_ws, stop_http, stop_ws) = (ctx.clone(), stop.clone(), stop.clone());
        let threads = vec![
            std::thread::spawn(move || serve(server, ctx, stop_http)),
            std::thread::spawn(move || serve_ws(ws, stop_ws, ctx_ws)),
        ];
        assert_eq!(try_status(port), Some(401));
        stop.store(true, Ordering::SeqCst);
        finish(threads, port);
    }
}

/// Removing the password stops a server other devices can reach BEFORE
/// the password goes: never a moment of no password on 0.0.0.0.
#[test]
fn removing_the_password_stops_the_lan_server_first() {
    let auth = locked("correct horse");
    let srv = start(auth.clone());
    // the server as Settings left it: running, other devices let in
    let dash = Dashboard {
        inner: Mutex::new(Inner { running: true, port: srv.port, lan: true, stop: Some(srv.stop.clone()), ..Default::default() }),
        auth: auth.clone(),
    };
    assert_eq!(try_status(srv.port), Some(401));
    let info = dash.clear_password().unwrap();
    assert!(!info.running && !auth.has_password());
    // whatever still reaches the old server finds it closing, never open
    for _ in 0..5 {
        let got = try_status(srv.port);
        assert_ne!(got, Some(200), "the fleet was open with no password");
    }
}

/// DNS rebinding: a page on evil.com that re-points its name at 127.0.0.1
/// reaches the server with `Host: evil.com:port`. No password is the worst case.
#[test]
fn a_rebound_host_is_refused() {
    let srv = start(Arc::new(DashAuth::default()));
    let p = srv.port;
    let bad = [
        "evil.com".to_string(),
        format!("evil.com:{p}"),
        format!("localhost.evil.com:{p}"),
        format!("192.168.1.50:{p}"),
        "127.0.0.1:1".to_string(),
        format!("127.0.0.1:{p}.evil.com"),
    ];
    for host in &bad {
        let r = raw(p, "GET", "/api/fleet", &[("Host", host), ("X-Maestro", "1")], "");
        assert_eq!(r.status, 403, "Host {host}");
        assert!(!r.body.contains("Ana"), "Host {host} read the fleet");
        let r = raw(p, "GET", "/", &[("Host", host)], "");
        assert_eq!(r.status, 403, "page for Host {host}");
        let r = raw(
            p,
            "POST",
            "/api/send",
            &[("Host", host), ("X-Maestro", "1"), ("Content-Type", "application/json")],
            r#"{"paneId":"a","message":"rm -rf"}"#,
        );
        assert_eq!(r.status, 403, "send for Host {host}");
    }
    assert!(srv.sent.lock().unwrap().is_empty(), "nothing reached an agent");
    // the names this computer really uses still work
    for host in [format!("127.0.0.1:{p}"), format!("localhost:{p}"), format!("LocalHost:{p}")] {
        let r = raw(p, "GET", "/api/fleet", &[("Host", &host), ("X-Maestro", "1")], "");
        assert_eq!(r.status, 200, "Host {host}");
    }
}

#[test]
fn a_foreign_origin_is_refused() {
    let srv = start(Arc::new(DashAuth::default()));
    let p = srv.port;
    let foreign = [
        "http://evil.com".to_string(),
        "http://127.0.0.1:3000".to_string(),
        format!("https://127.0.0.1:{p}"),
        format!("http://evil.com:{p}"),
        "null".to_string(),
    ];
    for origin in &foreign {
        let h = [("Origin", origin.as_str()), ("X-Maestro", "1"), ("Content-Type", "application/json")];
        assert_eq!(raw(p, "POST", "/api/send", &h, r#"{"paneId":"a","message":"x"}"#).status, 403, "{origin}");
        assert_eq!(raw(p, "POST", "/term/input", &h, r#"{"id":"a","data":"eA=="}"#).status, 403, "{origin}");
        assert_eq!(raw(p, "GET", "/api/fleet", &h[..2], "").status, 403, "{origin}");
        assert_eq!(raw(p, "GET", "/term?id=a", &h[..2], "").status, 403, "{origin}");
    }
    assert!(srv.sent.lock().unwrap().is_empty(), "nothing reached an agent");
    for origin in [format!("http://127.0.0.1:{p}"), format!("http://localhost:{p}")] {
        let h = [("Origin", origin.as_str()), ("X-Maestro", "1"), ("Content-Type", "application/json")];
        assert_eq!(raw(p, "POST", "/api/send", &h, r#"{"paneId":"a","message":"hi"}"#).status, 200, "{origin}");
    }
    assert_eq!(srv.sent.lock().unwrap().len(), 2);
}

/// Without JSON and the X-Maestro header a request is not the page's own:
/// a browser can't send either cross-site without a preflight nobody answers.
#[test]
fn api_calls_need_json_and_the_maestro_header() {
    let srv = start(Arc::new(DashAuth::default()));
    let p = srv.port;
    let body = r#"{"paneId":"a","message":"x"}"#;
    for ct in ["text/plain", "application/x-www-form-urlencoded", "multipart/form-data; boundary=x"] {
        let r = raw(p, "POST", "/api/send", &[("X-Maestro", "1"), ("Content-Type", ct)], body);
        assert_eq!(r.status, 415, "{ct}");
    }
    assert_eq!(raw(p, "POST", "/api/send", &[("X-Maestro", "1")], body).status, 415, "no content type");
    assert_eq!(raw(p, "POST", "/api/send", &[("Content-Type", "application/json")], body).status, 403);
    assert_eq!(raw(p, "POST", "/term/input", &[("Content-Type", "application/json")], "{}").status, 403);
    assert_eq!(raw(p, "POST", "/term/resize", &[("Content-Type", "application/json")], "{}").status, 403);
    assert_eq!(raw(p, "POST", "/api/logout", &[("Content-Type", "application/json")], "").status, 403);
    assert_eq!(raw(p, "GET", "/api/fleet", &[], "").status, 403);
    assert_eq!(raw(p, "GET", "/term?id=a", &[], "").status, 403);
    assert!(srv.sent.lock().unwrap().is_empty(), "nothing reached an agent");
    // charset is fine; the page and its scripts load without the header
    let h = [("X-Maestro", "1"), ("Content-Type", "application/json; charset=utf-8")];
    assert_eq!(raw(p, "POST", "/api/send", &h, body).status, 200);
    assert_eq!(raw(p, "GET", "/", &[], "").status, 200);
    assert_eq!(raw(p, "GET", "/xterm.js", &[], "").status, 200);
}

/// Another site can't burn this address's tries with no-cors text/plain
/// POSTs to /api/login, locking the owner out or timing guesses.
#[test]
fn a_cross_site_login_cannot_lock_the_owner_out() {
    let srv = start(locked("correct horse"));
    let p = srv.port;
    for _ in 0..6 {
        let h = [("Origin", "http://evil.com"), ("Content-Type", "text/plain;charset=UTF-8")];
        assert_eq!(raw(p, "POST", "/api/login", &h, r#"{"password":"guess"}"#).status, 403);
        let h = [("Content-Type", "text/plain;charset=UTF-8")];
        assert_ne!(raw(p, "POST", "/api/login", &h, r#"{"password":"guess"}"#).status, 401);
    }
    let r = http(p, "POST", "/api/login", None, r#"{"password":"correct horse"}"#);
    assert_eq!(r.status, 200, "the owner signs in at the first try");
}
