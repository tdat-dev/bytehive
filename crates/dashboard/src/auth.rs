//! The phone dashboard's password. It can type into agents, so once other
//! devices can reach it, it has to know who is asking.
//!
//! One password per install, kept only as an argon2id hash (random salt) in
//! `dashboard-auth.json` in the app-config dir (`%APPDATA%\com.maestro.app`,
//! the same file the Tauri app uses, so one password serves both); the plaintext is never stored
//! or logged. A good login gets a session: a random 32-byte token in an
//! HttpOnly, SameSite=Strict cookie, held in memory for 12 hours. A wrong one
//! waits half a second, and an address that keeps guessing waits longer and
//! longer; every address together gets GLOBAL_FAILS misses a minute, and only
//! MAX_CHECKS password checks run at once. With no password set, the
//! dashboard stays on localhost and needs no login, as before.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};

/// How long a login lasts.
pub const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);
/// Every wrong password costs this, so guessing is slow even before backoff.
pub const FAIL_DELAY: Duration = Duration::from_millis(500);
/// Wrong guesses an address gets before it has to wait between tries.
pub(crate) const FREE_TRIES: u32 = 3;
/// The longest an address is made to wait.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(5 * 60);
/// Failures are forgotten after this long without another one.
const FORGET_AFTER: Duration = Duration::from_secs(15 * 60);
/// Password checks that may run at once: each argon2 check holds ~19 MiB.
pub const MAX_CHECKS: usize = 2;
/// Wrong passwords allowed per GLOBAL_WINDOW across every address together.
pub const GLOBAL_FAILS: usize = 10;
pub const GLOBAL_WINDOW: Duration = Duration::from_secs(60);
/// Shortest password the settings accept.
pub const MIN_LEN: usize = 10;
pub const COOKIE: &str = "bytehive_dash";
pub(crate) const FILE: &str = "dashboard-auth.json";

#[derive(Serialize, Deserialize)]
pub(crate) struct Stored {
    hash: String,
}

struct Fails {
    count: u32,
    last: Instant,
}

/// Wrong guesses: per address, and every one in the last GLOBAL_WINDOW.
#[derive(Default)]
struct Misses {
    by_ip: HashMap<IpAddr, Fails>,
    recent: VecDeque<Instant>,
}

/// One of the MAX_CHECKS password-check slots, given back on drop.
struct Slot<'a>(&'a AtomicUsize);

impl<'a> Slot<'a> {
    fn take(running: &'a AtomicUsize, peak: &AtomicUsize) -> Option<Self> {
        let mut n = running.load(Ordering::SeqCst);
        loop {
            if n >= MAX_CHECKS {
                return None;
            }
            match running.compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => {
                    peak.fetch_max(n + 1, Ordering::SeqCst);
                    return Some(Slot(running));
                }
                Err(now) => n = now,
            }
        }
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub struct DashAuth {
    /// Where the hash is kept; None until startup knows the config dir (and in tests).
    file: Mutex<Option<PathBuf>>,
    hash: Mutex<Option<String>>,
    sessions: Mutex<HashMap<String, Instant>>,
    fails: Mutex<Misses>,
    /// Password checks running now, and the most ever at once.
    checks: AtomicUsize,
    peak: AtomicUsize,
    /// Bumped whenever the password is set, changed or removed: a stream
    /// opened before that ends with it.
    generation: AtomicU64,
}

/// What an open stream (terminal socket, SSE) was let in with, so it can
/// check, while it runs, that it still would be.
pub struct Pass {
    cookie: Option<String>,
    generation: u64,
}

#[derive(Debug, PartialEq)]
pub enum Login {
    /// The session token to set as the cookie.
    Ok(String),
    Wrong,
    /// Too many wrong guesses from this address: try again after this long.
    Wait(Duration),
    /// Too many checks running already: try again in a moment.
    Busy,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(|e| format!("no randomness: {e}"))?;
    Ok(b)
}

/// argon2id hash (PHC string) of `password` with a fresh random salt.
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::encode_b64(&random_bytes::<16>()?).map_err(|e| e.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("could not hash the password: {e}"))
}

/// Does `password` match `hash`? (argon2 compares in constant time.)
pub fn verify_password(hash: &str, password: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// Replace `path` with `body` all at once: write a temp file next to it, flush
/// it to disk, then rename it over. A crash or a reader mid-way sees the old
/// file or the new one, never a truncated one.
fn write_whole(path: &std::path::Path, body: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // Windows refuses the rename for a moment while something (a virus
    // scanner) has the old file open without sharing: try a few times.
    let mut tries = 0;
    loop {
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && tries < 10 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
        }
    }
}

/// How long an address with `fails` wrong guesses must wait between tries.
pub(crate) fn wait_for(fails: u32) -> Duration {
    if fails < FREE_TRIES {
        return Duration::ZERO;
    }
    let secs = 1u64 << (fails - FREE_TRIES).min(16);
    Duration::from_secs(secs).min(MAX_WAIT)
}

/// The session token in a `Cookie:` header value, if any.
pub fn token_in(cookie_header: &str) -> Option<&str> {
    cookie_header
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v)
}

/// `Set-Cookie` value for a new session.
pub fn session_cookie(token: &str) -> String {
    format!("{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}", SESSION_TTL.as_secs())
}

/// `Set-Cookie` value that ends the session in the browser.
pub fn cleared_cookie() -> String {
    format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
}

impl DashAuth {
    /// Read the stored hash from `dir` (the app-config dir) and keep writing there.
    pub fn load_from(&self, dir: PathBuf) {
        let path = dir.join(FILE);
        let hash = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Stored>(&s).ok())
            .map(|s| s.hash)
            .filter(|h| PasswordHash::new(h).is_ok());
        *lock(&self.hash) = hash;
        *lock(&self.file) = Some(path);
    }

    pub fn has_password(&self) -> bool {
        lock(&self.hash).is_some()
    }

    /// Set or change the password. Every device signed in has to sign in again.
    pub fn set_password(&self, password: &str) -> Result<(), String> {
        if password.chars().count() < MIN_LEN {
            return Err(format!("Use at least {MIN_LEN} characters."));
        }
        let hash = hash_password(password)?;
        self.save(Some(&hash))?;
        *lock(&self.hash) = Some(hash);
        lock(&self.sessions).clear();
        self.generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Remove the password (the dashboard goes back to localhost only).
    pub fn clear_password(&self) -> Result<(), String> {
        self.save(None)?;
        *lock(&self.hash) = None;
        lock(&self.sessions).clear();
        self.generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn save(&self, hash: Option<&str>) -> Result<(), String> {
        let Some(path) = lock(&self.file).clone() else { return Ok(()) };
        match hash {
            Some(h) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("could not save the password: {e}"))?;
                }
                let body = serde_json::to_string(&Stored { hash: h.to_string() }).map_err(|e| e.to_string())?;
                write_whole(&path, body.as_bytes()).map_err(|e| format!("could not save the password: {e}"))
            }
            None => match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("could not remove the password: {e}")),
                _ => Ok(()),
            },
        }
    }

    /// Check a login from `ip`. Doesn't sleep: the caller holds a wrong answer
    /// back by FAIL_DELAY.
    ///
    /// Requests arrive in parallel (a thread each), so the attempt is booked
    /// as a miss BEFORE the slow check and given back if it was right: a burst
    /// can't all pass the backoff while none has failed yet. At most
    /// MAX_CHECKS checks run at once, and every address shares GLOBAL_FAILS.
    pub fn login(&self, ip: IpAddr, password: &str, now: Instant) -> Login {
        let Some(hash) = lock(&self.hash).clone() else { return Login::Wrong };
        let Some(_slot) = Slot::take(&self.checks, &self.peak) else { return Login::Busy };
        {
            let mut g = lock(&self.fails);
            g.by_ip.retain(|_, f| now.saturating_duration_since(f.last) < FORGET_AFTER);
            g.recent.retain(|t| now.saturating_duration_since(*t) < GLOBAL_WINDOW);
            if let Some(f) = g.by_ip.get(&ip) {
                let ready = f.last + wait_for(f.count);
                if now < ready {
                    return Login::Wait(ready - now);
                }
            }
            if g.recent.len() >= GLOBAL_FAILS {
                let ready = g.recent[0] + GLOBAL_WINDOW;
                return Login::Wait(ready.saturating_duration_since(now).max(Duration::from_secs(1)));
            }
            let f = g.by_ip.entry(ip).or_insert(Fails { count: 0, last: now });
            f.count += 1;
            f.last = now;
            g.recent.push_back(now);
        }
        if !verify_password(&hash, password) {
            return Login::Wrong;
        }
        {
            // right after all: a good login forgets this address's misses and
            // gives back the one it booked
            let mut g = lock(&self.fails);
            g.by_ip.remove(&ip);
            if let Some(i) = g.recent.iter().rposition(|t| *t == now) {
                g.recent.remove(i);
            }
        }
        let token: String = match random_bytes::<32>() {
            Ok(b) => b.iter().map(|x| format!("{x:02x}")).collect(),
            Err(_) => return Login::Wrong,
        };
        let mut sessions = lock(&self.sessions);
        sessions.retain(|_, until| *until > now);
        sessions.insert(token.clone(), now + SESSION_TTL);
        Login::Ok(token)
    }

    /// The most password checks that ever ran at once.
    pub fn peak_checks(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    /// Is this a live session?
    pub fn session_ok(&self, token: &str, now: Instant) -> bool {
        lock(&self.sessions).get(token).is_some_and(|until| *until > now)
    }

    pub fn logout(&self, token: &str) {
        lock(&self.sessions).remove(token);
    }

    /// May a request carrying this `Cookie:` header in? Always, with no password.
    pub fn allows(&self, cookie_header: Option<&str>, now: Instant) -> bool {
        if !self.has_password() {
            return true;
        }
        cookie_header.and_then(token_in).is_some_and(|t| self.session_ok(t, now))
    }

    /// Like `allows`, for a request that opens a stream: the pass it gets
    /// lets the stream check later that it still may run.
    pub fn pass(&self, cookie_header: Option<&str>, now: Instant) -> Option<Pass> {
        // read before the check: a password changed in between shows up later
        let generation = self.generation.load(Ordering::SeqCst);
        self.allows(cookie_header, now).then(|| Pass { cookie: cookie_header.map(str::to_string), generation })
    }

    /// May a stream opened with `pass` keep running? Not after its session
    /// signed out or expired, or the password was set, changed or removed.
    pub fn still(&self, pass: &Pass, now: Instant) -> bool {
        self.generation.load(Ordering::SeqCst) == pass.generation && self.allows(pass.cookie.as_deref(), now)
    }
}

/// Where the dashboard may listen. Other devices only with a password.
pub fn bind_addr(lan: bool, has_password: bool) -> Result<std::net::Ipv4Addr, String> {
    if !lan {
        return Ok(std::net::Ipv4Addr::LOCALHOST);
    }
    if !has_password {
        return Err("Set a dashboard password before letting other devices in.".into());
    }
    Ok(std::net::Ipv4Addr::UNSPECIFIED)
}

/// Where the Tauri app keeps its config (its `app_config_dir`, still under
/// its old identifier com.maestro.app), so the native app reads and writes
/// the same password file.
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("com.maestro.app"))
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
