//! A live terminal for the page: the WebSocket on port + 1 (typing and
//! output both ways) and the Server-Sent Events fallback on /term. Both end
//! within [`RECHECK`] of their session ending or the server stopping.

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use tiny_http::{Request, Response, StatusCode};

use crate::auth::{DashAuth, Pass};
use crate::http::{header, respond_404, Ctx};
use crate::terms::SharedTerminals;

/// How often an open stream checks its session is still good.
pub(crate) const RECHECK: Duration = Duration::from_millis(250);

/// Stream an agent's live output to the browser as Server-Sent Events. Each PTY
/// chunk is base64-encoded into one `data:` frame; xterm.js decodes and writes
/// it. Blocks in its own thread until the agent exits or the client disconnects.
pub(crate) fn sse_terminal(request: Request, terms: SharedTerminals, id: Option<String>, watch: Watch) {
    let Some(id) = id else { return respond_404(request) };
    let Some(rx) = terms.tap(&id) else { return respond_404(request) };
    let headers = vec![header(b"Content-Type", b"text/event-stream"), header(b"Cache-Control", b"no-cache")];
    let reader = SseReader { rx, pending: Vec::new(), pos: 0, watch };
    // data_length None: chunked transfer, streamed as the reader yields.
    let resp = Response::new(StatusCode(200), headers, reader, None, None);
    let _ = request.respond(resp);
}

/// WebSocket terminal server (port+1). A real bidirectional low-latency channel
/// for the full web terminal: TCP_NODELAY on, ordered by nature, no per-key HTTP
/// round-trips, so typing feels near-direct.
pub(crate) fn serve_ws(listener: TcpListener, stop: Arc<AtomicBool>, ctx: Ctx) {
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
    uri.split_once('?').and_then(|(_, q)| q.split('&').find_map(|p| p.strip_prefix("id="))).unwrap_or("").to_string()
}

fn ws_conn(stream: TcpStream, ctx: Ctx) {
    let Ctx { terms, auth, stop, guard, .. } = ctx;
    let _ = stream.set_nodelay(true); // the whole point: no Nagle coalescing
    // A socket accepted from the non-blocking listener is non-blocking too on
    // Windows; the handshake needs to wait for the request.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut agent_id = String::new();
    let mut pass = None;
    let ws = tungstenite::accept_hdr(stream, |req: &tungstenite::handshake::server::Request, resp: tungstenite::handshake::server::Response| {
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
        pass = auth.pass(get("Cookie"), Instant::now());
        if pass.is_none() {
            return refuse(tungstenite::http::StatusCode::UNAUTHORIZED, "sign in");
        }
        // after the session check, as in `handle`
        if stop.load(Ordering::SeqCst) {
            return refuse(tungstenite::http::StatusCode::SERVICE_UNAVAILABLE, "stopped");
        }
        agent_id = ws_query_id(&req.uri().to_string());
        Ok(resp)
    });
    let Ok(mut ws) = ws else { return };
    let Some(pass) = pass else { return };
    if agent_id.is_empty() {
        return;
    }
    let mut watch = Watch::new(auth, pass, stop);
    let Some(rx) = terms.tap(&agent_id) else { return };
    // However the socket ends, the host hears this page let go.
    let _release = Release(terms.clone(), agent_id.clone());
    let _ = ws.get_mut().set_nonblocking(true);
    loop {
        // Signed out, a new or removed password, or the server stopping: hang up.
        if !watch.alive() {
            let _ = ws.close(None);
            let _ = ws.flush();
            return;
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
        // Input: keystrokes from the browser to the PTY (in order, no round-trips).
        match ws.read() {
            Ok(tungstenite::Message::Binary(d)) => {
                terms.write(&agent_id, &d);
            }
            Ok(tungstenite::Message::Text(t)) => {
                terms.write(&agent_id, t.as_bytes());
            }
            Ok(tungstenite::Message::Close(_)) => return,
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return,
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Tells the host a page's terminal of an agent closed, when dropped.
struct Release(SharedTerminals, String);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.release(&self.1);
    }
}

/// Keeps an open stream honest: it ends within RECHECK of its session ending
/// (sign-out, expiry, a new or removed password) or of the server stopping,
/// instead of running on for the life of the agent.
pub(crate) struct Watch {
    auth: Arc<DashAuth>,
    pass: Pass,
    stop: Arc<AtomicBool>,
    next: Instant,
}

impl Watch {
    pub(crate) fn new(auth: Arc<DashAuth>, pass: Pass, stop: Arc<AtomicBool>) -> Self {
        Watch { auth, pass, stop, next: Instant::now() + RECHECK }
    }

    fn alive(&mut self) -> bool {
        if self.stop.load(Ordering::SeqCst) {
            return false;
        }
        let now = Instant::now();
        if now < self.next {
            return true;
        }
        self.next = now + RECHECK;
        self.auth.still(&self.pass, now)
    }
}

/// A blocking `Read` that turns an agent's output channel into an SSE byte
/// stream. Reads wait on the channel; a closed channel (agent gone) or an
/// ended session ends it.
struct SseReader {
    rx: Receiver<Vec<u8>>,
    pending: Vec<u8>,
    pos: usize,
    watch: Watch,
}

impl Read for SseReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.pending.len() {
            if !self.watch.alive() {
                return Ok(0); // signed out: EOF ends the stream
            }
            match self.rx.recv_timeout(RECHECK) {
                Err(RecvTimeoutError::Timeout) => continue,
                Ok(chunk) => {
                    // tiny_http wraps the socket in a 1024-byte BufWriter and only
                    // flushes when it fills or the response ends, but an SSE stream
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
                Err(_) => return Ok(0), // agent exited: EOF closes the stream
            }
        }
        let n = std::cmp::min(out.len(), self.pending.len() - self.pos);
        out[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}
