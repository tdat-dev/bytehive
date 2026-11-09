//! The page and its API: every request passes the [`Guard`](crate::guard)
//! first, then (with a password set) wants a signed-in session for anything
//! that shows or drives an agent.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::assets::{ADDON_FIT_JS, LOGIN_PAGE, PAGE, XTERM_CSS, XTERM_JS};
use crate::auth::{self, DashAuth, Login};
use crate::guard::Guard;
use crate::stream::{sse_terminal, Watch};
use crate::terms::SharedTerminals;

/// What every request handler needs.
#[derive(Clone)]
pub(crate) struct Ctx {
    pub(crate) snapshot: Arc<Mutex<String>>,
    pub(crate) terms: SharedTerminals,
    pub(crate) auth: Arc<DashAuth>,
    /// Hands a POST /api/send body to the host app.
    pub(crate) send: Arc<dyn Fn(String) + Send + Sync>,
    /// Which hosts and origins are this server's own.
    pub(crate) guard: Guard,
    /// Set when the server is stopping: nothing is served after it.
    pub(crate) stop: Arc<AtomicBool>,
}

fn json_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    Response::from_string(body).with_header(header)
}

pub(crate) fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).unwrap()
}

pub(crate) fn respond_404(request: Request) {
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
            let cookie = header(b"Set-Cookie", auth::session_cookie(&token).as_bytes());
            let _ = request.respond(json_response("{\"ok\":true}".into()).with_header(cookie));
        }
        Login::Wrong => {
            std::thread::sleep(auth::FAIL_DELAY);
            let _ = request.respond(json_response("{\"error\":\"wrong\"}".into()).with_status_code(401));
        }
        Login::Busy => {
            let body = "{\"error\":\"busy\",\"message\":\"Too many attempts, try again in a moment\"}";
            let _ = request.respond(json_response(body.into()).with_status_code(429));
        }
        Login::Wait(d) => {
            std::thread::sleep(auth::FAIL_DELAY);
            let body = format!("{{\"error\":\"wait\",\"seconds\":{}}}", d.as_secs().max(1));
            let _ = request.respond(json_response(body).with_status_code(429));
        }
    }
}

pub(crate) fn serve(server: Server, ctx: Ctx, stop: Arc<AtomicBool>) {
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
    let pass = ctx.auth.pass(cookie_of(&request).as_deref(), Instant::now());
    let signed_in = pass.is_some();
    // A stopping server serves nothing. Checked AFTER the session: a password
    // is only removed after the stop, so a request that saw it gone sees this.
    if ctx.stop.load(Ordering::SeqCst) {
        let _ = request.respond(json_response("{\"error\":\"stopped\"}".into()).with_status_code(503));
        return;
    }

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
            if let Some(token) = cookie_of(&request).as_deref().and_then(auth::token_in) {
                ctx.auth.logout(token);
            }
            let cookie = header(b"Set-Cookie", auth::cleared_cookie().as_bytes());
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
            // Forward the raw JSON to the host app, which validates + delivers.
            (ctx.send)(body);
            let _ = request.respond(json_response("{\"ok\":true}".into()));
        }
        // Full web terminal: SSE output stream + raw input + resize.
        (Method::Get, "/term") => match pass {
            Some(pass) => {
                let watch = Watch::new(ctx.auth.clone(), pass, ctx.stop.clone());
                sse_terminal(request, ctx.terms, query_id(&url), watch)
            }
            None => respond_401(request),
        },
        (Method::Post, "/term/input") => {
            let v = read_body(&mut request);
            if let (Some(id), Some(data)) = (v.get("id").and_then(|x| x.as_str()), v.get("data").and_then(|x| x.as_str())) {
                if let Ok(bytes) = STANDARD.decode(data) {
                    ctx.terms.write(id, &bytes);
                }
            }
            let _ = request.respond(json_response("{\"ok\":true}".into()));
        }
        (Method::Post, "/term/resize") => {
            let v = read_body(&mut request);
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                let cols = v.get("cols").and_then(|x| x.as_u64()).unwrap_or(80) as u16;
                let rows = v.get("rows").and_then(|x| x.as_u64()).unwrap_or(24) as u16;
                ctx.terms.resize(id, cols, rows);
            }
            let _ = request.respond(json_response("{\"ok\":true}".into()));
        }
        _ => respond_404(request),
    }
}
