//! The board's fields and what typing in them does (the listeners kanban.ts
//! puts on its inputs): Enter or leaving a field commits it, as there.

use super::*;

/// Which field has the keys.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::inbox) enum Field {
    Rename,
    NewList,
    Composer,
    Title,
    Desc,
    Item,
    Due,
}

impl Inbox {
    pub(super) fn board_inputs(window: &mut Window, cx: &mut Context<Self>) -> (Inputs, Vec<Subscription>) {
        let inputs = Inputs {
            rename: cx.new(|cx| InputState::new(window, cx)),
            new_list: cx.new(|cx| InputState::new(window, cx).placeholder("List title…")),
            composer: cx.new(|cx| TextareaState::new(window, cx).placeholder("+ Add a card").auto_grow(1, 8).submit_on_enter(true)),
            title: cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 6).submit_on_enter(true)),
            desc: cx.new(|cx| TextareaState::new(window, cx).placeholder("Add a more detailed description…").auto_grow(3, 12)),
            item: cx.new(|cx| InputState::new(window, cx).placeholder("Add an item…")),
            due: cx.new(|cx| InputState::new(window, cx).placeholder("yyyy-mm-dd")),
        };
        let subs = vec![
            watch(&inputs.rename, Field::Rename, window, cx),
            watch(&inputs.new_list, Field::NewList, window, cx),
            watch(&inputs.composer, Field::Composer, window, cx),
            watch(&inputs.title, Field::Title, window, cx),
            watch(&inputs.desc, Field::Desc, window, cx),
            watch(&inputs.item, Field::Item, window, cx),
            watch(&inputs.due, Field::Due, window, cx),
        ];
        (inputs, subs)
    }

    fn board_input(&mut self, field: Field, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        match event {
            InputEvent::Focus => b.focused = Some(field),
            InputEvent::Blur => {
                if b.focused == Some(field) {
                    b.focused = None;
                }
                self.board_commit(field, false, window, cx);
            }
            // Shift+Enter is a new line in the card box; the description
            // takes Enter as a new line too.
            InputEvent::PressEnter { secondary: false, shift: false } if field != Field::Desc => self.board_commit(field, true, window, cx),
            _ => return,
        }
        cx.notify();
    }

    /// A field done with: by Enter (`enter`) or by leaving it.
    fn board_commit(&mut self, field: Field, enter: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        let i = &b.inputs;
        match field {
            Field::Rename => {
                let Some(id) = b.renaming.take() else { return };
                let title = i.rename.read(cx).value().to_string();
                self.board_change(move |board| model::rename_list(board, &id, &title), window, cx);
                if enter {
                    self.board_blur(window, cx);
                }
            }
            Field::NewList => {
                if !std::mem::take(&mut b.adding_list) {
                    return;
                }
                let title = i.new_list.read(cx).value().to_string();
                i.new_list.update(cx, |t, cx| t.set_value("", window, cx));
                if !title.trim().is_empty() {
                    self.board_change(move |board| model::add_list(board, &title), window, cx);
                }
                if enter {
                    self.board_blur(window, cx);
                }
            }
            Field::Composer => {
                let text = i.composer.read(cx).value().to_string();
                if enter {
                    self.board_add_card(window, cx);
                } else if text.trim().is_empty() {
                    // Left empty: the box folds back to "+ Add a card".
                    b.composing = None;
                }
            }
            Field::Title | Field::Desc => {
                self.board_commit_detail(window, cx);
                if enter {
                    self.board_blur(window, cx);
                }
            }
            Field::Item => {
                let text = i.item.read(cx).value().trim().to_string();
                if !enter || text.is_empty() {
                    return;
                }
                i.item.update(cx, |t, cx| t.set_value("", window, cx));
                let Some(id) = b.open.clone() else { return };
                let item = model::ChecklistItem { id: model::uid("i"), text, done: false };
                self.board_change(
                    move |board| {
                        if let Some(c) = model::card_mut(board, &id) {
                            c.checklist.push(item.clone());
                        }
                    },
                    window,
                    cx,
                );
            }
            Field::Due => {
                let typed = i.due.read(cx).value().trim().to_string();
                let Some(id) = b.open.clone() else { return };
                let current = model::find_card(&b.board, &id).and_then(|(l, c)| b.board.lists[l].cards[c].due.clone());
                if typed.is_empty() || model::valid_due(&typed) {
                    let due = (!typed.is_empty()).then_some(typed);
                    if due != current {
                        self.board_set_due(id, due, window, cx);
                    }
                } else {
                    // Not a date: the field shows the card's again.
                    i.due.update(cx, |t, cx| t.set_value(current.unwrap_or_default(), window, cx));
                }
                if enter {
                    self.board_blur(window, cx);
                }
            }
        }
    }

    /// The card box's text becomes a card at the end of its list; the box
    /// folds back, as the board redrawn after addCard leaves it.
    pub(super) fn board_add_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        let text = b.inputs.composer.read(cx).value().to_string();
        let Some(list) = b.composing.clone() else { return };
        if text.trim().is_empty() {
            return;
        }
        b.composing = None;
        b.inputs.composer.update(cx, |t, cx| t.set_value("", window, cx));
        self.board_change(move |board| model::add_card(board, &list, &text), window, cx);
        self.board_blur(window, cx);
    }

    /// The open card's title and description, when they changed: an empty
    /// title keeps the old one.
    pub(super) fn board_commit_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &self.board else { return };
        let Some(id) = b.open.clone() else { return };
        let Some((l, c)) = model::find_card(&b.board, &id) else { return };
        let card = &b.board.lists[l].cards[c];
        let typed = b.inputs.title.read(cx).value().trim().to_string();
        if typed.is_empty() {
            let old = card.title.clone();
            b.inputs.title.update(cx, |t, cx| t.set_value(old, window, cx));
        }
        // Only what you changed: an agent's edit of the other one stays.
        let title = Some(typed).filter(|t| !t.is_empty() && *t != card.title);
        let desc = Some(b.inputs.desc.read(cx).value().to_string()).filter(|d| *d != card.desc);
        if title.is_none() && desc.is_none() {
            return;
        }
        self.board_change(
            move |board| {
                if let Some(c) = model::card_mut(board, &id) {
                    if let Some(t) = &title {
                        c.title = t.clone();
                    }
                    if let Some(d) = &desc {
                        c.desc = d.clone();
                    }
                }
            },
            window,
            cx,
        );
    }
}

fn watch<T: EventEmitter<InputEvent>>(input: &Entity<T>, field: Field, window: &mut Window, cx: &mut Context<Inbox>) -> Subscription {
    cx.subscribe_in(input, window, move |this, _, event: &InputEvent, window, cx| this.board_input(field, event, window, cx))
}
