//! Review of an agent's work (land.rs, review.rs, diff.ts, reviewtext.ts in
//! today's app): read its diff, comment on it, send it back, merge it into
//! the project folder's branch or throw it away. Everything here blocks, so
//! call it from a background thread.

#![allow(dead_code, unused_imports)]

pub mod diff;
mod git;
pub mod land;
pub mod text;

pub use diff::{parse_diff, DiffFile, Hunk};
pub use land::{branch_diff, discard_branch, land, repo_diff, repos_under, LandResult};
pub use text::{comment_where, diff_totals, number_hunk, send_back_text, Conflict, Kind, LineComment, NumberedLine, Side};
