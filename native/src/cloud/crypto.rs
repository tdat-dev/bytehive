//! The Remote store's keys and seals (docs/superpowers/specs/2026-11-13-remote-session-store.md).
//! The web page does the same with WebCrypto (remote-web/src/store/crypto.ts);
//! the vectors in the tests are shared by both, so the two cannot drift.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// PBKDF2 rounds for a new vault.
pub const ROUNDS: u32 = 600_000;
/// The first byte of every sealed blob.
const VERSION: u8 = 1;
/// What the vault's check blob holds, sealed: a wrong passphrase cannot open it.
const CHECK: &[u8] = b"bytehive-remote-v1";
const CHECK_SLOT: &str = "vault/check";

/// The two keys a passphrase gives: one seals, one names sessions.
#[derive(Clone, PartialEq, Eq)]
pub struct Keys {
    pub enc: [u8; 32],
    pub id: [u8; 32],
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keys(..)")
    }
}

pub fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn unb64(s: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(s).ok()
}

/// PBKDF2-HMAC-SHA256, then HKDF-SHA256 (no salt) splits it into the two keys.
pub fn derive(passphrase: &str, salt: &[u8], rounds: u32) -> Keys {
    let mut master = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, rounds, &mut master);
    let hk = Hkdf::<Sha256>::new(None, &master);
    let mut keys = Keys { enc: [0; 32], id: [0; 32] };
    hk.expand(b"bytehive remote enc", &mut keys.enc).expect("32 bytes is a valid HKDF length");
    hk.expand(b"bytehive remote id", &mut keys.id).expect("32 bytes is a valid HKDF length");
    keys
}

/// A fresh random salt for a new vault.
pub fn new_salt() -> Result<[u8; 16], String> {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| format!("no random numbers: {e}"))?;
    Ok(salt)
}

/// Seal `plain` for `slot` (`<sid>/meta`, `<sid>/<n>`): the relay cannot move it elsewhere.
pub fn seal(keys: &Keys, slot: &str, plain: &[u8]) -> Result<String, String> {
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut nonce).map_err(|e| format!("no random numbers: {e}"))?;
    seal_with(keys, slot, plain, &nonce)
}

fn seal_with(keys: &Keys, slot: &str, plain: &[u8], nonce: &[u8; 12]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(&keys.enc).map_err(|e| e.to_string())?;
    let sealed = cipher
        .encrypt(Nonce::from_slice(nonce), Payload { msg: plain, aad: slot.as_bytes() })
        .map_err(|_| "sealing failed".to_string())?;
    let mut out = Vec::with_capacity(1 + 12 + sealed.len());
    out.push(VERSION);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&sealed);
    Ok(b64(&out))
}

/// The plain bytes, or None for a wrong key, another slot or a damaged blob.
pub fn open(keys: &Keys, slot: &str, sealed: &str) -> Option<Vec<u8>> {
    let raw = unb64(sealed)?;
    if raw.len() < 1 + 12 + 16 || raw[0] != VERSION {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(&keys.enc).ok()?;
    cipher.decrypt(Nonce::from_slice(&raw[1..13]), Payload { msg: &raw[13..], aad: slot.as_bytes() }).ok()
}

/// The vault's check blob for these keys.
pub fn check(keys: &Keys) -> Result<String, String> {
    seal(keys, CHECK_SLOT, CHECK)
}

/// Whether these keys open the vault's check blob: the passphrase was right.
pub fn checks(keys: &Keys, check: &str) -> bool {
    open(keys, CHECK_SLOT, check).as_deref() == Some(CHECK)
}

/// A session's name on the relay: stable for this machine, meaningless to the relay.
pub fn sid(keys: &Keys, machine: &str, cli: &str, session: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&keys.id).expect("HMAC takes any key length");
    mac.update(format!("{machine}:{cli}:{session}").as_bytes());
    mac.finalize().into_bytes()[..16].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "crypto_tests.rs"]
mod tests;
