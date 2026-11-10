//! The Company in the Inbox: Needs you, the job box, Jobs and the floors in 2D,
//! ported from src/company/office/rail.ts, model.ts and office.css. A person's
//! desk is their seat on the stage, with a crumb back here.

mod floors;
mod jobs;
mod look;
mod model;
mod rail;
mod rows;
mod source;
mod state;
mod view;
mod words;
#[cfg(test)]
mod tests;

// Named, not the Inbox's glob: its colours would clash with the Company's own (look.rs).
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::style::tip;
use super::{Inbox, chrome, logo};
use crate::fleet::now_ms;

pub(in crate::inbox) use state::CompanyUi;

/// What the page shows this frame: the office, or a line saying why there is none.
struct Snapshot {
    pub model: Option<model::OfficeModel>,
    /// Starting, or the Company runs in another window.
    pub note: Option<String>,
    /// The company could not be loaded.
    pub error: Option<String>,
}
