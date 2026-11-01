//! Settings > Remote: the phone dashboard on or off, other devices let in,
//! its password, and where to open it. The password is typed here and
//! handed to the dashboard crate, which keeps only an argon2 hash; it is
//! never saved in the settings file.

use eframe::egui::{self, RichText};

use crate::prefs::Prefs;
use crate::settings_ui::{row, switch};
use crate::theme::{DEL, DIM, INK3, PRIMARY, PRIMARY_INK, TEXT};

/// Shortest password the dashboard accepts (its auth MIN_LEN).
pub const MIN_PASSWORD: usize = dashboard::auth::MIN_LEN;
const LAN_CLEARTEXT: &str = "On your Wi-Fi the password goes unencrypted: use it only on a network you trust.";

/// What the Remote rows ask the app to do.
#[derive(Debug, PartialEq)]
pub enum RemoteCmd {
    /// Start or stop to match the switches.
    Apply,
    SetPassword(String),
    ClearPassword,
}

#[derive(Default)]
pub struct RemoteForm {
    pub info: dashboard::DashboardInfo,
    pub has_password: bool,
    /// A start, stop or password change is running.
    pub busy: bool,
    pub error: Option<String>,
    pub note: Option<String>,
    pw_open: bool,
    pw1: String,
    pw2: String,
    pw_error: Option<String>,
    confirm_clear: bool,
    pub want: Option<RemoteCmd>,
}

/// What is wrong with a new password typed twice, or none when it will do.
pub fn password_problem(first: &str, again: &str) -> Option<String> {
    if first.is_empty() {
        return Some("Type a password.".into());
    }
    if first.chars().count() < MIN_PASSWORD {
        return Some(format!("Use at least {MIN_PASSWORD} characters."));
    }
    (first != again).then(|| "The two don't match.".into())
}

fn link(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(12.5).color(color)).frame(false)).clicked()
}

/// The Remote rows; true when a saved setting changed.
pub fn remote_rows(ui: &mut egui::Ui, p: &mut Prefs, f: &mut RemoteForm) -> bool {
    let mut ch = false;
    let why = if f.info.running {
        format!("Open on any device on your network: {}", f.info.urls.join("  \u{00B7}  "))
    } else {
        "Off. Turn it on to see the fleet and type into agents from your phone.".to_string()
    };
    let on = row(ui, "Remote dashboard", &why, |ui| {
        ui.add_enabled_ui(!f.busy, |ui| switch(ui, &mut p.remote_on).changed()).inner
    });
    if on {
        f.want = Some(RemoteCmd::Apply);
        ch = true;
    }
    if let Some(e) = &f.error {
        ui.label(RichText::new(e).size(12.5).color(DEL));
    }
    let lan_why = if f.has_password {
        format!(
            "Reachable from devices on the same Wi-Fi, after signing in. {LAN_CLEARTEXT} From another network: put Tailscale on this computer and your phone, then open this computer's Tailscale address (100.x.y.z) with :{}.",
            p.remote_port
        )
    } else {
        format!("Set a password first. Without one, anyone on your network could type into your agents. {LAN_CLEARTEXT}")
    };
    let lan = row(ui, "Let other devices in", &lan_why, |ui| {
        let can = !f.busy && (f.has_password || p.remote_lan);
        ui.add_enabled_ui(can, |ui| switch(ui, &mut p.remote_lan).changed()).inner
    });
    if lan {
        ch = true;
        if p.remote_on {
            f.want = Some(RemoteCmd::Apply); // bind on the new interface
        }
    }
    let port = row(ui, "Port", "Where the page listens; its live terminal takes the next port too. Applies when the dashboard (re)starts.", |ui| {
        let r = ui.add_enabled(!f.busy, egui::DragValue::new(&mut p.remote_port).range(1024..=65534));
        r.lost_focus() || r.drag_stopped()
    });
    if port {
        ch = true;
        if p.remote_on {
            f.want = Some(RemoteCmd::Apply);
        }
    }
    let pw_why = f.note.clone().unwrap_or_else(|| {
        if f.has_password {
            "Set. Anyone opening the dashboard signs in with it, this computer included.".into()
        } else {
            "None yet. Other devices need one before they can connect.".into()
        }
    });
    row(ui, "Password", &pw_why, |ui| {
        ui.add_enabled_ui(!f.busy, |ui| {
            if f.has_password && link(ui, "Remove", DEL) {
                f.confirm_clear = true;
            }
            if link(ui, if f.has_password { "Change" } else { "Set password" }, TEXT) {
                f.pw_open = !f.pw_open;
                f.pw1.clear();
                f.pw2.clear();
                f.pw_error = None;
            }
        });
        false
    });
    if f.confirm_clear {
        let lan_on = f.info.running && p.remote_lan;
        let msg = if lan_on {
            "Remove the password? Other devices can't connect without one, so the dashboard goes back to this computer only."
        } else {
            "Remove the password? The dashboard on this computer opens without signing in. Other devices can't connect until you set one again."
        };
        ui.label(RichText::new(msg).size(12.5).color(DIM));
        ui.horizontal(|ui| {
            if link(ui, "Remove", DEL) {
                f.want = Some(RemoteCmd::ClearPassword);
                f.confirm_clear = false;
                f.note = None;
            }
            if link(ui, "Keep it", INK3) {
                f.confirm_clear = false;
            }
        });
    }
    if f.pw_open {
        password_form(ui, f);
    }
    ch
}

fn password_form(ui: &mut egui::Ui, f: &mut RemoteForm) {
    ui.label(RichText::new(if f.has_password { "Change the password" } else { "Set a password" }).size(12.5).color(TEXT));
    ui.add(egui::TextEdit::singleline(&mut f.pw1).password(true).hint_text("New password").desired_width(260.0));
    ui.add(egui::TextEdit::singleline(&mut f.pw2).password(true).hint_text("Type it again").desired_width(260.0));
    if let Some(e) = &f.pw_error {
        ui.label(RichText::new(e).size(12.5).color(DEL));
    }
    ui.horizontal(|ui| {
        let save = egui::Button::new(RichText::new("Save").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0);
        if ui.add(save).clicked() {
            match password_problem(&f.pw1, &f.pw2) {
                Some(e) => f.pw_error = Some(e),
                None => {
                    f.want = Some(RemoteCmd::SetPassword(std::mem::take(&mut f.pw1)));
                    f.pw2.clear();
                    f.pw_open = false;
                    f.note = Some("Saving\u{2026}".into());
                }
            }
        }
        if link(ui, "Cancel", INK3) {
            f.pw_open = false;
            f.pw1.clear();
            f.pw2.clear();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_a_new_password() {
        assert_eq!(password_problem("", ""), Some("Type a password.".into()));
        assert_eq!(password_problem("short", "short"), Some("Use at least 10 characters.".into()));
        assert_eq!(password_problem("correct horse", "correct hors"), Some("The two don't match.".into()));
        assert_eq!(password_problem("correct horse", "correct horse"), None);
    }
}
