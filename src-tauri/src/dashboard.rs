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
//!
//! Only the dashboard's own page gets in (`Guard`): the Host must be one of
//! this server's names (no DNS rebinding), an Origin must be the page's own
//! (no other site or other port on this computer), a POST must be JSON and an
//! API call must carry `X-Maestro: 1`, neither of which another site can send
//! without a CORS preflight that nothing here answers.

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
    /// Which hosts and origins are this server's own.
    guard: Guard,
    /// Set when the server is stopping: nothing is served after it.
    stop: Arc<AtomicBool>,
}

/// The names this server answers to. Anything else is another site: a DNS
/// rebinding page (Host: evil.com) or a page on another port of this computer
/// (Origin: http://127.0.0.1:3000) that the browser would happily send our
/// cookie from.
#[derive(Clone)]
struct Guard {
    /// The page's port: Host of a page request, and the port in every Origin.
    port: u16,
    /// The terminal WebSocket's port (port + 1): Host of a socket handshake.
    ws_port: u16,
    /// The LAN address shown in Settings, when other devices are let in.
    lan_ip: Option<Ipv4Addr>,
}

impl Guard {
    fn names(&self) -> Vec<String> {
        let mut out = vec!["127.0.0.1".to_string(), "localhost".to_string()];
        if let Some(ip) = self.lan_ip {
            out.push(ip.to_string());
        }
        out
    }

    /// Is `host` (a Host header) one of ours on `port`?
    fn host_ok(&self, host: Option<&str>, port: u16) -> bool {
        let Some(host) = host.map(|h| h.trim().to_ascii_lowercase()) else { return false };
        self.names().iter().any(|n| host == format!("{n}:{port}") || (port == 80 && host == *n))
    }

    /// No Origin (not a browser, or a same-origin GET), or the page's own.
    fn origin_ok(&self, origin: Option<&str>) -> bool {
        let Some(origin) = origin.map(|o| o.trim().to_ascii_lowercase()) else { return true };
        let Some(host) = origin.strip_prefix("http://") else { return false };
        self.host_ok(Some(host), self.port)
    }

    /// Why an HTTP request is turned away before any route sees it.
    fn refuse(&self, method: &Method, path: &str, get: impl Fn(&str) -> Option<String>) -> Option<(u16, &'static str)> {
        if !self.host_ok(get("Host").as_deref(), self.port) {
            return Some((403, "unknown host"));
        }
        if !self.origin_ok(get("Origin").as_deref()) {
            return Some((403, "foreign origin"));
        }
        // JSON only: a cross-site form or no-cors fetch can't send it without
        // a preflight, and nothing here answers one.
        if *method == Method::Post {
            let ct = get("Content-Type").unwrap_or_default();
            let mime = ct.split(';').next().unwrap_or("").trim();
            if !mime.eq_ignore_ascii_case("application/json") {
                return Some((415, "send JSON"));
            }
        }
        // The page's own calls carry X-Maestro; another site's can't without a preflight.
        let api = path.starts_with("/api/") || path == "/term" || path.starts_with("/term/");
        if api && get("X-Maestro").as_deref().map(str::trim) != Some("1") {
            return Some((403, "not from the dashboard"));
        }
        None
    }
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

/// The URLs to show, and the LAN address (when let in) the server answers to.
fn urls_for(port: u16, lan: bool) -> (Vec<String>, Option<Ipv4Addr>) {
    let mut out = vec![format!("http://127.0.0.1:{port}")];
    let ip = if lan { local_ip() } else { None };
    if let Some(ip) = ip {
        out.push(format!("http://{ip}:{port}"));
    }
    (out, ip)
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
    let (server, ws_listener, (urls, lan_ip)) = run_blocking(move || {
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
        guard: Guard { port, ws_port: port + 1, lan_ip },
        stop: stop.clone(),
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

/// The value of the request's `name` header, if any.
fn header_of(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

/// The request's `Cookie:` header, if any.
fn cookie_of(request: &Request) -> Option<String> {
    header_of(request, "Cookie")
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
        Login::Busy => {
            let body = "{\"error\":\"busy\",\"message\":\"Too many attempts, try again in a moment\"}";
            let _ = request.respond(json_response(body.into()).with_status_code(429));
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
    if let Some((code, why)) = ctx.guard.refuse(&method, &path, |name| header_of(&request, name)) {
        let body = format!("{{\"error\":\"{why}\"}}");
        let _ = request.respond(json_response(body).with_status_code(code));
        return;
    }
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
                let ctx = ctx.clone();
                std::thread::spawn(move || ws_conn(stream, ctx));
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

fn ws_conn(stream: TcpStream, ctx: Ctx) {
    let Ctx { registry, auth, stop, guard, .. } = ctx;
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
            let refuse = |code: tungstenite::http::StatusCode, why: &str| {
                let mut no = tungstenite::handshake::server::ErrorResponse::new(Some(why.into()));
                *no.status_mut() = code;
                Err(no)
            };
            let get = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
            // Same rules as a page request: our own host (this port), and an
            // Origin that is the dashboard page itself (a browser always sends one).
            if !guard.host_ok(get("Host"), guard.ws_port) {
                return refuse(tungstenite::http::StatusCode::FORBIDDEN, "unknown host");
            }
            if !guard.origin_ok(get("Origin")) {
                return refuse(tungstenite::http::StatusCode::FORBIDDEN, "foreign origin");
            }
            // The browser sends the page's cookie along (cookies ignore the port).
            if !auth.allows(get("Cookie"), Instant::now()) {
                return refuse(tungstenite::http::StatusCode::UNAUTHORIZED, "sign in");
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
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = Ctx {
            snapshot: Arc::new(Mutex::new("{\"agents\":[{\"id\":\"a\",\"name\":\"Ana\"}]}".into())),
            registry: Arc::new(Mutex::new(Registry::default())),
            auth,
            send: Arc::new(move |b| sink.lock().unwrap().push(b)),
            guard: Guard { port, ws_port, lan_ip: None },
            stop: stop.clone(),
        };
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

    /// One plain HTTP/1.1 request with exactly these headers (plus
    /// `Host: 127.0.0.1:{port}` unless one is given).
    fn raw(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Reply {
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
    /// X-Maestro header, JSON on a POST.
    fn http(port: u16, method: &str, path: &str, cookie: Option<&str>, body: &str) -> Reply {
        let mut h = vec![("X-Maestro", "1")];
        if method == "POST" {
            h.push(("Content-Type", "application/json"));
        }
        if let Some(c) = cookie {
            h.push(("Cookie", c));
        }
        raw(port, method, path, &h, body)
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

    #[test]
    fn the_guard_knows_its_own_names() {
        let g = Guard { port: 8477, ws_port: 8478, lan_ip: Some(Ipv4Addr::new(192, 168, 1, 7)) };
        assert!(g.host_ok(Some("192.168.1.7:8477"), 8477));
        assert!(g.host_ok(Some("192.168.1.7:8478"), 8478));
        assert!(!g.host_ok(Some("192.168.1.7:8478"), 8477));
        assert!(!g.host_ok(Some("192.168.1.8:8477"), 8477));
        assert!(!g.host_ok(None, 8477));
        assert!(g.origin_ok(None));
        assert!(g.origin_ok(Some("http://192.168.1.7:8477")));
        assert!(!g.origin_ok(Some("http://192.168.1.7:8478")), "the socket port is not a page");
        assert!(!g.origin_ok(Some("http://192.168.1.7:8477/x")));
        let local = Guard { lan_ip: None, ..g };
        assert!(!local.host_ok(Some("192.168.1.7:8477"), 8477), "the LAN address only when let in");
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
}
