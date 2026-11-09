//! What the Changes buttons do: merge, send back, discard, and the line
//! comments that wait for Send back.

use crate::review::text::send_back_text;

use super::*;

impl Inbox {
    fn changes_say(&mut self, text: impl Into<String>, tone: Tone, cx: &mut Context<Self>) {
        if let Some(c) = &mut self.changes {
            c.msg = Some((text.into(), tone));
        }
        cx.notify();
    }

    /// The agent's run, stopped so Windows lets go of its folder. Whether it ran.
    fn stop_for_git(&mut self, index: usize, cx: &App) -> bool {
        let running = self.fleet.seats[index].terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
        if running {
            self.fleet.stop(index);
        }
        running
    }

    /// Merge its branch in and remove the worktree, once a dry run says the
    /// merge is clean. The agent stops first; if the merge fails it starts again.
    pub(super) fn merge_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.changes_seat() else { return };
        let Some(c) = self.changes.as_mut().filter(|c| c.own && !c.busy) else { return };
        let Some(wt) = self.fleet.seats[index].spec.worktree.clone() else { return };
        let title = self.fleet.seats[index].spec.title.clone();
        let no = c.no;
        c.busy = true;
        self.changes_say("Checking the merge…", Tone::Busy, cx);
        let dry = {
            let (wt, title) = (wt.clone(), title.clone());
            cx.background_spawn(async move { land::land(&wt, title.as_deref(), true, None) })
        };
        cx.spawn_in(window, async move |this, cx| {
            let dry = dry.await;
            let Ok(Some(base)) = this.update(cx, |this, cx| {
                if this.changes.as_ref().is_none_or(|c| c.no != no) {
                    return None;
                }
                match &dry {
                    LandResult::Ok { base, .. } => {
                        this.changes_say(format!("Merging into {base}…"), Tone::Busy, cx);
                        Some(base.clone())
                    }
                    _ => {
                        this.merge_failed(dry.clone(), cx);
                        None
                    }
                }
            }) else {
                return;
            };
            let Ok(ran) = this.update(cx, |this, cx| this.changes_seat().map(|i| this.stop_for_git(i, cx)).unwrap_or(false)) else { return };
            // Give the stopped process a moment to let go of its folder.
            cx.background_executor().timer(Duration::from_millis(400)).await;
            let real = cx.background_spawn(async move { land::land(&wt, title.as_deref(), false, Some(&base)) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(index) = this.changes_seat() else { return };
                match real {
                    LandResult::Ok { base, branch, removed, note } => {
                        let seat = &mut this.fleet.seats[index];
                        seat.spec.merged_into = Some(base.clone());
                        if removed {
                            seat.spec.worktree = None;
                            seat.spec.branch = None;
                            seat.branch = agent_core::git::current_branch(std::path::Path::new(seat.spec.run_dir()));
                        }
                        let id = seat.spec.id.clone();
                        this.history.log(&id, super::super::history::Event { at: now_ms(), state: State::Merged, text: format!("Merged {branch} into {base}") });
                        this.fleet.save();
                        this.kept.remove(&id);
                        this.close_changes(window, cx);
                        let tail = if removed { "Worktree removed.".to_string() } else { note.unwrap_or_default() };
                        this.toast(format!("Merged {branch} into {base}. {tail}").trim_end().to_string(), cx);
                    }
                    other => {
                        if ran {
                            this.start_seat(index, cx);
                        }
                        this.merge_failed(other, cx);
                    }
                }
            });
        })
        .detach();
    }

    fn merge_failed(&mut self, r: LandResult, cx: &mut Context<Self>) {
        let Some(c) = &mut self.changes else { return };
        c.busy = false;
        match r {
            LandResult::Conflict { base, branch, files } => {
                c.conflict = Some(Conflict { base: base.clone(), branch: branch.clone(), files: files.clone() });
                c.check = Some(LandResult::Conflict { base, branch, files });
                c.msg = None;
            }
            LandResult::Error { message } => c.msg = Some((message, Tone::Err)),
            LandResult::Ok { .. } => c.msg = None,
        }
        cx.notify();
    }

    /// Type the note, the line comments and what to resolve into the agent,
    /// starting it again first when it has stopped.
    pub(super) fn send_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.changes_seat() else { return };
        if self.changes.as_ref().is_none_or(|c| c.busy) {
            return;
        }
        // A comment still open in its box goes too.
        if self.changes.as_ref().is_some_and(|c| c.editing.is_some()) {
            self.save_comment(window, cx);
        }
        let Some(c) = &self.changes else { return };
        let id = c.id.clone();
        let note = c.note.read(cx).value().to_string();
        let comments = self.kept.get(&id).map(|k| k.comments.clone()).unwrap_or_default();
        let text = send_back_text(&note, &comments, c.conflict.as_ref());
        if text.trim().is_empty() {
            c.note.update(cx, |n, cx| {
                n.set_placeholder("Write what should change first, then press Send back.", window, cx);
                n.focus(window, cx);
            });
            return;
        }
        let name = self.fleet.seats[index].spec.name.clone();
        let stopped = self.fleet.seats[index].terminal.as_ref().is_none_or(|t| t.read(cx).exit_code().is_some());
        if stopped && !self.start_seat(index, cx) {
            self.changes_say(format!("{name} didn't start, so nothing was sent."), Tone::Err, cx);
            return;
        }
        // Typed in once it is at its prompt, as a job is.
        self.queue_job(id.clone(), text, cx);
        if let Some(k) = self.kept.get_mut(&id) {
            k.note.clear();
            k.viewed.clear();
            k.comments.retain(|c| !comments.iter().any(|s| s.id == c.id));
        }
        self.close_changes(window, cx);
        self.toast(format!("Sent back to {name}"), cx);
    }

    /// Throw the branch away, after asking: the worktree, its uncommitted work, the branch.
    pub(super) fn discard_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.changes_seat() else { return };
        let Some(c) = self.changes.as_mut().filter(|c| c.own && !c.busy) else { return };
        let Some(wt) = self.fleet.seats[index].spec.worktree.clone() else { return };
        let br = if c.branch.is_empty() { "its branch".to_string() } else { c.branch.clone() };
        c.discarding = false;
        c.busy = true;
        let no = c.no;
        self.changes_say("Discarding…", Tone::Busy, cx);
        let ran = self.stop_for_git(index, cx);
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(400)).await;
            let gone = cx.background_spawn(async move { land::discard_branch(&wt) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(index) = this.changes_seat().filter(|_| this.changes.as_ref().is_some_and(|c| c.no == no)) else { return };
                if let Err(e) = gone {
                    if ran {
                        this.start_seat(index, cx);
                    }
                    if let Some(c) = &mut this.changes {
                        c.busy = false;
                    }
                    this.changes_say(e, Tone::Err, cx);
                    return;
                }
                let seat = &mut this.fleet.seats[index];
                seat.spec.worktree = None;
                seat.spec.branch = None;
                seat.branch = agent_core::git::current_branch(std::path::Path::new(seat.spec.run_dir()));
                let (id, name) = (seat.spec.id.clone(), seat.spec.name.clone());
                this.kept.remove(&id);
                this.close_changes(window, cx);
                // Free again: a running agent starts over on a fresh conversation.
                if ran {
                    this.fleet.seats[index].spec.session_id = None;
                    this.start_seat(index, cx);
                }
                this.fleet.save();
                this.toast(format!("Discarded {br}. {name} is free."), cx);
            });
        })
        .detach();
    }

    /* ---------------- comments ---------------- */

    /// Open the box under a line; with Shift, for the run from the last line picked.
    pub(super) fn start_comment(&mut self, side: Side, num: u32, extend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &mut self.changes else { return };
        let from = match c.anchor {
            Some((s, n)) if extend && s == side => n,
            _ => num,
        };
        c.editing = Some(Editing { id: None, side, start: from.min(num), end: from.max(num) });
        if !extend {
            c.anchor = Some((side, num));
        }
        c.comment.update(cx, |box_, cx| {
            box_.set_value("", window, cx);
            box_.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn edit_comment(&mut self, cid: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &mut self.changes else { return };
        let Some(found) = self.kept.get(&c.id).and_then(|k| k.comments.iter().find(|x| x.id == cid)) else { return };
        c.editing = Some(Editing { id: Some(found.id.clone()), side: found.side, start: found.start, end: found.end });
        let text = found.text.clone();
        c.comment.update(cx, |box_, cx| {
            box_.set_value(text, window, cx);
            box_.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn drop_comment(&mut self, cid: &str, cx: &mut Context<Self>) {
        let Some(c) = &self.changes else { return };
        if let Some(k) = self.kept.get_mut(&c.id) {
            k.comments.retain(|x| x.id != cid);
        }
        cx.notify();
    }

    /// Keep the comment in its box; an emptied one goes.
    pub(super) fn save_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &mut self.changes else { return };
        let Some(e) = c.editing.take() else { return };
        let Some(file) = c.files.as_ref().and_then(|f| f.get(c.file)) else { return };
        let path = file.path.clone();
        // The lines it is about, quoted in what Send back types.
        let code: Vec<String> = file
            .hunks
            .iter()
            .flat_map(|h| crate::review::text::number_hunk(&h.header, &h.lines))
            .filter(|l| {
                let (side, num) = if l.kind == crate::review::text::Kind::Del { (Side::Old, l.old) } else { (Side::New, l.new) };
                side == e.side && num.is_some_and(|n| n >= e.start && n <= e.end)
            })
            .map(|l| l.text)
            .collect();
        let text = c.comment.read(cx).value().trim().to_string();
        c.focus.focus(window, cx);
        let kept = self.kept.entry(c.id.clone()).or_default();
        match (text.is_empty(), e.id) {
            (true, Some(id)) => kept.comments.retain(|x| x.id != id),
            (true, None) => {}
            (false, Some(id)) => {
                if let Some(x) = kept.comments.iter_mut().find(|x| x.id == id) {
                    x.text = text;
                }
            }
            (false, None) => kept.comments.push(LineComment { id: uuid::Uuid::new_v4().to_string(), path, side: e.side, start: e.start, end: e.end, code, text }),
        }
        cx.notify();
    }

    pub(in crate::inbox) fn cancel_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(c) = &mut self.changes {
            c.editing = None;
            c.focus.focus(window, cx);
        }
        cx.notify();
    }
}
