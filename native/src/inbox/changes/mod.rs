//! Changes (inboxreview.ts, .inbox-review in inbox.css): what an agent
//! changed, and the three ways to end its job. Merge takes its own branch
//! into the project's and removes its worktree; Send back types your note and
//! line comments into it; Discard branch throws the branch away. An agent in
//! the project folder itself has no branch: it gets the diff and Send back.

mod act;
mod diff;
mod foot;
mod view;

use crate::review::diff::DiffFile;
use crate::review::land::{self, LandResult};
use crate::review::text::{Conflict, LineComment, Side};

use super::*;

/// Lines drawn for one file before the rest waits behind "Show all".
const MAX_LINES: usize = 1500;

/// Kept per agent while the app runs: the note being written, the files
/// ticked as viewed and the line comments, so closing loses none of them.
#[derive(Default)]
pub(super) struct Kept {
    pub note: String,
    pub viewed: HashSet<String>,
    pub comments: Vec<LineComment>,
}

/// The comment being written: the lines it is about. `id` when it edits one.
pub(in crate::inbox) struct Editing {
    id: Option<String>,
    side: Side,
    start: u32,
    end: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Plain,
    Busy,
    Err,
}

pub(super) struct Changes {
    /// The agent's id: its place in the list can move while this is open.
    pub id: String,
    files: Option<Vec<DiffFile>>,
    error: String,
    file: usize,
    show_all: bool,
    /// It has a worktree and branch of its own: Merge and Discard apply.
    own: bool,
    base: String,
    branch: String,
    /// The dry run's answer.
    check: Option<LandResult>,
    conflict: Option<Conflict>,
    busy: bool,
    msg: Option<(String, Tone)>,
    pub(in crate::inbox) editing: Option<Editing>,
    /// Where a Shift-click run of lines starts.
    anchor: Option<(Side, u32)>,
    /// "Discard branch?" waiting for an answer.
    pub(in crate::inbox) discarding: bool,
    note: Entity<TextareaState>,
    comment: Entity<TextareaState>,
    /// Bumped on every open, so a slow read for another agent is dropped.
    no: u64,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Inbox {
    /// Open Changes for the agent at `index`.
    pub(super) fn open_changes(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(seat) = self.fleet.seats.get(index) else { return };
        let id = seat.spec.id.clone();
        let name = seat.spec.name.clone();
        let worktree = seat.spec.worktree.clone();
        let dir = seat.spec.run_dir().to_string();
        let title = seat.spec.title.clone();
        self.palette = None;
        self.menu = None;
        self.popup = None;
        crate::keys::set_active(false);
        let kept_note = self.kept.get(&id).map(|k| k.note.clone()).unwrap_or_default();
        let note = cx.new(|cx| TextareaState::new(window, cx).placeholder(format!("What should {name} change? Needed only for Send back.")).auto_grow(2, 5).default_value(kept_note));
        let comment = cx.new(|cx| TextareaState::new(window, cx).placeholder("What should change here?").auto_grow(3, 8));
        let keep_id = id.clone();
        let subs = vec![
            cx.subscribe(&note, move |this, note, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let text = note.read(cx).value().to_string();
                    this.kept.entry(keep_id.clone()).or_default().note = text;
                }
            }),
            // Ctrl+Enter keeps the comment.
            cx.subscribe_in(&comment, window, |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: true, .. } = event {
                    this.save_comment(window, cx);
                }
            }),
        ];
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let no = self.changes.as_ref().map_or(0, |c| c.no) + 1;
        let branch = seat.spec.branch.clone().unwrap_or_default();
        self.changes = Some(Changes {
            id,
            files: None,
            error: String::new(),
            file: 0,
            show_all: false,
            own: worktree.is_some(),
            base: String::new(),
            branch,
            check: None,
            conflict: None,
            busy: false,
            msg: None,
            editing: None,
            anchor: None,
            discarding: false,
            note,
            comment,
            no,
            focus,
            _subs: subs,
        });
        let read = cx.background_spawn(async move { read_changes(worktree.as_deref(), &dir, title.as_deref()) });
        cx.spawn(async move |this, cx| {
            let got = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(c) = this.changes.as_mut().filter(|c| c.no == no) else { return };
                c.files = Some(crate::review::diff::parse_diff(&got.diff));
                c.error = got.error;
                c.own &= got.own;
                if let Some(LandResult::Ok { base, branch, .. } | LandResult::Conflict { base, branch, .. }) = &got.check {
                    c.base = base.clone();
                    c.branch = branch.clone();
                }
                if let Some(LandResult::Conflict { base, branch, files }) = &got.check {
                    c.conflict = Some(Conflict { base: base.clone(), branch: branch.clone(), files: files.clone() });
                }
                c.check = got.check;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn close_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.changes.take().map(|c| c.id);
        if let Some(index) = id.and_then(|id| self.fleet.seats.iter().position(|s| s.spec.id == id)) {
            if !self.home && self.settings.is_none() && self.sessions.is_none() {
                self.fleet.focus(index, window, cx);
            }
        }
        cx.notify();
    }

    /// The agent Changes is about, by its place in the list now.
    fn changes_seat(&self) -> Option<usize> {
        let id = &self.changes.as_ref()?.id;
        self.fleet.seats.iter().position(|s| &s.spec.id == id)
    }
}

/// What a read of an agent's changes found.
struct Read {
    diff: String,
    error: String,
    own: bool,
    check: Option<LandResult>,
}

/// Its branch's whole diff and a dry run of the merge when it has a branch,
/// else its folder's uncommitted changes.
fn read_changes(worktree: Option<&str>, dir: &str, title: Option<&str>) -> Read {
    if let Some(wt) = worktree {
        let check = land::land(wt, title, true, None);
        match land::branch_diff(wt) {
            Some(diff) => return Read { diff, error: String::new(), own: true, check: Some(check) },
            // No branch of its own after all: its folder's plain diff.
            None => {
                return match land::repo_diff(wt) {
                    Ok(diff) => Read { diff, error: String::new(), own: false, check: Some(check) },
                    Err(e) => Read { diff: String::new(), error: format!("Couldn't read the changes: {e}"), own: false, check: Some(check) },
                };
            }
        }
    }
    let Some(repo) = land::repos_under(dir).into_iter().next() else {
        let error = if dir.is_empty() { "This agent has no folder." } else { "This folder is not a git repository, so there is no diff to show." };
        return Read { diff: String::new(), error: error.to_string(), own: false, check: None };
    };
    match land::repo_diff(&repo) {
        Ok(diff) => Read { diff, error: String::new(), own: false, check: None },
        Err(e) => Read { diff: String::new(), error: format!("Couldn't read the changes: {e}"), own: false, check: None },
    }
}
