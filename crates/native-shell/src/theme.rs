//! One palette for the whole shell: panels, text tiers, diff marks and
//! state badges. Every UI module reads these; nothing hardcodes a gray.

use eframe::egui::Color32;
use supervise::task_state::TaskState;

/// Panel and terminal background.
pub const BG: Color32 = Color32::from_rgb(24, 26, 31);
/// Header bars and raised rows.
pub const SURFACE: Color32 = Color32::from_rgb(32, 35, 42);
/// Borders and separators.
pub const BORDER: Color32 = Color32::from_rgb(52, 57, 68);
/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(220, 224, 232);
/// Secondary text: status lines, hints.
pub const DIM: Color32 = Color32::from_rgb(150, 158, 172);
/// Faint text: placeholders, capped markers.
pub const FAINT: Color32 = Color32::from_rgb(110, 116, 128);
/// Added diff lines, success marks.
pub const ADD: Color32 = Color32::from_rgb(126, 200, 138);
/// Removed diff lines, errors.
pub const DEL: Color32 = Color32::from_rgb(232, 122, 122);
/// Unchanged diff lines.
pub const CTX: Color32 = Color32::from_gray(140);
/// The agent's own words in history.
pub const YOU: Color32 = Color32::from_rgb(140, 190, 250);
/// The focused pane's ring, and the working badge dot.
pub const FOCUS: Color32 = Color32::from_rgb(100, 170, 250);

/// State badge dot per task state.
pub fn state_dot(state: TaskState) -> Color32 {
    match state {
        TaskState::Needs => Color32::from_rgb(250, 180, 60),
        TaskState::Blocked => DEL,
        TaskState::Review => ADD,
        TaskState::Working => FOCUS,
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
