//! The remote fleet dashboard: a small HTTP view of the agent fleet that you
//! open from another device (your phone), a "send a message to an agent"
//! POST, and a live web terminal over a WebSocket on port + 1.
//!
//! The host app owns the fleet: it pushes a JSON snapshot ([`Dashboard::push`])
//! that the server serves at /api/fleet, and gets every POST /api/send body
//! through the `send` callback it passes to [`Dashboard::start`]. Terminals go
//! through the host's [`Terminals`]. Bound to localhost by default; other
//! devices (0.0.0.0) only with a password ([`auth`]), and with one set every
//! route that shows or drives an agent wants a signed-in session.

pub mod auth;
mod assets;
mod guard;
mod http;
mod stream;
mod terms;
#[cfg(test)]
mod testkit;
#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod stream_tests;

use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tiny_http::Server;

pub use auth::DashAuth;
pub use terms::{SharedTerminals, Terminals};
use guard::{urls_for, Guard};
use http::{serve, Ctx};
use stream::serve_ws;

/// The port the web app uses too (its Supervisor build moves to 8577).
pub const DEFAULT_PORT: u16 = 8477;

/// Hands a POST /api/send body to the host app.
pub type SendHook = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Default)]
pub struct Dashboard {
    inner: Mutex<Inner>,
    /// The password and who is signed in; outlives any one server.
    pub auth: Arc<DashAuth>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    port: u16,
    lan: bool,
    /// Computed once at start (off the UI thread), then cached.
    urls: Vec<String>,
    snapshot: Arc<Mutex<String>>,
    stop: Option<Arc<AtomicBool>>,
    /// The running server's threads: a restart waits for them, so the old
    /// server has let go of its ports before the new one takes them.
    threads: Vec<std::thread::JoinHandle<()>>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct DashboardInfo {
    pub running: bool,
    pub port: u16,
    pub lan: bool,
    pub urls: Vec<String>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn info(inner: &Inner) -> DashboardInfo {
    DashboardInfo {
        running: inner.running,
        port: inner.port,
        lan: inner.lan,
        // Cached: never probe the network here (the UI thread asks).
        urls: if inner.running { inner.urls.clone() } else { vec![] },
    }
}

/// Wait for a stopped server's threads, then make sure its port is let go.
/// tiny_http closes its listener on its own accept thread, which it wakes by
/// connecting to the address it listens on; 0.0.0.0 (other devices let in)
/// can't be connected to on Windows, so it stays blocked, holding the port,
/// until something connects. Connecting on loopback wakes it.
pub(crate) fn finish(threads: Vec<std::thread::JoinHandle<()>>, port: u16) {
    for t in threads {
        let _ = t.join();
    }
    let _ = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_millis(300));
}

/// How long a start waits for ports a server it just stopped is letting go of.
const BIND_PATIENCE: Duration = Duration::from_secs(3);

/// Bind the page's port and the terminal's (port + 1), trying again for a
/// little while: a server stopped a moment ago may still be letting go of them.
pub(crate) fn bind_both(bind: Ipv4Addr, port: u16, patience: Duration) -> Result<(Server, TcpListener), String> {
    let until = Instant::now() + patience;
    loop {
        let tried = match Server::http((bind, port)) {
            Err(e) => Err(format!("dashboard: cannot bind port {port}: {e}")),
            Ok(server) => match TcpListener::bind((bind, port + 1)) {
                Ok(ws) => Ok((server, ws)),
                Err(e) => {
                    drop(server);
                    finish(Vec::new(), port);
                    Err(format!("dashboard: cannot bind ws port {}: {e}", port + 1))
                }
            },
        };
        match tried {
            Ok(both) => return Ok(both),
            Err(e) if Instant::now() >= until => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

impl Dashboard {
    /// A dashboard keeping its password hash in `auth_dir` (none: in memory only).
    pub fn new(auth_dir: Option<PathBuf>) -> Dashboard {
        let dash = Dashboard::default();
        if let Some(dir) = auth_dir {
            dash.auth.load_from(dir);
        }
        dash
    }

    pub fn status(&self) -> DashboardInfo {
        info(&lock(&self.inner))
    }

    /// The latest fleet snapshot JSON, served at /api/fleet.
    pub fn push(&self, snapshot: String) {
        let inner = lock(&self.inner);
        *lock(&inner.snapshot) = snapshot;
    }

    pub fn has_password(&self) -> bool {
        self.auth.has_password()
    }

    /// Set or change the password (everyone signed in signs in again). Takes
    /// a moment on purpose (argon2): call it off the UI thread.
    pub fn set_password(&self, password: &str) -> Result<(), String> {
        self.auth.set_password(password)
    }

    pub fn stop(&self) -> DashboardInfo {
        let mut inner = lock(&self.inner);
        if let Some(stop) = inner.stop.take() {
            stop.store(true, Ordering::SeqCst);
        }
        // Waiting for the threads takes up to half a second: not on the UI thread.
        let (threads, port) = (std::mem::take(&mut inner.threads), inner.port);
        if !threads.is_empty() {
            std::thread::spawn(move || finish(threads, port));
        }
        inner.running = false;
        info(&inner)
    }

    /// Remove the password. Stop a server other devices can reach FIRST, then
    /// drop the password: the other way round left 0.0.0.0 open with no
    /// password until the server noticed the stop.
    pub fn clear_password(&self) -> Result<DashboardInfo, String> {
        let lan_on = {
            let inner = lock(&self.inner);
            inner.running && inner.lan
        };
        let stopped = lan_on.then(|| self.stop());
        self.auth.clear_password()?;
        Ok(stopped.unwrap_or_else(|| self.status()))
    }

    /// Start (or restart) the server on `port`, for other devices too when
    /// `lan` (never without a password). Blocks while the ports are bound and
    /// an old server lets go of them: call it off the UI thread.
    pub fn start(&self, port: u16, lan: bool, terms: SharedTerminals, send: SendHook) -> Result<DashboardInfo, String> {
        // Other devices can type into agents: never without a password.
        let bind = auth::bind_addr(lan, self.auth.has_password())?;
        let (snapshot, old_threads, old_port) = {
            let mut inner = lock(&self.inner);
            if let Some(stop) = inner.stop.take() {
                stop.store(true, Ordering::SeqCst);
            }
            inner.running = false;
            (inner.snapshot.clone(), std::mem::take(&mut inner.threads), inner.port)
        };
        if !old_threads.is_empty() {
            finish(old_threads, old_port);
        }
        let (server, ws_listener) = bind_both(bind, port, BIND_PATIENCE)?;
        let (urls, lan_ip) = urls_for(port, lan);
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = Ctx { snapshot, terms, auth: self.auth.clone(), send, guard: Guard { port, ws_port: port + 1, lan_ip, others: lan }, stop: stop.clone() };
        let (ctx_ws, stop_http, stop_ws) = (ctx.clone(), stop.clone(), stop.clone());
        let threads = vec![std::thread::spawn(move || serve(server, ctx, stop_http)), std::thread::spawn(move || serve_ws(ws_listener, stop_ws, ctx_ws))];
        let mut inner = lock(&self.inner);
        inner.running = true;
        inner.port = port;
        inner.lan = lan;
        inner.urls = urls;
        inner.stop = Some(stop);
        inner.threads = threads;
        Ok(info(&inner))
    }
}
