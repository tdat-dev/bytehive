//! What Remote keeps on this machine: its id, the relay login and the store keys
//! (never the passphrase). The file is sealed with Windows DPAPI for the current
//! user, so another account on this machine, or a copy of the file, cannot use it.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::crypto::{Keys, b64, unb64};

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Saved {
    /// This machine's peer id without the `m:`: random, made once.
    pub machine: String,
    /// Remote is on: sessions are being saved.
    pub on: bool,
    /// The relay login (a signed token), once signed in.
    pub token: Option<String>,
    /// The store keys, base64url, once the passphrase was set or typed.
    pub enc: Option<String>,
    pub id: Option<String>,
}

impl Saved {
    pub fn keys(&self) -> Option<Keys> {
        let enc: [u8; 32] = unb64(self.enc.as_deref()?)?.try_into().ok()?;
        let id: [u8; 32] = unb64(self.id.as_deref()?)?.try_into().ok()?;
        Some(Keys { enc, id })
    }

    pub fn set_keys(&mut self, k: Option<&Keys>) {
        self.enc = k.map(|k| b64(&k.enc));
        self.id = k.map(|k| b64(&k.id));
    }
}

fn path() -> Option<PathBuf> {
    crate::fleet::data_dir().map(|d| d.join("remote.bin"))
}

/// What is saved, or a fresh record with a new machine id.
pub fn load() -> Saved {
    let read = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| unprotect(&b)).and_then(|b| serde_json::from_slice::<Saved>(&b).ok());
    read.unwrap_or_else(|| Saved { machine: uuid::Uuid::new_v4().simple().to_string(), ..Saved::default() })
}

pub fn save(s: &Saved) -> Result<(), String> {
    let p = path().ok_or("ByteHive's data folder can't be found")?;
    let plain = serde_json::to_vec(s).map_err(|e| e.to_string())?;
    let sealed = protect(&plain).ok_or("Windows would not protect the Remote keys")?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("bin.tmp");
    std::fs::write(&tmp, sealed).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn crypt(data: &[u8], seal: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData};
    let input = CRYPT_INTEGER_BLOB { cbData: u32::try_from(data.len()).ok()?, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: `input` points at `data` for the call; `out` is filled by Windows and freed below.
    let ok = unsafe {
        if seal {
            CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        }
    };
    if ok == 0 || out.pbData.is_null() {
        return None;
    }
    // SAFETY: Windows gave `out.cbData` bytes at `out.pbData`; copied, then handed back.
    let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
    unsafe { LocalFree(out.pbData as _) };
    Some(bytes)
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Option<Vec<u8>> {
    crypt(data, true)
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    crypt(data, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_round_trips_and_refuses_a_changed_blob() {
        let sealed = protect(b"store keys").unwrap();
        assert_ne!(sealed, b"store keys");
        assert_eq!(unprotect(&sealed).as_deref(), Some(&b"store keys"[..]));
        let mut bad = sealed.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert_eq!(unprotect(&bad), None);
    }

    #[test]
    fn keeps_the_keys_and_gives_them_back() {
        let k = Keys { enc: [1; 32], id: [2; 32] };
        let mut s = Saved::default();
        assert_eq!(s.keys(), None);
        s.set_keys(Some(&k));
        assert_eq!(s.keys(), Some(k));
        s.set_keys(None);
        assert_eq!(s.keys(), None);
    }
}
