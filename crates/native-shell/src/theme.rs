//! One design system for the whole shell, ported from the web app's
//! `src/styles/tokens.css`: the same near-black ground, the same text tiers,
//! colour only for state (green working, blue ready to review, amber needs
//! you, red stop/deny). Every UI module reads these; nothing hardcodes a
//! color, radius, or size.
//!
//! Fonts (also mirroring the web app, which loads Geist + Geist Mono):
//! install_fonts() embeds the vendored TTFs (assets/fonts) and makes them
//! the Proportional/Monospace families plus named weights for strong text.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily};
use supervise::task_state::TaskState;

/// Cinema ground: the app background.
pub const BG: Color32 = Color32::from_rgb(5, 5, 6);
/// Sunken wells (terminal bodies, inset lists).
pub const WELL: Color32 = Color32::from_rgb(10, 10, 12);
/// Header bars and raised rows.
pub const SURFACE: Color32 = Color32::from_rgb(21, 22, 26);
/// Raised cards on top of the surface (attention cards, popovers).
pub const SURFACE_HIGH: Color32 = Color32::from_rgb(29, 30, 35);
/// Borders and separators: white 7% over the ground.
pub const BORDER: Color32 = Color32::from_rgb(23, 23, 24);
/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(244, 244, 245);
/// Secondary text: status lines, hints.
pub const DIM: Color32 = Color32::from_rgb(180, 180, 188);
/// Faint text: placeholders, capped markers.
pub const FAINT: Color32 = Color32::from_rgb(92, 92, 102);
/// Added diff lines, success marks, working dots.
pub const ADD: Color32 = Color32::from_rgb(111, 214, 160);
/// Removed diff lines, errors, blocked/stopped dots.
pub const DEL: Color32 = Color32::from_rgb(242, 118, 107);
/// Unchanged diff lines.
pub const CTX: Color32 = Color32::from_gray(140);
/// The agent's own words in history.
pub const YOU: Color32 = Color32::from_rgb(154, 184, 238);
/// Ready-to-review dots.
pub const FOCUS: Color32 = Color32::from_rgb(154, 184, 238);
/// Needs-you dots.
pub const AMBER: Color32 = Color32::from_rgb(240, 191, 106);

/// Corner radii, same scale as the web app's --r1..r4.
pub const R1: f32 = 8.0;
pub const R2: f32 = 10.0;
pub const R3: f32 = 12.0;
pub const R4: f32 = 16.0;

/// UI chrome text: the web app's 13.5px body.
pub const UI_BASE: f32 = 13.5;
/// Panel titles ("Queue", "Review"...): the web app's strong 15px.
pub const PANEL_TITLE: f32 = 15.0;
/// Grid header labels.
pub const GRID_TITLE: f32 = 12.0;
/// Terminal glyphs: the web app's default terminal size.
pub const TERM_SIZE: f32 = 15.0;

/// State badge dot per task state.
pub fn state_dot(state: TaskState) -> Color32 {    match state {
        TaskState::Needs => AMBER,
        TaskState::Blocked => DEL,
        TaskState::Review => FOCUS,
        TaskState::Working => ADD,
        TaskState::Idle => DIM,
        TaskState::Stopped => Color32::from_gray(90),
        TaskState::Merged => Color32::from_rgb(150, 130, 220),
    }
}

/// Short word per task state, for the grid header badge.
pub fn state_word(state: TaskState) -> &'static str {
    match state {
        TaskState::Needs => "needs you",
        TaskState::Blocked => "blocked",
        TaskState::Review => "review",
        TaskState::Working => "working",
        TaskState::Idle => "idle",
        TaskState::Stopped => "stopped",
        TaskState::Merged => "merged",
    }
}

fn font(name: &str, bytes: &'static [u8]) -> (String, FontData) {
    (name.to_string(), FontData::from_static(bytes))
}

/// Make Geist (+ Geist Mono) the UI fonts and match the web app's 13.5px
/// body. Call once at startup; egui keeps the fonts after this returns.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let files = [
        font("Geist-Regular", include_bytes!("../assets/fonts/Geist-Regular.ttf")),
        font("Geist-Medium", include_bytes!("../assets/fonts/Geist-Medium.ttf")),
        font("Geist-Bold", include_bytes!("../assets/fonts/Geist-Bold.ttf")),
        font("GeistMono-Regular", include_bytes!("../assets/fonts/GeistMono-Regular.ttf")),
        font("GeistMono-Medium", include_bytes!("../assets/fonts/GeistMono-Medium.ttf")),
    ];
    for (name, data) in files {
        fonts.font_data.insert(name, data.into());
    }
    fonts.families.insert(FontFamily::Proportional, vec!["Geist-Regular".to_string()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["GeistMono-Regular".to_string()]);
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        vec!["Geist-Medium".to_string(), "Geist-Regular".to_string()],
    );
    fonts.families.insert(
        FontFamily::Name("bold".into()),
        vec!["Geist-Bold".to_string(), "Geist-Regular".to_string()],
    );
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(UI_BASE, FontFamily::Proportional),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dots read straight off the web app's tokens: green working, blue
    /// ready to review, amber needs you, red stop/deny.
    #[test]
    fn state_dots_match_web_tokens() {
        let hex = |c: Color32| format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
        assert_eq!(hex(state_dot(TaskState::Working)), "6fd6a0");
        assert_eq!(hex(state_dot(TaskState::Review)), "9ab8ee");
        assert_eq!(hex(state_dot(TaskState::Needs)), "f0bf6a");
        assert_eq!(hex(state_dot(TaskState::Blocked)), "f2766b");
        assert_eq!(hex(BG), "050506");
        assert_eq!(hex(TEXT), "f4f4f5");
    }
}
