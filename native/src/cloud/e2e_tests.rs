//! Against a real relay on this machine. Ignored by default:
//!   cd relay && npx wrangler dev --port 18787
//!   set BYTEHIVE_RELAY=http://127.0.0.1:18787
//!   cargo test --bin bytehive-native cloud::e2e -- --ignored
//! The relay's dev sign-in (DEV_LOGIN=1, loopback only) stands in for GitHub.

use super::crypto::{self, open};
use super::relay::{self, Vault};
use super::sync::{Sent, Source, round};
use crate::transcript::Item;
use std::time::Duration;

fn no_redirects() -> ureq::Agent {
    ureq::Agent::config_builder().max_redirects(0).http_status_as_error(false).build().new_agent()
}

/// A machine signs in the way the app does: the relay hands the token to our loopback port.
fn machine_login(account: &str) -> String {
    let login = relay::Login::start("e2e-desk").unwrap();
    let url = format!("{}&account={account}", login.url);
    std::thread::spawn(move || {
        let _ = ureq::get(&url).call();
    });
    login.wait(Duration::from_secs(15)).unwrap()
}

/// A browser's sign-in: the token comes back in the fragment of the redirect.
fn device_login(account: &str) -> String {
    let r = no_redirects().get(format!("{}/auth/start?account={account}&peer=d:e2e-phone&state=s", relay::base())).call().unwrap();
    let to = r.headers().get("location").unwrap().to_str().unwrap().to_string();
    to.split("#token=").nth(1).unwrap().replace("%2E", ".").replace("%2e", ".")
}

fn get_json(token: &str, path: &str) -> serde_json::Value {
    let mut r = no_redirects().get(format!("{}{path}", relay::base())).header("Authorization", &format!("Bearer {token}")).call().unwrap();
    assert_eq!(r.status().as_u16(), 200, "{path}");
    r.body_mut().read_json().unwrap()
}

#[test]
#[ignore]
fn a_session_goes_up_sealed_and_another_device_reads_it() {
    assert!(std::env::var("BYTEHIVE_RELAY").is_ok(), "set BYTEHIVE_RELAY to the local relay");
    let account = format!("e2e-{}", uuid::Uuid::new_v4().simple());
    let token = machine_login(&account);

    // The passphrase makes the vault, as Settings does.
    let salt = crypto::new_salt().unwrap();
    let keys = crypto::derive("a long remote passphrase", &salt, 100_000);
    let vault = Vault { salt: crypto::b64(&salt), rounds: 100_000, check: crypto::check(&keys).unwrap() };
    relay::set_vault(&token, &vault, false).unwrap();

    let items: Vec<Item> = (0..250).map(|i| Item::User { id: format!("c{i}"), text: format!("ask {i}") }).collect();
    let src = Source { cli: "claude".into(), session: "s-1".into(), run_dir: "D:\\p".into(), agent: "Ana".into(), project: "bytehive".into(), started: 1 };
    let mut sent = Sent::default();
    let n = round(&keys, "e2e-desk", "DESK", &[src], &mut sent, 1000, &mut |_| Some(items.clone()), &mut |p| relay::put(&token, p)).unwrap();
    assert_eq!(n, 1);

    // Another device: same account, its own login, the passphrase typed there.
    let phone = device_login(&account);
    let v: Vault = serde_json::from_value(get_json(&phone, "/vault")).unwrap();
    let typed = crypto::derive("a long remote passphrase", &crypto::unb64(&v.salt).unwrap(), v.rounds);
    assert!(crypto::checks(&typed, &v.check));
    let list = get_json(&phone, "/store/list");
    let s = &list[0];
    let sid = s["sid"].as_str().unwrap();
    assert_eq!(s["segments"], 2);
    let meta: serde_json::Value = serde_json::from_slice(&open(&typed, &format!("{sid}/meta"), s["meta"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(meta["title"], "ask 0");
    assert_eq!(meta["items"], 250);
    let segs = get_json(&phone, &format!("/store/segments?sid={sid}&from=1"));
    let last: serde_json::Value = serde_json::from_slice(&open(&typed, &format!("{sid}/1"), segs["segments"][0]["sealed"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(last[49]["text"], "ask 249");

    // The relay keeps nothing readable: the text is nowhere in what it returns.
    assert!(!list.to_string().contains("ask 0"));
}
