//! Tests for the dashboard password and sessions.

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
fn a_pass_ends_with_its_session_or_its_password() {
    let a = with_password("correct horse");
    let now = Instant::now();
    let Login::Ok(t) = a.login(IP, "correct horse", now) else { panic!() };
    let cookie = format!("{COOKIE}={t}");
    assert!(a.pass(None, now).is_none());
    let p = a.pass(Some(&cookie), now).unwrap();
    assert!(a.still(&p, now));
    assert!(!a.still(&p, now + SESSION_TTL), "expired");
    a.logout(&t);
    assert!(!a.still(&p, now), "signed out");

    let Login::Ok(t) = a.login(IP, "correct horse", now) else { panic!() };
    let p = a.pass(Some(&format!("{COOKIE}={t}")), now).unwrap();
    a.set_password("battery staple").unwrap();
    assert!(!a.still(&p, now), "a new password");

    // with no password everyone may open one, until a password is set
    let open = DashAuth::default();
    let p = open.pass(None, now).unwrap();
    assert!(open.still(&p, now));
    open.set_password("correct horse").unwrap();
    assert!(!open.still(&p, now));
    open.clear_password().unwrap();
    assert!(!open.still(&p, now), "removed: opened under another password");
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

/// The hash file is replaced whole (temp file + rename): a reader, or a
/// crash, never meets a half-written or empty file.
#[test]
fn the_hash_file_is_never_seen_half_written() {
    let dir = tempfile::tempdir().unwrap();
    let a = Arc::new(DashAuth::default());
    a.load_from(dir.path().to_path_buf());
    let hash = hash_password("correct horse").unwrap();
    a.save(Some(&hash)).unwrap();
    let path = dir.path().join(FILE);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let (path, done) = (path.clone(), done.clone());
        std::thread::spawn(move || {
            let mut bad = 0;
            while !done.load(Ordering::SeqCst) {
                if let Ok(s) = std::fs::read_to_string(&path) {
                    if serde_json::from_str::<Stored>(&s).is_err() {
                        bad += 1;
                    }
                }
            }
            bad
        })
    };
    for _ in 0..300 {
        a.save(Some(&hash)).unwrap();
    }
    done.store(true, Ordering::SeqCst);
    assert_eq!(reader.join().unwrap(), 0, "a reader saw a half-written file");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from(FILE)], "no temp file left behind");
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
