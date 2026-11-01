//! One design system for the whole shell, ported from the web app's
//! `src/styles/tokens.css`: the same near-black ground, the same text tiers,
//! colour only for state (green working, blue ready to review, amber needs
//! you, red stop/deny). Every UI module reads these; nothing hardcodes a
//! color, radius, or size.
//!
//! Fonts: Geist and Geist Mono at the web app's weights, Windows' fonts
//! behind them for missing glyphs; see install_fonts().
//!
//! Some palette entries are reserved for surfaces still being ported
//! (Native 2/3); they mirror tokens.css 1:1 on purpose.
#![allow(dead_code)]

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

/// Floating sheets (New task): the design's rgba(20,21,24,.98).
pub const SHEET: Color32 = Color32::from_rgb(20, 21, 24);
/// Pills on a sheet: white 6% over it; hover is white 10%.
pub const PILL: Color32 = Color32::from_rgb(34, 35, 38);
pub const PILL_HOVER: Color32 = Color32::from_rgb(43, 44, 47);
/// The sheet design's third text tier (--ink-3): labels, hints, placeholders.
pub const INK3: Color32 = Color32::from_rgb(140, 140, 150);
/// Primary action: white pill, black words.
pub const PRIMARY: Color32 = Color32::from_rgb(255, 255, 255);
pub const PRIMARY_INK: Color32 = Color32::from_rgb(0, 0, 0);

/// Corner radii, same scale as the web app's --r1..r4.
pub const R1: f32 = 8.0;
pub const R2: f32 = 10.0;
pub const R3: f32 = 12.0;
pub const R4: f32 = 16.0;
/// Sheets and dialogs (--r-xl).
pub const R_XL: f32 = 18.0;

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

/// Windows' fonts that stand in for glyphs Geist lacks, as (name, file).
const SYSTEM: [(&str, &str); 4] = [
    ("Segoe UI", "segoeui.ttf"),
    ("Segoe UI Symbol", "seguisym.ttf"),
    ("Cascadia Mono", "CascadiaMono.ttf"),
    ("Consolas", "consola.ttf"),
];

/// Read the Windows fonts that are on this machine; their names.
fn system_fonts(fonts: &mut FontDefinitions) -> Vec<&'static str> {
    let dir = std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into())).join("Fonts");
    SYSTEM
        .iter()
        .filter_map(|(name, file)| {
            let bytes = std::fs::read(dir.join(file)).ok()?;
            fonts.font_data.insert(name.to_string(), FontData::from_owned(bytes).into());
            Some(*name)
        })
        .collect()
}

/// The web app's fonts: Geist and Geist Mono (it loaded them from Google
/// Fonts), at 400, 500 ("medium") and 600 ("semibold", also "bold"). What
/// Geist lacks (box drawing past its set, Claude's ⏺ ⎿ ✻, ✓ ⚠) falls back
/// to Windows' own fonts as the browser did, read at startup rather than
/// shipped. Body text is 13.5px. Call once.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let egui_back: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let bundled: [(&str, &'static [u8]); 5] = [
        ("Geist-Regular", include_bytes!("../assets/fonts/Geist-Regular.ttf")),
        ("Geist-Medium", include_bytes!("../assets/fonts/Geist-Medium.ttf")),
        ("Geist-SemiBold", include_bytes!("../assets/fonts/Geist-SemiBold.ttf")),
        ("GeistMono-Regular", include_bytes!("../assets/fonts/GeistMono-Regular.ttf")),
        ("GeistMono-Medium", include_bytes!("../assets/fonts/GeistMono-Medium.ttf")),
    ];
    for (name, bytes) in bundled {
        fonts.font_data.insert(name.into(), FontData::from_static(bytes).into());
    }
    let have = system_fonts(&mut fonts);
    let chain = |names: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = names.iter().filter(|n| n.starts_with("Geist") || have.contains(n)).map(|s| s.to_string()).collect();
        v.extend(egui_back.iter().cloned());
        v
    };
    let sans_back = ["Geist-Regular", "Segoe UI", "Segoe UI Symbol"];
    let with = |first: &str, back: &[&str]| chain(&[&[first][..], back].concat());
    let mono_back = ["GeistMono-Regular", "Cascadia Mono", "Consolas", "Segoe UI Symbol"];
    fonts.families.insert(FontFamily::Proportional, chain(&sans_back));
    fonts.families.insert(FontFamily::Name("medium".into()), with("Geist-Medium", &sans_back));
    fonts.families.insert(FontFamily::Name("semibold".into()), with("Geist-SemiBold", &sans_back));
    fonts.families.insert(FontFamily::Name("bold".into()), with("Geist-SemiBold", &sans_back));
    fonts.families.insert(FontFamily::Monospace, chain(&mono_back));
    fonts.families.insert(FontFamily::Name("monomedium".into()), with("GeistMono-Medium", &mono_back));
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::new(UI_BASE, FontFamily::Proportional));
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
