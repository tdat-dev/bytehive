//! What the board's buttons and drags do (the mutations in kanban.ts).

use super::*;

impl Inbox {
    pub(super) fn board_open_card(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        b.open = Some(id);
        b.picking = false;
        b.inputs.item.update(cx, |t, cx| t.set_value("", window, cx));
        self.board_sync_detail(window, cx);
        cx.notify();
    }

    /// "← Board".
    pub(super) fn board_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.board_commit_detail(window, cx);
        if let Some(b) = &mut self.board {
            b.open = None;
            b.picking = false;
        }
        cx.notify();
    }

    pub(super) fn board_start_rename(&mut self, list: String, title: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        b.renaming = Some(list);
        let input = b.inputs.rename.clone();
        input.update(cx, |t, cx| {
            t.set_value(title, window, cx);
            t.focus(window, cx);
            t.select_all(window, cx);
        });
        cx.notify();
    }

    pub(super) fn board_start_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        b.adding_list = true;
        let input = b.inputs.new_list.clone();
        input.update(cx, |t, cx| {
            t.set_value("", window, cx);
            t.focus(window, cx);
        });
        cx.notify();
    }

    /// Open a list's "+ Add a card" box (the one box moves to that list).
    pub(super) fn board_start_card(&mut self, list: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        if b.composing.as_ref() != Some(&list) {
            b.inputs.composer.update(cx, |t, cx| t.set_value("", window, cx));
        }
        b.composing = Some(list);
        let input = b.inputs.composer.clone();
        input.update(cx, |t, cx| t.focus(window, cx));
        cx.notify();
    }

    /// The list's delete: straight away when it is empty, else asked first.
    pub(super) fn board_delete_list(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        let empty = b.board.lists.iter().find(|l| l.id == id).is_none_or(|l| l.cards.is_empty());
        if empty {
            self.board_change(move |board| model::delete_list(board, &id), window, cx);
        } else {
            b.asking = Some(Ask::List(id));
            cx.notify();
        }
    }

    pub(super) fn board_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.board.as_mut().and_then(|b| b.asking.take()) else { return };
        match ask {
            Ask::List(id) => self.board_change(move |board| model::delete_list(board, &id), window, cx),
            Ask::Card(id) => {
                if let Some(b) = &mut self.board {
                    b.open = None;
                }
                self.board_change(move |board| model::delete_card(board, &id), window, cx);
            }
        }
        cx.notify();
    }

    /// Change the open card in place: labels, checklist, due.
    pub(super) fn board_edit_card(&mut self, f: impl Fn(&mut model::Card) + Send + Sync + 'static, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.board.as_ref().and_then(|b| b.open.clone()) else { return };
        self.board_change(
            move |board| {
                if let Some(c) = model::card_mut(board, &id) {
                    f(c);
                }
            },
            window,
            cx,
        );
    }

    pub(super) fn board_set_due(&mut self, id: String, due: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(b) = &self.board {
            b.inputs.due.update(cx, |t, cx| t.set_value(due.clone().unwrap_or_default(), window, cx));
        }
        self.board_change(
            move |board| {
                if let Some(c) = model::card_mut(board, &id) {
                    c.due = due.clone();
                }
            },
            window,
            cx,
        );
    }

    /// A card dropped on a list (at its end) or on a card (before it). Into
    /// Done by hand: what changed in the repo is kept on it (maybeFireDone).
    pub(super) fn board_drop_card(&mut self, id: String, to: String, before: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &self.board else { return };
        let Some((l, c)) = model::find_card(&b.board, &id) else { return };
        let was_done = b.board.lists[l].title.eq_ignore_ascii_case("done");
        let now_done = b.board.lists.iter().find(|x| x.id == to).is_some_and(|x| x.title.eq_ignore_ascii_case("done"));
        let had_evidence = b.board.lists[l].cards[c].done.is_some();
        let dir = b.dir.clone();
        let card = id.clone();
        self.board_change(move |board| model::move_card(board, &id, &to, before.as_deref()), window, cx);
        let (Some(dir), true) = (dir, now_done && !was_done && !had_evidence) else { return };
        let read = cx.background_spawn(async move {
            let root = crate::review::repos_under(&dir).into_iter().next().unwrap_or(dir);
            let files = agent_core::git::changed_files(std::path::Path::new(&root)).into_iter().map(|f| f.replace('/', "\\")).collect::<Vec<_>>();
            (root, files)
        });
        cx.spawn_in(window, async move |this, cx| {
            let (root, files) = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let done = model::DoneInfo { repo_root: root, files, summary: None, at: now_ms() as f64, by: None };
                this.board_change(
                    move |board| {
                        if let Some(c) = model::card_mut(board, &card) {
                            c.done = Some(done.clone());
                        }
                    },
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    pub(super) fn board_drop_list(&mut self, id: String, before: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.board_change(move |board| model::move_list(board, &id, before.as_deref()), window, cx);
    }

    /// The assignee chip: that agent on the stage.
    pub(super) fn board_focus_agent(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((index, ..)) = self.board_agents(cx).into_iter().find(|(_, n, _)| n == name) else { return };
        self.home = false;
        self.fleet.focus(index, window, cx);
        cx.notify();
    }

    /// "Send to agent…": the card typed into it, claimed and pulled into Doing.
    pub(super) fn board_send(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        b.picking = false;
        let Some(id) = b.open.clone() else { return };
        let Some((l, c)) = model::find_card(&b.board, &id) else { return };
        let prompt = model::dispatch_prompt(&b.board.lists[l].cards[c]);
        let Some(seat) = self.fleet.seats.get(index) else { return };
        let (seat_id, name) = (seat.spec.id.clone(), seat.spec.name.clone());
        self.queue_message(seat_id, prompt, cx);
        self.board_change(move |board| model::assign_card(board, &id, Some(&name), true), window, cx);
    }

    pub(super) fn board_unassign(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.board.as_ref().and_then(|b| b.open.clone()) else { return };
        self.board_change(move |board| model::assign_card(board, &id, None, false), window, cx);
    }
}
