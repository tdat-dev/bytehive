//! How many files each agent changed (changesFor in tasks.ts), which turns a
//! finished agent's row into "ready to review". Only an agent with its own
//! worktree can be pinned a diff: agents sharing a checkout share one, so
//! theirs stays unknown. A stopped agent's tree does not change by itself:
//! it is looked at once (first sight, or once more right after it stops).

use super::*;

/// How often each agent's worktree is asked for changed files.
const DIFF_EVERY_MS: u64 = 5000;

impl Inbox {
    /// Called from the tick: ask the agents that are due, off the UI thread.
    pub(super) fn poll_changes(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        for seat in &mut self.fleet.seats {
            let Some(wt) = seat.spec.worktree.clone() else {
                seat.changes.count = None;
                continue;
            };
            let running = seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
            let c = &mut seat.changes;
            let due = running || c.at == 0 || c.was_running;
            if !due || c.pending || now.saturating_sub(c.at) < DIFF_EVERY_MS {
                continue;
            }
            c.pending = true;
            c.was_running = running;
            let id = seat.spec.id.clone();
            let read = cx.background_spawn(async move {
                let path = std::path::Path::new(&wt);
                path.is_dir().then(|| agent_core::git::changed_files(path).len() as u32)
            });
            cx.spawn(async move |this, cx| {
                let n = read.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(seat) = this.fleet.seats.iter_mut().find(|s| s.spec.id == id) {
                        let moved = seat.changes.count != n;
                        seat.changes.count = n;
                        seat.changes.at = now_ms();
                        seat.changes.pending = false;
                        if moved {
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
    }
}
