//! Local web dashboard — a tiny read-only HTTP view of the agent fleet that the
//! user can open from another device on the same network, plus a "send a
//! message to an agent" POST that hands off to the frontend.
//!
//! The fleet state lives in the JS frontend, so the frontend pushes a JSON
//! snapshot here (`dashboard_push`) on each fleet tick; the HTTP server serves
//! that snapshot. A POST /api/send emits a `dashboard-send` event the frontend
//! turns into a keystroke to the target pane. Bound to localhost by default;
//! LAN (0.0.0.0) is opt-in since the send endpoint can drive an agent, and
//! needs a password (dashauth.rs): with one set, every route that shows or
//! drives an agent wants a signed-in session.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::core::registry::Registry;
use crate::dashauth::{self, DashAuth, Login};
use crate::error::{run_blocking, CommandError};
use crate::state::AppState;

#[derive(Default)]
pub struct Dashboard {
    inner: Mutex<Inner>,
    /// The password and who is signed in; outlives any one server.
    pub auth: Arc<DashAuth>,
}

/// What every request handler needs.
#[derive(Clone)]
struct Ctx {
    snapshot: Arc<Mutex<String>>,
    registry: Arc<Mutex<Registry>>,
    auth: Arc<DashAuth>,
    /// Hands a POST /api/send body to the frontend.
    send: Arc<dyn Fn(String) + Send + Sync>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    port: u16,
    lan: bool,
    urls: Vec<String>, // computed once at start (off the main thread), then cached
    snapshot: Arc<Mutex<String>>,
    stop: Option<Arc<AtomicBool>>,
}

#[derive(Serialize, Clone)]
pub struct DashboardInfo {
    pub running: bool,
    pub port: u16,
    pub lan: bool,
    pub urls: Vec<String>,
}

const PAGE: &str = include_str!("dashboard.html");
const LOGIN_PAGE: &str = include_str!("dashboard-login.html");
// xterm.js bundled + served same-origin so the full web terminal works offline.
const XTERM_JS: &str = include_str!("../../node_modules/@xterm/xterm/lib/xterm.js");
const XTERM_CSS: &str = include_str!("../../node_modules/@xterm/xterm/css/xterm.css");
const ADDON_FIT_JS: &str = include_str!("../../node_modules/@xterm/addon-fit/lib/addon-fit.js");

/// Best-effort primary LAN IPv4 (no packet is actually sent — the connect just
/// picks the outbound interface). None if it can't be determined.
fn local_ip() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

fn urls_for(port: u16, lan: bool) -> Vec<String> {
    let mut out = vec![format!("http://127.0.0.1:{port}")];
    if lan {
        if let Some(ip) = local_ip() {
            out.push(format!("http://{ip}:{port}"));
        }
    }
    out
}

fn info(inner: &Inner) -> DashboardInfo {
    DashboardInfo {
        running: inner.running,
        port: inner.port,
        lan: inner.lan,
        // Cached — never call local_ip() here (this runs on the UI thread).
        urls: if inner.running { inner.urls.clone() } else { vec![] },
    }
}

#[tauri::command]
pub fn dashboard_status(state: State<'_, AppState>) -> DashboardInfo {
    info(&state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner()))
}

/// Store the latest fleet snapshot JSON the HTTP server serves at /api/fleet.
#[tauri::command]
pub fn dashboard_push(state: State<'_, AppState>, snapshot: String) {
    let inner = state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner());
    *inner.snapshot.lock().unwrap_or_else(|p| p.into_inner()) = snapshot;
}

#[tauri::command]
pub fn dashboard_stop(state: State<'_, AppState>) -> DashboardInfo {
    stop_server(&state.dashboard)
}

fn stop_server(dash: &Dashboard) -> DashboardInfo {
    let mut inner = dash.inner.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(stop) = inner.stop.take() {
        stop.store(true, Ordering::SeqCst);
    }
    inner.running = false;
    info(&inner)
}

/// Is a dashboard password set?
#[tauri::command]
pub fn dashboard_password_status(state: State<'_, AppState>) -> bool {
    state.dashboard.auth.has_password()
}

/// Set or change the dashboard password (everyone signed in signs in again).
#[tauri::command]
pub async fn dashboard_password_set(state: State<'_, AppState>, password: String) -> Result<bool, CommandError> {
    let auth = state.dashboard.auth.clone();
    // hashing takes a moment on purpose: keep it off the main thread
    run_blocking(move || auth.set_password(&password).map_err(CommandError::Failed)).await?;
    Ok(true)
}

/// Remove the password. A server other devices can reach stops with it: without
/// a password the dashboard is for this computer only.
#[tauri::command]
pub fn dashboard_password_clear(state: State<'_, AppState>) -> Result<DashboardInfo, CommandError> {
    state.dashboard.auth.clear_password().map_err(CommandError::Failed)?;
    let lan_on = {
        let inner = state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.running && inner.lan
    };
    if lan_on {
        return Ok(stop_server(&state.dashboard));
    }
    Ok(info(&state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner())))
}

#[tauri::command]
pub async fn dashboard_start(
    app: AppHandle,
    state: State<'_, AppState>,
    port: u16,
    lan: bool,
) -> Result<DashboardInfo, CommandError> {
    // Other devices can type into agents: never without a password.
    let auth = state.dashboard.auth.clone();
    let bind = dashauth::bind_addr(lan, auth.has_password()).map_err(CommandError::Failed)?;
    // Grab what the server thread needs WITHOUT holding a lock across an await.
    let snapshot = state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner()).snapshot.clone();
    let registry = state.registry.clone();
    // Stop any previous server before rebinding.
    if let Some(stop) = state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner()).stop.take() {
        stop.store(true, Ordering::SeqCst);
    }

    // Binding to 0.0.0.0 (LAN) and probing the local IP can block; do it OFF the
    // main/UI thread so toggling LAN never freezes the app. The full web
    // terminal uses a WebSocket on port+1 (own the TcpStream → set TCP_NODELAY).
    let (server, ws_listener, urls) = run_blocking(move || {
        let server = Server::http((bind, port)).map_err(|e| {
            CommandError::Failed(format!("dashboard: cannot bind port {port}: {e}"))
        })?;
        let ws_listener = TcpListener::bind((bind, port + 1)).map_err(|e| {
            CommandError::Failed(format!("dashboard: cannot bind ws port {}: {e}", port + 1))
        })?;
        Ok((server, ws_listener, urls_for(port, lan)))
    })
    .await?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let send_app = app.clone();
    let ctx = Ctx {
        snapshot,
        registry,
        auth,
        send: Arc::new(move |body| {
            let _ = send_app.emit("dashboard-send", body);
        }),
    };
    let ctx_ws = ctx.clone();
    std::thread::spawn(move || serve(server, ctx, stop_thread));
    let stop_ws = stop.clone();
    std::thread::spawn(move || serve_ws(ws_listener, stop_ws, ctx_ws));

    let mut inner = state.dashboard.inner.lock().unwrap_or_else(|p| p.into_inner());
    inner.running = true;
    inner.port = port;
    inner.lan = lan;
    inner.urls = urls;
    inner.stop = Some(stop);
    Ok(info(&inner))
}

fn json_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    Response::from_string(body).with_header(header)
}

fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).unwrap()
}

fn respond_404(request: Request) {
    let _ = request.respond(Response::from_string("not found").with_status_code(404));
}

/// Serve a bundled static asset (xterm.js / css) with a content type.
fn asset(request: Request, body: &'static str, mime: &[u8]) {
    let _ = request.respond(Response::from_string(body).with_header(header(b"Content-Type", mime)));
}

/// Extract `id` from a `?id=...&...` query string.
fn query_id(url: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    q.split('&').find_map(|p| p.strip_prefix("id=").map(|v| v.to_string()))
}

fn read_body(request: &mut Request) -> serde_json::Value {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    serde_json::from_str(&body).unwrap_or(serde_json::Value::Null)
}

/// The request's `Cookie:` header, if any.
fn cookie_of(request: &Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Cookie"))
        .map(|h| h.value.as_str().to_string())
}

fn respond_401(request: Request) {
    let _ = request.respond(json_response("{\"error\":\"sign in\"}".into()).with_status_code(401));
}

fn html(request: Request, body: String) {
    let ct = header(b"Content-Type", b"text/html; charset=utf-8");
    // No-cache so a rebuilt page always loads fresh on refresh.
    let nc = header(b"Cache-Control", b"no-cache, no-store");
    let _ = request.respond(Response::from_string(body).with_header(ct).with_header(nc));
}

/// POST /api/login: `{"password": "..."}`. A good one sets the session cookie;
/// a wrong one is held back half a second, and a guessing address waits longer.
fn login(mut request: Request, auth: &DashAuth) {
    let ip = request.remote_addr().map(|a| a.ip()).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    // a password is short: don't read a large body
    let mut body = String::new();
    let _ = request.as_reader().take(4096).read_to_string(&mut body);
    let password = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("password").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_default();
    match auth.login(ip, &password, Instant::now()) {
        Login::Ok(token) => {
            let cookie = header(b"Set-Cookie", dashauth::session_cookie(&token).as_bytes());
            let _ = request.respond(json_response("{\"ok\":true}".into()).with_header(cookie));
        }
        Login::Wrong => {
            std::thread::sleep(dashauth::FAIL_DELAY);
            let _ = request.respond(json_response("{\"error\":\"wrong\"}".into()).with_status_code(401));
        }
        Login::Wait(d) => {
            std::thread::sleep(dashauth::FAIL_DELAY);
            let body = format!("{{\"error\":\"wait\",\"seconds\":{}}}", d.as_secs().max(1));
            let _ = request.respond(json_response(body).with_status_code(429));
        }
    }
}

fn serve(server: Server, ctx: Ctx, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        // Timeout so the stop flag is checked even with no traffic.
        let request = match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => break,
        };
        // One thread per request: a /term SSE stream holds its connection open
        // for the life of the terminal, so it must not block the accept loop.
        let ctx = ctx.clone();
        std::thread::spawn(move || handle(request, ctx));
    }
}

fn handle(mut request: Request, ctx: Ctx) {
    let url = request.url().to_string();
    let method = request.method().clone();
    let path = url.split('?').next().unwrap_or("").to_string();
    let signed_in = ctx.auth.allows(cookie_of(&request).as_deref(), Instant::now());

    match (method, path.as_str()) {
        // Without a session the page is the sign-in form.
        (Method::Get, "/") if !signed_in => html(request, LOGIN_PAGE.to_string()),
        (Method::Get, "/") => {
            // a page behind a password offers Sign out
            let page = if ctx.auth.has_password() { PAGE.replacen("<body>", "<body data-auth>", 1) } else { PAGE.to_string() };
            html(request, page)
        }
        (Method::Post, "/api/login") => login(request, &ctx.auth),
        (Method::Post, "/api/logout") => {
            if let Some(token) = cookie_of(&request).as_deref().and_then(dashauth::token_in) {
                ctx.auth.logout(token);
            }
            let cookie = header(b"Set-Cookie", dashauth::cleared_cookie().as_bytes());
            let _ = request.respond(json_response("{\"ok\":true}".into()).with_header(cookie));
        }
        (Method::Get, "/xterm.js") => asset(request, XTERM_JS, b"application/javascript"),
        (Method::Get, "/addon-fit.js") => asset(request, ADDON_FIT_JS, b"application/javascript"),
        (Method::Get, "/xterm.css") => asset(request, XTERM_CSS, b"text/css"),
        // Everything past here shows or drives an agent.
        _ if !signed_in => respond_401(request),
        (Method::Get, "/api/fleet") => {
            let body = ctx.snapshot.lock().unwrap_or_else(|p| p.into_inner()).clone();
            let body = if body.is_empty() { "{\"agents\":[]}".to_string() } else { body };
            let _ = request.respond(json_response(body));
        }
        (Method::Post, "/api/send") => {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            // Forward the raw JSON to the frontend, which validates + delivers.
            (ctx.send)(body);
            let _ = request.respond(json_response("{\"ok\":true}".into()));
        }
        // Full web terminal: SSE output stream + raw input + resize.
        (Method::Get, "/term") => sse_terminal(request, ctx.registry, query_id(&url)),
        (Method::Post, "/term/input") => {
            let v = read_body(&mut request);
            if let (Some(id), Some(data)) = (
                v.get("id").and_then(|x| x.as_str()),
                v.get("data").and_then(|x| x.as_str()),
            ) {
                if let Ok(bytes) = STANDARD.decode(data) {
                    let _ = ctx.registry.lock().unwrap_or_else(|p| p.into_inner()).write_input(id, &bytes);
                }
            }
            let _ = request.respond(json_response("{\"ok\":true}".into()));
        }
        (Method::Post, "/term/resize") => {
            let v = read_body(&mut request);
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                let cols = v.get("cols").and_then(|x| x.as_u64()).unwrap_or(80) as u16;
                let rows = v.get("rows").and_then(|x| x.as_u64()).unwrap_or(24) as u16;
                let _ = ctx.registry.lock().unwrap_or_else(|p| p.into_inner()).resize(id, cols, rows);
            }
            let _ = request.respond(json_response("{\"ok\":true}".into()));
        }
        _ => respond_404(request),
    }
}

/// Stream an agent's live output to the browser as Server-Sent Events. Each PTY
/// chunk is base64-encoded into one `data:` frame; xterm.js decodes and writes
/// it. Blocks in its own thread until the agent exits or the client disconnects.
fn sse_terminal(request: Request, registry: Arc<Mutex<Registry>>, id: Option<String>) {
    let id = match id {
        Some(i) => i,
        None => return respond_404(request),
    };
    let rx = match registry.lock().unwrap_or_else(|p| p.into_inner()).tap(&id) {
        Ok(r) => r,
        Err(_) => return respond_404(request),
    };
    let headers = vec![
        header(b"Content-Type", b"text/event-stream"),
        header(b"Cache-Control", b"no-cache"),
    ];
    let reader = SseReader { rx, pending: Vec::new(), pos: 0 };
    // data_length None → chunked transfer, streamed as the reader yields.
    let resp = Response::new(StatusCode(200), headers, reader, None, None);
    let _ = request.respond(resp);
}

/// WebSocket terminal server (port+1). A real bidirectional low-latency channel
/// for the full web terminal: TCP_NODELAY on, ordered by nature, no per-key HTTP
/// round-trips — so typing feels near-direct.
fn serve_ws(listener: TcpListener, stop: Arc<AtomicBool>, ctx: Ctx) {
    // Non-blocking accept so the stop flag is honoured with no traffic.
    let _ = listener.set_nonblocking(true);
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let (registry, auth) = (ctx.registry.clone(), ctx.auth.clone());
                let stop = stop.clone();
                std::thread::spawn(move || ws_conn(stream, registry, auth, stop));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}

fn ws_query_id(uri: &str) -> String {
    uri.split_once('?')
        .and_then(|(_, q)| q.split('&').find_map(|p| p.strip_prefix("id=")))
        .unwrap_or("")
        .to_string()
}

fn ws_conn(stream: TcpStream, registry: Arc<Mutex<Registry>>, auth: Arc<DashAuth>, stop: Arc<AtomicBool>) {
    let _ = stream.set_nodelay(true); // the whole point — no Nagle coalescing
    // A socket accepted from the non-blocking listener is non-blocking too on
    // Windows; the handshake needs to wait for the request.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut agent_id = String::new();
    let ws = tungstenite::accept_hdr(
        stream,
        |req: &tungstenite::handshake::server::Request,
         resp: tungstenite::handshake::server::Response| {
            // The browser sends the page's cookie along (cookies ignore the port).
            let cookie = req.headers().get("Cookie").and_then(|v| v.to_str().ok());
            if !auth.allows(cookie, Instant::now()) {
                let mut no = tungstenite::handshake::server::ErrorResponse::new(Some("sign in".into()));
                *no.status_mut() = tungstenite::http::StatusCode::UNAUTHORIZED;
                return Err(no);
            }
            agent_id = ws_query_id(&req.uri().to_string());
            Ok(resp)
        },
    );
    let mut ws = match ws {
        Ok(w) => w,
        Err(_) => return,
    };
    if agent_id.is_empty() {
        return;
    }
    let rx = match registry.lock().unwrap_or_else(|p| p.into_inner()).tap(&agent_id) {
        Ok(r) => r,
        Err(_) => return,
    };
    let _ = ws.get_mut().set_nonblocking(true);
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        // Output: forward every available PTY chunk to the browser.
        loop {
            match rx.try_recv() {
                Ok(chunk) => {
                    if ws.write(tungstenite::Message::Binary(chunk)).is_err() {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return, // agent exited
            }
        }
        let _ = ws.flush();
        // Input: keystrokes from the browser → PTY (in order, no round-trips).
        match ws.read() {
            Ok(tungstenite::Message::Binary(d)) => {
                let _ = registry.lock().unwrap_or_else(|p| p.into_inner()).write_input(&agent_id, &d);
            }
            Ok(tungstenite::Message::Text(t)) => {
                let _ = registry.lock().unwrap_or_else(|p| p.into_inner()).write_input(&agent_id, t.as_bytes());
            }
            Ok(tungstenite::Message::Close(_)) => return,
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return,
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A blocking `Read` that turns an agent's output channel into an SSE byte
/// stream. Reads block on the channel; a closed channel (agent gone) ends it.
struct SseReader {
    rx: Receiver<Vec<u8>>,
    pending: Vec<u8>,
    pos: usize,
}
impl Read for SseReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.pending.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    // tiny_http wraps the socket in a 1024-byte BufWriter and only
                    // flushes when it fills or the response ends — but an SSE stream
                    // never ends, so small frames (a keystroke echo) stall in the
                    // buffer. Pad each frame past 1 KB with an ignored `:` comment
                    // line so every frame overflows the buffer and flushes at once.
                    let mut frame = format!("data: {}\n", STANDARD.encode(&chunk));
                    const MIN: usize = 1500;
                    let l = frame.len();
                    if l + 3 < MIN {
                        frame.push(':');
                        frame.push_str(&" ".repeat(MIN - l - 3));
                        frame.push('\n');
                    }
                    frame.push('\n');
                    self.pending = frame.into_bytes();
                    self.pos = 0;
                }
                Err(_) => return Ok(0), // agent exited → EOF closes the stream
            }
        }
        let n = std::cmp::min(out.len(), self.pending.len() - self.pos);
        out[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tungstenite::client::IntoClientRequest;

    /// A real dashboard server on a free localhost port, stopped on drop.
    struct TestServer {
        port: u16,
        ws_port: u16,
        sent: Arc<Mutex<Vec<String>>>,
        stop: Arc<AtomicBool>,
    }
    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
        }
    }

    fn start(auth: Arc<DashAuth>) -> TestServer {
        let server = Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let ws_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let ws_port = ws_listener.local_addr().unwrap().port();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let sink = sent.clone();
        let ctx = Ctx {
            snapshot: Arc::new(Mutex::new("{\"agents\":[{\"id\":\"a\",\"name\":\"Ana\"}]}".into())),
            registry: Arc::new(Mutex::new(Registry::default())),
            auth,
            send: Arc::new(move |b| sink.lock().unwrap().push(b)),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (ctx_ws, stop_http, stop_ws) = (ctx.clone(), stop.clone(), stop.clone());
        std::thread::spawn(move || serve(server, ctx, stop_http));
        std::thread::spawn(move || serve_ws(ws_listener, stop_ws, ctx_ws));
        TestServer { port, ws_port, sent, stop }
    }

    struct Reply {
        status: u16,
        head: String,
        body: String,
    }

    /// One plain HTTP/1.1 request, the way curl would send it.
    fn http(port: u16, method: &str, path: &str, cookie: Option<&str>, body: &str) -> Reply {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        if let Some(c) = cookie {
            req.push_str(&format!("Cookie: {c}\r\n"));
        }
        if !body.is_empty() {
            req.push_str("Content-Type: application/json\r\n");
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

    /// The session cookie (name=value) a reply sets.
    fn cookie_from(r: &Reply) -> String {
        let line = r.head.lines().find(|l| l.to_ascii_lowercase().starts_with("set-cookie:")).expect("a cookie");
        line.split_once(':').unwrap().1.trim().split(';').next().unwrap().to_string()
    }

    fn locked(pw: &str) -> Arc<DashAuth> {
        let auth = Arc::new(DashAuth::default());
        auth.set_password(pw).unwrap();
        auth
    }

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
        assert!(t.elapsed() >= dashauth::FAIL_DELAY);
        assert!(!r.head.to_ascii_lowercase().contains("set-cookie"));
        assert_eq!(http(srv.port, "POST", "/api/login", None, "not json").status, 401);
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
        assert_eq!(code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", &fleet])), "401");
        let login = format!("{base}/api/login");
        let ok = code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", "-c", jar, "-H", "Content-Type: application/json",
            "-d", r#"{"password":"correct horse"}"#, &login]));
        assert_eq!(ok, "200");
        assert_eq!(code(curl(&["-s", "-o", "nul", "-w", "%{http_code}", "-b", jar, &fleet])), "200");
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
}
