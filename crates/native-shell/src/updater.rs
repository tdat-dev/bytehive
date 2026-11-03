//! Updates: the same channel as the Tauri app (`latest.json` on the public
//! tdat-dev/maestro-releases, signed with the same minisign key), so one
//! release reaches both. A check runs a little after start and every six
//! hours (not from a build under target\, which is always "older"); a newer
//! version shows on the top bar. Installing downloads the installer, checks
//! its signature against the key built in here, runs it, and closes ByteHive;
//! the installer opens it again.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use base64::Engine;
use eframe::egui;
use minisign_verify::{PublicKey, Signature};

use crate::app::ShellApp;

pub const LATEST_URL: &str = "https://github.com/tdat-dev/maestro-releases/releases/latest/download/latest.json";
/// The updater key (src-tauri/tauri.conf.json's pubkey): base64 of a
/// minisign public key file.
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVBMDJBOUQxN0Y5QzJDODcKUldTSExKeC8wYWtDV3FEU0JQN1ROeHBiL1FmVTREbkJVSHZkeEt1NXFRaGlGV3FoV0pLODhqK2wK";
const FIRST: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(6 * 3600);

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

/// The Windows installer in a latest.json: the NSIS one when it is named.
pub fn parse_latest(text: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let p = ["windows-x86_64-nsis", "windows-x86_64"].iter().find_map(|k| v["platforms"].get(*k).filter(|p| p["url"].as_str().is_some_and(|u| u.ends_with(".exe"))))?;
    Some(Release { version: v["version"].as_str()?.to_string(), notes: v["notes"].as_str().unwrap_or_default().to_string(), url: p["url"].as_str()?.to_string(), signature: p["signature"].as_str()?.to_string() })
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

fn fetch_latest() -> Result<Release, String> {
    let text = agent().get(LATEST_URL).call().map_err(|e| format!("couldn't reach the releases: {e}"))?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    parse_latest(&text).ok_or_else(|| "the release has no Windows installer".to_string())
}

/// Download, verify, save; the installer's path.
fn download(r: &Release) -> Result<std::path::PathBuf, String> {
    let bytes = agent().get(&r.url).call().map_err(|e| format!("download failed: {e}"))?.body_mut().with_config().limit(400 * 1024 * 1024).read_to_vec().map_err(|e| format!("download failed: {e}"))?;
    verify(&bytes, &r.signature)?;
    let path = std::env::temp_dir().join(format!("ByteHive_{}_x64-setup.exe", r.version));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

enum Msg {
    Checked(Result<Release, String>),
    Ready(Result<std::path::PathBuf, String>),
}

pub struct Updater {
    pub available: Option<Release>,
    pub status: String,
    pub busy: bool,
    /// Asking before closing the agents for an install.
    pub asking: bool,
    /// The automatic install was started once (a failure is not retried
    /// in a loop; the button still works).
    auto_tried: bool,
    /// You pressed Update now: a new version found installs straight away.
    install_found: bool,
    started: Instant,
    last: Option<Instant>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Default for Updater {
    fn default() -> Self {
        let (tx, rx) = channel();
        Updater { available: None, status: String::new(), busy: false, asking: false, auto_tried: false, install_found: false, started: Instant::now(), last: None, tx, rx }
    }
}

/// A build run from target\ (a developer's): never checks by itself.
fn dev_build() -> bool {
    std::env::current_exe().ok().is_some_and(|p| p.components().any(|c| c.as_os_str() == "target"))
}

impl Updater {
    pub fn check_now(&mut self, ctx: &egui::Context) {
        self.last = Some(Instant::now());
        self.busy = true;
        self.status = "Checking\u{2026}".into();
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Checked(fetch_latest()));
            ctx.request_repaint();
        });
    }

    /// Update now: look, and install what is found without a second press.
    pub fn update_now(&mut self, ctx: &egui::Context) {
        if self.available.is_some() {
            self.install(ctx);
            return;
        }
        self.install_found = true;
        self.check_now(ctx);
    }

    pub fn install(&mut self, ctx: &egui::Context) {
        let Some(r) = self.available.clone() else { return };
        self.busy = true;
        self.status = format!("Downloading {}\u{2026}", r.version);
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Ready(download(&r)));
            ctx.request_repaint();
        });
    }
}

impl ShellApp {
    /// Check when due, take answers, run a downloaded installer (logic pass).
    pub(crate) fn poll_updates(&mut self, ctx: &egui::Context) {
        let u = &mut self.extras.updates;
        let due = match u.last {
            None => u.started.elapsed() >= FIRST,
            Some(t) => t.elapsed() >= EVERY,
        };
        if due && self.prefs.update_check && !dev_build() && !cfg!(test) {
            u.check_now(ctx);
        }
        while let Ok(m) = u.rx.try_recv() {
            u.busy = false;
            match m {
                Msg::Checked(Ok(r)) if newer(current(), &r.version) => {
                    u.status = format!("Version {} is out.", r.version);
                    u.available = Some(r);
                    if std::mem::take(&mut u.install_found) {
                        u.install(ctx);
                    }
                }
                Msg::Checked(Ok(_)) => {
                    u.install_found = false;
                    u.status = format!("ByteHive {} is up to date.", current());
                }
                Msg::Checked(Err(e)) => {
                    u.install_found = false;
                    u.status = e;
                }
                Msg::Ready(Err(e)) => u.status = e,
                // Silent: the installer opens ByteHive again by itself once
                // done. With its pages shown, the finish page's ticked "Open
                // ByteHive" opened a second one on top.
                Msg::Ready(Ok(path)) => match std::process::Command::new(&path).args(["/S", "/UPDATE"]).spawn() {
                    Ok(_) => {
                        self.extras.system.quit_ok = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => u.status = format!("Couldn't start the installer: {e}"),
                },
            }
        }
        // A new version installs by itself once no agent is in the middle of
        // work; they come back stopped in the list and Resume carries each on.
        let now = crate::app::now_ms();
        let working = self.panes.iter().any(|p| p.cli.is_some() && p.exit_code.is_none() && crate::states::pane_status(p, now, None).state == supervise::task_state::TaskState::Working);
        let u = &mut self.extras.updates;
        if self.prefs.update_check && !dev_build() && !cfg!(test) && u.available.is_some() && !u.busy && !u.auto_tried && !working {
            u.auto_tried = true;
            u.install(ctx);
        }
    }

    /// "Update to x.y.z" on the top bar, and its ask when agents run.
    pub(crate) fn update_button(&mut self, ui: &mut egui::Ui) {
        let Some(r) = self.extras.updates.available.clone() else { return };
        let label = if self.extras.updates.busy { "Downloading\u{2026}".to_string() } else { format!("Update to {}", r.version) };
        let b = egui::Button::new(egui::RichText::new(label).size(12.0).color(crate::theme::PRIMARY_INK)).fill(crate::theme::PRIMARY).corner_radius(999.0);
        if ui.add_enabled(!self.extras.updates.busy, b).on_hover_text(r.notes.chars().take(600).collect::<String>()).clicked() {
            let live = self.panes.iter().any(|p| p.cli.is_some() && p.exit_code.is_none());
            if live {
                self.extras.updates.asking = true;
            } else {
                self.extras.updates.install(ui.ctx());
            }
        }
        if self.extras.updates.asking {
            let mut close = false;
            egui::Modal::new(egui::Id::unique("update-ask")).show(ui.ctx(), |ui| {
                ui.set_width(380.0);
                ui.label(egui::RichText::new(format!("Install ByteHive {}?", r.version)).size(15.0));
                ui.label(egui::RichText::new("ByteHive closes to update and the agents stop with it. They are back in the list when it opens; Resume carries each one on.").size(12.5).color(crate::theme::INK3));
                ui.horizontal(|ui| {
                    if ui.button("Update now").clicked() {
                        self.extras.updates.install(ui.ctx());
                        close = true;
                    }
                    close |= ui.button("Later").clicked();
                });
            });
            if close {
                self.extras.updates.asking = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("0.10.10", "0.11.0"));
        assert!(newer("0.9.9", "v0.10.0"));
        assert!(!newer("0.11.0", "0.11.0"));
        assert!(!newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn picks_the_nsis_installer() {
        let j = r#"{"version":"0.10.10","notes":"n","platforms":{"windows-x86_64":{"url":"https://x/a.msi","signature":"m"},"windows-x86_64-nsis":{"url":"https://x/a-setup.exe","signature":"s"}}}"#;
        let r = parse_latest(j).unwrap();
        assert_eq!((r.version.as_str(), r.url.as_str(), r.signature.as_str()), ("0.10.10", "https://x/a-setup.exe", "s"));
        assert_eq!(parse_latest(r#"{"version":"1","platforms":{"windows-x86_64":{"url":"a.msi","signature":"m"}}}"#), None, "no exe, nothing to run");
    }

    #[test]
    fn a_bad_signature_is_refused() {
        assert!(verify(b"hello", "not base64!").is_err());
        let fake = base64::engine::general_purpose::STANDARD.encode("untrusted comment: x\nRWQ=\ntrusted comment: y\nAA==\n");
        assert!(verify(b"hello", &fake).is_err());
    }

    /// Downloads the published installer and checks it for real:
    /// `cargo test real_release -- --ignored`.
    #[test]
    #[ignore]
    fn real_release_verifies() {
        let r = fetch_latest().unwrap();
        let bytes = agent().get(&r.url).call().unwrap().body_mut().with_config().limit(400 * 1024 * 1024).read_to_vec().unwrap();
        verify(&bytes, &r.signature).unwrap();
        let mut bad = bytes.clone();
        bad[1000] ^= 1;
        assert!(verify(&bad, &r.signature).is_err());
    }
}
