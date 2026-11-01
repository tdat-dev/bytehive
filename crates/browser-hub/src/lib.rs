//! ByteHive for Chrome: agents use the user's real browsers through an
//! extension, the way Claude in Chrome does. Tauri-free, so the native shell
//! hosts the same hub the Tauri app did.
//!
//! ```text
//! agent -> bytehive-mcp -ws-+
//!                          +- hub (in ByteHive)
//! extension - native ------+
//!           messaging -> bytehive host -ws-
//! ```
//!
//! See docs/superpowers/specs/2026-09-24-bytehive-browser-design.md.

pub mod host;
pub mod hub;
pub mod profiles;

use std::sync::Arc;

pub use hub::{Activity, Ask, Blocker, BrowserInfo, Hub, HubEvent};

/// The extension's fixed id (from the public `key` in its manifest).
pub const EXTENSION_ID: &str = "lhnncnhgomngapalcfncgehffmmibmbf";
/// The id the Chrome Web Store gave the published extension (the store strips `key`).
pub const STORE_EXTENSION_ID: &str = "gcclbkbkaijioodjbpjjmhnkfpmnoikk";
pub const HOST_NAME: &str = "com.bytehive.browser";
/// The name the extension used before the rename (Maestro): still
/// registered, so an extension that has not updated yet still connects.
pub const OLD_HOST_NAME: &str = "com.maestro.browser";

/// Start the hub, point this process's agents at it (BYTEHIVE_HUB_FILE),
/// and, when this ByteHive owns the browser, (re)register the native host.
pub fn start(on_change: Box<dyn Fn(HubEvent) + Send + Sync>) -> std::io::Result<Arc<Hub>> {
    let dir = std::path::PathBuf::from(extension_dir());
    let wanted = Box::new(move || {
        dev_sync(&dir);
        manifest_version(&dir)
    });
    let hub = Hub::start(on_change, wanted)?;
    if let Some(f) = hub::own_hub_file() {
        std::env::set_var("BYTEHIVE_HUB_FILE", f);
    }
    if hub::owns_browser() {
        std::thread::spawn(|| {
            if let Err(e) = register_host() {
                eprintln!("browser host registration: {e}");
            }
        });
    }
    Ok(hub)
}

pub fn manifest_version(dir: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(dir.join("manifest.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["version"].as_str().map(String::from))
}

/// In development, Tauri copies `browser-extension/` next to the exe only when
/// it rebuilds, and Chrome loads that copy. Refresh it from the repo whenever
/// the repo has another version, so an edit to the extension reaches Chrome
/// (the hub then tells it to reload). Release builds ship their own copy.
pub fn dev_sync(dst: &std::path::Path) {
    if !cfg!(debug_assertions) {
        return;
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("browser-extension");
    if src == dst || !src.join("manifest.json").exists() || manifest_version(&src) == manifest_version(dst) {
        return;
    }
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        let _ = std::fs::create_dir_all(to);
        for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
            let (f, t) = (e.path(), to.join(e.file_name()));
            if f.is_dir() {
                copy(&f, &t);
            } else {
                let _ = std::fs::copy(&f, &t);
            }
        }
    }
    copy(&src, dst);
}

/// Where the unpacked extension lives: next to the app when installed,
/// the repo's `browser-extension/` in development.
pub fn extension_dir() -> String {
    let exe = std::env::current_exe().ok();
    let beside = exe.as_ref().and_then(|e| e.parent()).map(|d| d.join("browser-extension"));
    if let Some(d) = beside.filter(|d| d.join("manifest.json").exists()) {
        return d.to_string_lossy().into_owned();
    }
    let dev = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("browser-extension");
    dunce_like(dev)
}

fn dunce_like(p: std::path::PathBuf) -> String {
    std::fs::canonicalize(&p)
        .map(|c| c.to_string_lossy().trim_start_matches(r"\\?\").to_string())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

/// Write the host manifest and point every Chromium browser's HKCU key at it,
/// as Claude Code does for its own extension.
#[cfg(windows)]
pub fn register_host() -> Result<(), String> {
    let local = std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(local).join("ByteHive").join("NativeHost");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe = host_copy(&dir)?;
    for name in [HOST_NAME, OLD_HOST_NAME] {
        register_name(&dir, &exe, name)?;
    }
    Ok(())
}

#[cfg(windows)]
fn register_name(dir: &std::path::Path, exe: &std::path::Path, name: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let manifest = dir.join(format!("{name}.json"));
    let body = serde_json::json!({
        "name": name,
        "description": "ByteHive browser bridge",
        "path": exe.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": [
            format!("chrome-extension://{EXTENSION_ID}/"),
            format!("chrome-extension://{STORE_EXTENSION_ID}/"),
        ],
    });
    std::fs::write(&manifest, serde_json::to_string_pretty(&body).unwrap()).map_err(|e| e.to_string())?;
    for vendor in [r"Google\Chrome", r"Microsoft\Edge", r"BraveSoftware\Brave-Browser", r"Chromium", r"Vivaldi"] {
        let key = format!(r"HKCU\Software\{vendor}\NativeMessagingHosts\{name}");
        let _ = std::process::Command::new("reg")
            .args(["add", &key, "/ve", "/t", "REG_SZ", "/d", &manifest.to_string_lossy(), "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    Ok(())
}

/// Chrome keeps the host running for as long as the extension is connected,
/// which is always. If the host were bytehive.exe itself, that would lock the
/// file, and neither an update nor a dev rebuild could replace it. So the host
/// is a copy, named after the build it came from. A new build writes a new
/// copy beside the old one, which Chrome is still running; old copies are
/// removed once they are no longer locked.
#[cfg(windows)]
fn host_copy(dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&exe).map_err(|e| e.to_string())?;
    let stamp = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let name = format!("bytehive-host-{:x}-{:x}.exe", meta.len(), stamp);
    let copy = dir.join(&name);
    if !copy.exists() {
        let tmp = dir.join(format!("{name}.part"));
        std::fs::copy(&exe, &tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &copy).map_err(|e| e.to_string())?;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("bytehive-host-") && n != name {
                let _ = std::fs::remove_file(e.path()); // fails while Chrome still runs it
            }
        }
    }
    Ok(copy)
}

#[cfg(not(windows))]
pub fn register_host() -> Result<(), String> {
    Ok(())
}
