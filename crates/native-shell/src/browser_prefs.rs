//! Settings > Browser: which of your browser profiles agents can use, and the
//! steps to add "ByteHive for Chrome" to one. A profile turns to Connected by
//! itself when its extension reaches the hub. Port of `browserprefs.ts`.

use browser_hub::profiles::ProfileRow;
use browser_hub::BrowserInfo;
use eframe::egui::{self, RichText};

use crate::prefs::Prefs;
use crate::settings_ui::{row, switch};
use crate::theme::{ADD, DEL, INK3, TEXT};

#[derive(Default)]
pub struct BrowserForm {
    pub profiles: Vec<ProfileRow>,
    pub connected: Vec<BrowserInfo>,
    pub ext_dir: String,
    /// The profile the setup steps are open for.
    pub setting_up: Option<ProfileRow>,
    /// A profile window to open, for the app to start.
    pub want_open: Option<ProfileRow>,
    pub error: Option<String>,
    pub hub_error: Option<String>,
}

fn short(browser: &str) -> &str {
    browser.trim_start_matches("Google ").trim_start_matches("Microsoft ")
}

/// A profile and a connected browser are the same when the browser and the
/// signed-in email match.
pub fn same(p: &ProfileRow, c: &BrowserInfo) -> bool {
    !p.email.is_empty() && p.email.eq_ignore_ascii_case(&c.email) && p.browser == c.browser
}

/// Connected browsers no profile row names (not signed in to the browser).
pub fn unnamed<'a>(profiles: &[ProfileRow], connected: &'a [BrowserInfo]) -> Vec<&'a BrowserInfo> {
    connected.iter().filter(|c| !profiles.iter().any(|p| same(p, c))).collect()
}

fn connected_label(ui: &mut egui::Ui) {
    ui.label(RichText::new("\u{25CF} Connected").size(12.5).color(ADD));
}

/// The rows; true when a setting changed.
pub fn browser_rows(ui: &mut egui::Ui, p: &mut Prefs, f: &mut BrowserForm) -> bool {
    let ch = row(ui, "Ask me before a risky click", "An agent that wants to send, post, pay or delete waits for your Allow.", |ui| switch(ui, &mut p.browser_ask).changed());
    if let Some(e) = &f.hub_error {
        ui.label(RichText::new(e).size(12.5).color(DEL));
    }
    if f.setting_up.as_ref().is_some_and(|s| f.connected.iter().any(|c| same(s, c))) {
        f.setting_up = None;
    }
    let mut open = None;
    for prof in &f.profiles {
        let on = f.connected.iter().any(|c| same(prof, c));
        let sub = format!("{} \u{00B7} {}", if prof.email.is_empty() { "Not signed in" } else { &prof.email }, short(&prof.browser));
        row(ui, &prof.name, &sub, |ui| {
            if on {
                connected_label(ui);
            } else if ui.button(RichText::new("Set up").size(12.5)).on_hover_text("Opens this profile so you can add the extension").clicked() {
                open = Some(prof.clone());
            }
            false
        });
    }
    for c in unnamed(&f.profiles, &f.connected) {
        let name = if c.profile.is_empty() { "A profile" } else { &c.profile };
        let sub = format!("{} \u{00B7} {}", if c.email.is_empty() { "Not signed in" } else { &c.email }, short(&c.browser));
        row(ui, name, &sub, |ui| {
            connected_label(ui);
            false
        });
    }
    if f.profiles.is_empty() && f.connected.is_empty() {
        ui.label(RichText::new("No Chrome, Edge or Brave profiles on this computer.").size(12.5).color(INK3));
    }
    if let Some(prof) = open {
        f.setting_up = Some(prof.clone());
        f.want_open = Some(prof);
    }
    if let Some(s) = f.setting_up.clone() {
        egui::Frame::new().fill(egui::Color32::from_white_alpha(8)).corner_radius(10.0).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.label(RichText::new(format!("Add ByteHive for Chrome to {}", s.name)).strong().color(TEXT));
            let b = short(&s.browser);
            ui.label(RichText::new(format!("1. In the {b} window that opened, go to {}://extensions.", if b == "Edge" { "edge" } else if b == "Brave" { "brave" } else { "chrome" })).size(12.5).color(INK3));
            ui.label(RichText::new("2. Turn on Developer mode (top right).").size(12.5).color(INK3));
            ui.horizontal(|ui| {
                ui.label(RichText::new("3. Click Load unpacked and pick this folder:").size(12.5).color(INK3));
                if ui.button(RichText::new("Copy").size(12.0)).clicked() {
                    ui.ctx().copy_text(f.ext_dir.clone());
                }
            });
            ui.label(RichText::new(&f.ext_dir).monospace().size(12.0).color(TEXT));
            ui.label(RichText::new("It turns to Connected here by itself.").size(12.0).color(INK3));
            if ui.button(RichText::new("Done").size(12.5)).clicked() {
                f.setting_up = None;
            }
        });
    }
    if let Some(e) = &f.error {
        ui.label(RichText::new(e).size(12.5).color(DEL));
    }
    ch
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prof(email: &str) -> ProfileRow {
        ProfileRow { browser: "Google Chrome".into(), dir: "Default".into(), name: "Work".into(), email: email.into() }
    }

    fn conn(email: &str) -> BrowserInfo {
        BrowserInfo { id: 1, browser: "Google Chrome".into(), email: email.into(), profile: "".into(), version: "1".into() }
    }

    #[test]
    fn matches_profiles_by_browser_and_email() {
        assert!(same(&prof("A@x.com"), &conn("a@x.com")));
        assert!(!same(&prof(""), &conn("")), "no email, no match");
        let c = [conn("a@x.com"), conn("z@x.com")];
        assert_eq!(unnamed(&[prof("a@x.com")], &c).len(), 1);
    }
}
