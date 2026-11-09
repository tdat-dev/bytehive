//! Updates: the same channel as the Tauri app (`latest.json` on the public
//! tdat-dev/bytehive-releases, signed with the same minisign key), so one
//! release reaches both. Never the old name maestro-releases. The checking,
//! the toast and the Settings row are inbox/updates.rs; this part fetches,
//! compares, downloads and verifies, and is blocking: run it off the UI thread.

use std::io::Read as _;
use std::time::Duration;

use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};

pub const LATEST_URL: &str = "https://github.com/tdat-dev/bytehive-releases/releases/latest/download/latest.json";
/// The updater key (src-tauri/tauri.conf.json's pubkey): base64 of a
/// minisign public key file.
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVBMDJBOUQxN0Y5QzJDODcKUldTSExKeC8wYWtDV3FEU0JQN1ROeHBiL1FmVTREbkJVSHZkeEt1NXFRaGlGV3FoV0pLODhqK2wK";
/// The biggest installer taken.
const MAX_BYTES: u64 = 400 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub signature: String,
}

/// This build's version.
pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn parts(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches('v').split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

/// `latest` is a newer version than `now`.
pub fn newer(now: &str, latest: &str) -> bool {
    parts(latest) > parts(now)
}

/// A build run from target\ (a developer's) never checks by itself.
pub fn dev_build() -> bool {
    std::env::current_exe().ok().is_some_and(|p| p.components().any(|c| c.as_os_str() == "target"))
}

/// The Windows installer in a latest.json: the NSIS one when it is named.
pub fn parse_latest(text: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let p = ["windows-x86_64-nsis", "windows-x86_64"].iter().find_map(|k| v["platforms"].get(*k).filter(|p| p["url"].as_str().is_some_and(|u| u.ends_with(".exe"))))?;
    Some(Release { version: v["version"].as_str()?.trim_start_matches('v').to_string(), notes: v["notes"].as_str().unwrap_or_default().to_string(), url: p["url"].as_str()?.to_string(), signature: p["signature"].as_str()?.to_string() })
}

/// Check `bytes` against a latest.json signature (base64 of a minisign
/// signature file) and the key built in.
pub fn verify(bytes: &[u8], signature_b64: &str) -> Result<(), String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let key_text = String::from_utf8(b64.decode(PUBKEY).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let key_line = key_text.lines().nth(1).ok_or("bad key")?;
    let key = PublicKey::from_base64(key_line).map_err(|e| format!("bad key: {e}"))?;
    let sig_text = String::from_utf8(b64.decode(signature_b64.trim()).map_err(|_| "the signature is not base64")?).map_err(|e| e.to_string())?;
    let sig = Signature::decode(&sig_text).map_err(|e| format!("unreadable signature: {e}"))?;
    key.verify(bytes, &sig, true).map_err(|_| "the download does not match its signature".to_string())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(300))).build().into()
}

/// The newest release, as latest.json says.
pub fn fetch_latest() -> Result<Release, String> {
    let text = agent().get(LATEST_URL).call().map_err(|e| format!("couldn't reach the releases: {e}"))?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    parse_latest(&text).ok_or_else(|| "the release has no Windows installer".to_string())
}

/// Download, telling `progress` (got, total) as it goes; verify; save. The
/// installer's path.
pub fn download(r: &Release, progress: impl Fn(u64, u64)) -> Result<std::path::PathBuf, String> {
    let mut resp = agent().get(&r.url).call().map_err(|e| format!("download failed: {e}"))?;
    let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut reader = resp.body_mut().with_config().limit(MAX_BYTES).reader();
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = vec![0u8; 256 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| format!("download failed: {e}"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        progress(bytes.len() as u64, total);
    }
    verify(&bytes, &r.signature)?;
    let path = std::env::temp_dir().join(format!("ByteHive_{}_x64-setup.exe", r.version));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Run the installer silently; it opens ByteHive again once done (/UPDATE).
pub fn run_installer(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new(path).args(["/S", "/UPDATE"]).spawn().map(|_| ()).map_err(|e| format!("Couldn't start the installer: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{newer, parse_latest, verify};

    #[test]
    fn compares_versions() {
        assert!(newer("0.14.2", "0.15.0"));
        assert!(newer("0.9.9", "v0.10.0"));
        assert!(!newer("0.15.0", "0.15.0"));
        assert!(!newer("0.15.1", "0.15.0"));
    }

    #[test]
    fn reads_the_nsis_installer() {
        let json = r#"{"version":"0.15.0","notes":"n","platforms":{"windows-x86_64":{"url":"https://x/a.msi","signature":"s1"},"windows-x86_64-nsis":{"url":"https://x/ByteHive_0.15.0_x64-setup.exe","signature":"s2"}}}"#;
        let r = parse_latest(json).unwrap();
        assert_eq!((r.version.as_str(), r.signature.as_str()), ("0.15.0", "s2"));
        assert!(parse_latest(r#"{"version":"1","platforms":{}}"#).is_none());
    }

    #[test]
    #[ignore = "network"]
    fn reaches_the_releases() {
        let r = super::fetch_latest().unwrap();
        assert!(!r.version.is_empty());
    }

    #[test]
    fn refuses_what_is_not_signed() {
        assert!(verify(b"installer", "bm90IGEgc2lnbmF0dXJl").is_err());
        assert!(verify(b"installer", "!!").is_err());
    }
}
