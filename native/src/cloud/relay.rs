//! The relay's session store over HTTPS (relay/src/storeroutes.ts), and signing
//! in with GitHub: the relay hands the login back to a port on 127.0.0.1 only.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// The relay; a local run sets BYTEHIVE_RELAY (wrangler dev).
pub fn base() -> String {
    std::env::var("BYTEHIVE_RELAY").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "https://relay.zoldify.com".into())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vault {
    pub salt: String,
    pub rounds: u32,
    pub check: String,
}

#[derive(Serialize, Debug)]
pub struct Segment {
    pub n: usize,
    pub sealed: String,
}

#[derive(Serialize, Debug)]
pub struct Put {
    pub sid: String,
    pub meta: String,
    pub updated: u64,
    pub segments: Vec<Segment>,
}

#[derive(Debug, PartialEq)]
pub enum Fail {
    /// The login is gone or expired: sign in again.
    SignIn,
    /// This machine was removed from the account.
    Revoked,
    /// The account's store is full.
    Quota,
    /// A vault is already there (set one with `replace`).
    Exists,
    Net(String),
    Other(u16),
}

impl Fail {
    pub fn words(&self) -> String {
        match self {
            Fail::SignIn => "Sign in to Remote again.".into(),
            Fail::Revoked => "This computer was removed from Remote.".into(),
            Fail::Quota => "The Remote store is full.".into(),
            Fail::Exists => "A passphrase is already set for this account.".into(),
            Fail::Net(e) => format!("The relay can't be reached: {e}"),
            Fail::Other(s) => format!("The relay answered {s}."),
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().new_agent()
}

fn check(status: u16) -> Result<(), Fail> {
    match status {
        200..=299 => Ok(()),
        401 => Err(Fail::SignIn),
        403 => Err(Fail::Revoked),
        409 => Err(Fail::Exists),
        413 => Err(Fail::Quota),
        s => Err(Fail::Other(s)),
    }
}

pub fn vault(token: &str) -> Result<Option<Vault>, Fail> {
    let mut r = agent().get(format!("{}/vault", base())).header("Authorization", &format!("Bearer {token}")).call().map_err(|e| Fail::Net(e.to_string()))?;
    if r.status().as_u16() == 404 {
        return Ok(None);
    }
    check(r.status().as_u16())?;
    r.body_mut().read_json().map(Some).map_err(|e| Fail::Net(e.to_string()))
}

fn send(method: &str, path: &str, token: &str, body: &serde_json::Value) -> Result<(), Fail> {
    let url = format!("{}{path}", base());
    let auth = format!("Bearer {token}");
    let req = match method {
        "PUT" => agent().put(url),
        _ => agent().post(url),
    };
    let r = req.header("Authorization", &auth).send_json(body).map_err(|e| Fail::Net(e.to_string()))?;
    check(r.status().as_u16())
}

pub fn set_vault(token: &str, v: &Vault, replace: bool) -> Result<(), Fail> {
    send("PUT", "/vault", token, &serde_json::json!({ "vault": v, "replace": replace }))
}

pub fn put(token: &str, p: &Put) -> Result<(), Fail> {
    send("POST", "/store/put", token, &serde_json::to_value(p).map_err(|e| Fail::Net(e.to_string()))?)
}

pub fn drop_session(token: &str, sid: &str) -> Result<(), Fail> {
    send("POST", "/store/drop", token, &serde_json::json!({ "sid": sid }))
}

/// A sign-in in progress: open `url` in the browser, then `wait` for the relay to come back.
pub struct Login {
    listener: TcpListener,
    state: String,
    pub url: String,
}

impl Login {
    pub fn start(machine: &str) -> Result<Login, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("no port for the sign-in: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let state = uuid::Uuid::new_v4().simple().to_string();
        let url = format!("{}/auth/start?peer=m:{machine}&state={state}&port={port}", base());
        Ok(Login { listener, state, url })
    }

    /// The login token, once the browser lands on the callback; gives up after `limit`.
    pub fn wait(self, limit: Duration) -> Result<String, String> {
        let until = Instant::now() + limit;
        self.listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        while Instant::now() < until {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).ok();
                    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                    let mut line = String::new();
                    BufReader::new(&stream).read_line(&mut line).ok();
                    let got = callback(&line, &self.state);
                    let page = if got.is_some() { "Signed in. You can close this tab and go back to ByteHive." } else { "This sign-in was not the one ByteHive asked for." };
                    let body = format!("<!doctype html><meta charset=utf-8><title>ByteHive</title><p style=\"font:16px system-ui;margin:40px\">{page}</p>");
                    let mut s = &stream;
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    if let Some(token) = got {
                        return Ok(token);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(150)),
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("The sign-in took too long. Try again.".into())
    }
}

/// The token from `GET /callback?token=..&state=..`, when `state` is ours.
fn callback(request_line: &str, state: &str) -> Option<String> {
    let target = request_line.split_whitespace().nth(1)?;
    let query = target.strip_prefix("/callback?")?;
    let mut token = None;
    let mut ok = false;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        match k {
            "token" => token = Some(unescape(v)),
            "state" => ok = unescape(v) == state,
            _ => {}
        }
    }
    token.filter(|t| ok && !t.is_empty())
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_token_only_with_our_state() {
        assert_eq!(callback("GET /callback?token=abc.def&state=s1 HTTP/1.1", "s1").as_deref(), Some("abc.def"));
        assert_eq!(callback("GET /callback?token=abc.def&state=other HTTP/1.1", "s1"), None);
        assert_eq!(callback("GET /elsewhere?token=abc&state=s1 HTTP/1.1", "s1"), None);
        assert_eq!(callback("GET /callback?state=s1 HTTP/1.1", "s1"), None);
    }

    #[test]
    fn unescapes_a_query_value() {
        assert_eq!(unescape("a%2Eb%3d"), "a.b=");
        assert_eq!(unescape("plain"), "plain");
        assert_eq!(unescape("bad%zz"), "bad%zz");
    }

    #[test]
    fn a_sign_in_answers_the_browser_and_hands_back_the_token() {
        let login = Login::start("m1").unwrap();
        assert!(login.url.contains("peer=m:m1"));
        let port: u16 = login.url.split("port=").nth(1).unwrap().parse().unwrap();
        let state = login.url.split("state=").nth(1).unwrap().split('&').next().unwrap().to_string();
        let t = std::thread::spawn(move || {
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(s, "GET /callback?token=tok.sig&state={state} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut back = String::new();
            std::io::Read::read_to_string(&mut s, &mut back).unwrap();
            back
        });
        assert_eq!(login.wait(Duration::from_secs(5)).unwrap(), "tok.sig");
        assert!(t.join().unwrap().contains("Signed in"));
    }
}
