//! The phone dashboard's password. It can type into agents, so once other
//! devices can reach it, it has to know who is asking.
//!
//! One password per install, kept only as an argon2id hash (random salt) in
//! `dashboard-auth.json` in the app-config dir; the plaintext is never stored
//! or logged. A good login gets a session: a random 32-byte token in an
//! HttpOnly, SameSite=Strict cookie, held in memory for 12 hours. A wrong one
//! waits half a second, and an address that keeps guessing waits longer and
//! longer; every address together gets GLOBAL_FAILS misses a minute, and only
//! MAX_CHECKS password checks run at once. With no password set, the
//! dashboard stays on localhost and needs no login, as before.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
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
const FREE_TRIES: u32 = 3;
/// The longest an address is made to wait.
const MAX_WAIT: Duration = Duration::from_secs(5 * 60);
/// Failures are forgotten after this long without another one.
const FORGET_AFTER: Duration = Duration::from_secs(15 * 60);
/// Password checks that may run at once: each argon2 check holds ~19 MiB.
pub const MAX_CHECKS: usize = 2;
/// Wrong passwords allowed per GLOBAL_WINDOW across every address together.
pub const GLOBAL_FAILS: usize = 10;
pub const GLOBAL_WINDOW: Duration = Duration::from_secs(60);
/// Shortest password the settings accept.
pub const MIN_LEN: usize = 10;
pub const COOKIE: &str = "maestro_dash";
const FILE: &str = "dashboard-auth.json";

#[derive(Serialize, Deserialize)]
struct Stored {
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

/// How long an address with `fails` wrong guesses must wait between tries.
fn wait_for(fails: u32) -> Duration {
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
        Ok(())
    }

    /// Remove the password (the dashboard goes back to localhost only).
    pub fn clear_password(&self) -> Result<(), String> {
        self.save(None)?;
        *lock(&self.hash) = None;
        lock(&self.sessions).clear();
        Ok(())
    }

    fn save(&self, hash: Option<&str>) -> Result<(), String> {
        let Some(path) = lock(&self.file).clone() else { return Ok(()) };
        match hash {
            Some(h) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("could not save the password: {e}"))?;
                }
                let body = serde_json::to_string(&Stored { hash: h.to_string() }).map_err(|e| e.to_string())?;
                std::fs::write(&path, body).map_err(|e| format!("could not save the password: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::sync::Arc;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));

    fn with_password(pw: &str) -> DashAuth {
        let a = DashAuth::default();
        a.set_password(pw).unwrap();
        a
    }

    #[test]
    fn hashes_with_argon2id_and_a_fresh_salt() {
        let a = hash_password("hunter22").unwrap();
        let b = hash_password("hunter22").unwrap();
        assert!(a.starts_with("$argon2id$"));
        assert_ne!(a, b, "each hash gets its own salt");
        assert!(!a.contains("hunter22"));
        assert!(verify_password(&a, "hunter22"));
        assert!(!verify_password(&a, "hunter23"));
        assert!(!verify_password("not a hash", "hunter22"));
    }

    #[test]
    fn refuses_a_short_password() {
        let a = DashAuth::default();
        assert!(a.set_password("abc").is_err());
        assert_eq!(a.set_password("ninechars"), Err("Use at least 10 characters.".to_string()));
        assert!(!a.has_password());
        assert!(a.set_password("tencharsok").is_ok());
    }

    /// Wrong guesses sent all at once from one address: the attempt is booked
    /// before the check, so they can't all slip past the backoff.
    #[test]
    fn a_burst_from_one_address_gets_only_the_free_tries() {
        let a = Arc::new(with_password("correct horse"));
        let gate = Arc::new(std::sync::Barrier::new(8));
        let results: Vec<Login> = (0..8)
            .map(|_| {
                let (a, gate) = (a.clone(), gate.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    a.login(IP, "nope", Instant::now())
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();
        let wrong = results.iter().filter(|r| **r == Login::Wrong).count();
        assert!(wrong <= FREE_TRIES as usize, "{wrong} guesses were checked: {results:?}");
        assert!(results.iter().all(|r| matches!(r, Login::Wrong | Login::Wait(_) | Login::Busy)));
    }

    /// Each argon2 check holds ~19 MiB: never more than two run at once, the
    /// rest are told to come back.
    #[test]
    fn no_more_than_two_checks_run_at_once() {
        let a = Arc::new(with_password("correct horse"));
        let gate = Arc::new(std::sync::Barrier::new(10));
        let results: Vec<Login> = (0..10u8)
            .map(|i| {
                let (a, gate) = (a.clone(), gate.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    a.login(IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)), "nope", Instant::now())
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();
        assert!(a.peak_checks() <= MAX_CHECKS, "{} at once", a.peak_checks());
        assert!(results.contains(&Login::Busy), "{results:?}");
        assert!(a.login(IP, "correct horse", Instant::now()) != Login::Busy, "a slot frees up again");
    }

    /// Many addresses (a LAN full of guessers) share one budget of misses.
    #[test]
    fn misses_from_every_address_share_one_budget() {
        let a = with_password("correct horse");
        let t0 = Instant::now();
        for i in 0..GLOBAL_FAILS as u8 {
            assert_eq!(a.login(IpAddr::V4(Ipv4Addr::new(10, 0, 1, i)), "nope", t0), Login::Wrong);
        }
        // the budget is spent: a fresh address waits, even with the right password
        let fresh = IpAddr::V4(Ipv4Addr::new(10, 0, 2, 1));
        assert!(matches!(a.login(fresh, "correct horse", t0), Login::Wait(_)));
        // a minute later it is back
        assert!(matches!(a.login(fresh, "correct horse", t0 + GLOBAL_WINDOW), Login::Ok(_)));
    }

    #[test]
    fn a_good_login_gives_its_try_back() {
        let a = with_password("correct horse");
        let t0 = Instant::now();
        for i in 0..(GLOBAL_FAILS as u8 + 2) {
            assert!(matches!(a.login(IpAddr::V4(Ipv4Addr::new(10, 0, 3, i)), "correct horse", t0), Login::Ok(_)));
        }
        assert_eq!(a.login(IP, "nope", t0), Login::Wrong, "good logins spent nothing");
    }

    #[test]
    fn right_password_gets_a_session_and_wrong_does_not() {
        let a = with_password("correct horse");
        let now = Instant::now();
        assert_eq!(a.login(IP, "wrong horse", now), Login::Wrong);
        let Login::Ok(token) = a.login(IP, "correct horse", now) else { panic!("should log in") };
        assert_eq!(token.len(), 64);
        assert!(a.session_ok(&token, now));
        assert!(!a.session_ok("made-up", now));
        assert!(a.allows(Some(&format!("other=1; {COOKIE}={token}")), now));
        assert!(!a.allows(None, now));
        assert!(!a.allows(Some("other=1"), now));
    }

    #[test]
    fn sessions_expire_after_12_hours() {
        let a = with_password("correct horse");
        let now = Instant::now();
        let Login::Ok(token) = a.login(IP, "correct horse", now) else { panic!() };
        assert!(a.session_ok(&token, now + SESSION_TTL - Duration::from_secs(1)));
        assert!(!a.session_ok(&token, now + SESSION_TTL));
    }

    #[test]
    fn logout_and_a_new_password_end_sessions() {
        let a = with_password("correct horse");
        let now = Instant::now();
        let Login::Ok(t1) = a.login(IP, "correct horse", now) else { panic!() };
        a.logout(&t1);
        assert!(!a.session_ok(&t1, now));
        let Login::Ok(t2) = a.login(IP, "correct horse", now) else { panic!() };
        a.set_password("battery staple").unwrap();
        assert!(!a.session_ok(&t2, now));
        assert_eq!(a.login(IP, "correct horse", now), Login::Wrong);
    }

    #[test]
    fn no_password_lets_everyone_in_but_nobody_log_in() {
        let a = DashAuth::default();
        assert!(a.allows(None, Instant::now()));
        assert_eq!(a.login(IP, "", Instant::now()), Login::Wrong);
        let a = with_password("correct horse");
        a.clear_password().unwrap();
        assert!(a.allows(None, Instant::now()));
    }

    #[test]
    fn an_address_that_keeps_guessing_has_to_wait() {
        let a = with_password("correct horse");
        let t0 = Instant::now();
        for _ in 0..FREE_TRIES {
            assert_eq!(a.login(IP, "nope", t0), Login::Wrong);
        }
        // now it waits, even with the right password
        assert!(matches!(a.login(IP, "correct horse", t0), Login::Wait(_)));
        // another address is not held up
        let other = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));
        assert!(matches!(a.login(other, "correct horse", t0), Login::Ok(_)));
        // after the wait it may try again; each miss doubles the wait
        let t1 = t0 + wait_for(FREE_TRIES);
        assert_eq!(a.login(IP, "nope", t1), Login::Wrong);
        assert_eq!(a.login(IP, "nope", t1 + Duration::from_millis(1500)), Login::Wait(Duration::from_millis(500)));
        assert!(matches!(a.login(IP, "correct horse", t1 + wait_for(FREE_TRIES + 1)), Login::Ok(_)));
        // a good login forgets the misses
        assert_eq!(a.login(IP, "nope", t1 + wait_for(FREE_TRIES + 1)), Login::Wrong);
    }

    #[test]
    fn waits_are_capped() {
        assert_eq!(wait_for(0), Duration::ZERO);
        assert_eq!(wait_for(FREE_TRIES), Duration::from_secs(1));
        assert_eq!(wait_for(100), MAX_WAIT);
    }

    #[test]
    fn keeps_only_the_hash_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let a = DashAuth::default();
        a.load_from(dir.path().to_path_buf());
        assert!(!a.has_password());
        a.set_password("correct horse").unwrap();
        let saved = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(saved.contains("$argon2id$"));
        assert!(!saved.contains("correct horse"));
        let b = DashAuth::default();
        b.load_from(dir.path().to_path_buf());
        assert!(b.has_password());
        assert!(matches!(b.login(IP, "correct horse", Instant::now()), Login::Ok(_)));
        b.clear_password().unwrap();
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn lan_needs_a_password() {
        assert_eq!(bind_addr(false, false), Ok(Ipv4Addr::LOCALHOST));
        assert!(bind_addr(true, false).is_err());
        assert_eq!(bind_addr(true, true), Ok(Ipv4Addr::UNSPECIFIED));
    }

    #[test]
    fn reads_the_token_from_a_cookie_header() {
        assert_eq!(token_in(&format!("a=1; {COOKIE}=abc; b=2")), Some("abc"));
        assert_eq!(token_in("a=1"), None);
        assert!(session_cookie("abc").contains("HttpOnly; SameSite=Strict; Path=/; Max-Age=43200"));
    }
}
