//! A real dashboard server on a free localhost port, and plain HTTP
//! requests to it, for the server tests.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_core::registry::Registry;
use tiny_http::Server;

use crate::auth::{DashAuth, Login};
use crate::guard::Guard;
use crate::http::{serve, Ctx};
use crate::stream::serve_ws;
use crate::terms::{SharedTerminals, Terminals};

/// The tests' terminals: a registry of real agents.
impl Terminals for Mutex<Registry> {
    fn tap(&self, id: &str) -> Option<std::sync::mpsc::Receiver<Vec<u8>>> {
        agent_core::registry::lock(self).tap(id).ok()
    }
    fn write(&self, id: &str, bytes: &[u8]) {
        let _ = agent_core::registry::lock(self).write_keys(id, bytes);
    }
    fn resize(&self, id: &str, cols: u16, rows: u16) {
        let _ = agent_core::registry::lock(self).resize(id, cols, rows);
    }
}

/// Terminals with no agent in them.
pub(crate) fn no_agents() -> SharedTerminals {
    Arc::new(Mutex::new(Registry::default()))
}

/// A real dashboard server on a free localhost port, stopped on drop.
pub(crate) struct TestServer {
    pub(crate) port: u16,
    pub(crate) ws_port: u16,
    pub(crate) sent: Arc<Mutex<Vec<String>>>,
    pub(crate) stop: Arc<AtomicBool>,
}
impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

pub(crate) fn start(auth: Arc<DashAuth>) -> TestServer {
    start_with(auth, Arc::new(Mutex::new(Registry::default())))
}

/// A registry with one real agent "a" that prints a line every second.
pub(crate) fn one_agent() -> Arc<Mutex<Registry>> {
    let spec = agent_core::command_spec::CommandSpec::new("cmd.exe")
        .arg("/C")
        .arg("echo hi & ping -n 30 127.0.0.1");
    let size = portable_pty::PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };
    let mut reg = Registry::default();
    reg.spawn("a".into(), &spec, size, |_| {}, |_| {}).expect("spawn agent");
    Arc::new(Mutex::new(reg))
}

pub(crate) fn start_with(auth: Arc<DashAuth>, registry: Arc<Mutex<Registry>>) -> TestServer {
    let server = Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let ws_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let ws_port = ws_listener.local_addr().unwrap().port();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let sink = sent.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let ctx = Ctx {
        snapshot: Arc::new(Mutex::new("{\"agents\":[{\"id\":\"a\",\"name\":\"Ana\"}]}".into())),
        terms: registry,
        auth,
        send: Arc::new(move |b| sink.lock().unwrap().push(b)),
        guard: Guard { port, ws_port, lan_ip: None, others: false },
        stop: stop.clone(),
    };
    let (ctx_ws, stop_http, stop_ws) = (ctx.clone(), stop.clone(), stop.clone());
    std::thread::spawn(move || serve(server, ctx, stop_http));
    std::thread::spawn(move || serve_ws(ws_listener, stop_ws, ctx_ws));
    TestServer { port, ws_port, sent, stop }
}

pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) head: String,
    pub(crate) body: String,
}

/// One plain HTTP/1.1 request with exactly these headers (plus
/// `Host: 127.0.0.1:{port}` unless one is given).
pub(crate) fn raw(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Reply {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("Host")) {
        req.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or((out.as_str(), ""));
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    Reply { status, head: head.to_string(), body: body.to_string() }
}

/// A request the way the dashboard page sends it: its own host, the
/// X-ByteHive header, JSON on a POST.
pub(crate) fn http(port: u16, method: &str, path: &str, cookie: Option<&str>, body: &str) -> Reply {
    let mut h = vec![("X-ByteHive", "1")];
    if method == "POST" {
        h.push(("Content-Type", "application/json"));
    }
    if let Some(c) = cookie {
        h.push(("Cookie", c));
    }
    raw(port, method, path, &h, body)
}

/// The session cookie (name=value) a reply sets.
pub(crate) fn cookie_from(r: &Reply) -> String {
    let line = r.head.lines().find(|l| l.to_ascii_lowercase().starts_with("set-cookie:")).expect("a cookie");
    line.split_once(':').unwrap().1.trim().split(';').next().unwrap().to_string()
}

pub(crate) fn locked(pw: &str) -> Arc<DashAuth> {
    let auth = Arc::new(DashAuth::default());
    auth.set_password(pw).unwrap();
    auth
}

pub(crate) fn session(auth: &DashAuth) -> String {
    let Login::Ok(token) = auth.login(IpAddr::V4(Ipv4Addr::LOCALHOST), "correct horse", Instant::now()) else {
        panic!("sign in")
    };
    token
}

/// The status of one request, or None when nothing answered (closed).
pub(crate) fn try_status(port: u16) -> Option<u16> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let req = format!("GET /api/fleet HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nX-ByteHive: 1\r\n\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    out.split(' ').nth(1)?.parse().ok()
}
