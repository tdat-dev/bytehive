//! The Changes drawer drawn: head, files, one file's diff (.inbox-review).

use crate::review::text::diff_totals;

use super::*;

/// "a.ts", "a.ts and b.ts", "a.ts, b.ts and 3 more".
pub(super) fn file_list(files: &[String]) -> String {
    match files {
        [] => "some files".to_string(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, rest @ ..] => format!("{a}, {b} and {} more", rest.len()),
    }
}

/// `.ir-close`: Close, at the top right of a drawer.
pub(in crate::inbox) fn close_button(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .tooltip(tip("Close (Esc)"))
        .flex_none()
        .h(px(28.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .rounded_full()
        .bg(white(0.06))
        .text_size(px(12.5))
        .text_color(color(INK_2))
        .hover(|b| b.bg(white(0.12)).text_color(color(INK)))
        .child("Close")
}

/// `.hbtn`: the small pill buttons.
pub(super) fn hbtn(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(30.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .rounded_full()
        .bg(white(0.05))
        .text_size(px(12.5))
        .text_color(color(INK_2))
        .whitespace_nowrap()
        .hover(|b| b.bg(white(0.1)).text_color(color(INK)))
        .child(label.into())
}

impl Inbox {
    pub(in crate::inbox) fn changes_view(&self, c: &Changes, cx: &mut Context<Self>) -> Option<AnyElement> {
        let index = self.changes_seat()?;
        let seat = &self.fleet.seats[index];
        let name = seat.spec.name.clone();
        let kept = self.kept.get(&c.id);
        let mut drawer = div()
            .id("changes")
            .track_focus(&c.focus)
            .absolute()
            .top(px(60.0))
            .right(px(16.0))
            .bottom(px(DOCK_H))
            .w(px(620.0))
            .max_w(relative(0.48))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(18.0))
            .bg(color(PANEL))
            .text_color(color(INK))
            .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.7), offset: point(px(0.0), px(30.0)), blur_radius: px(80.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                // M merges, as the button says; not while you type.
                let k = &event.keystroke;
                let on_drawer = this.changes.as_ref().is_some_and(|c| c.focus.is_focused(window));
                if !on_drawer {
                    return;
                }
                if this.diff_key(event, window, cx) {
                    cx.stop_propagation();
                } else if k.key == "m" && !k.modifiers.control && !k.modifiers.alt {
                    this.merge_changes(window, cx);
                }
            }))
            .child(self.changes_head(c, seat, cx));
        if let Some(into) = &seat.spec.merged_into {
            return Some(drawer.child(self.merged_body(into, &name, cx)).into_any_element());
        }
        match &c.files {
            None => drawer = drawer.child(empty(None, "Reading the changes…".to_string())),
            Some(files) if files.is_empty() => {
                let working = seat.state(cx) == State::Working;
                let why = if !c.error.is_empty() {
                    c.error.clone()
                } else if working {
                    format!("The diff lands here when {name} stops.")
                } else if c.own {
                    "No changes on this branch.".to_string()
                } else {
                    "No uncommitted changes in this folder.".to_string()
                };
                drawer = drawer.child(empty(Some(if working { "Still working" } else { "Nothing to review" }), why));
                if c.conflict.is_some() {
                    drawer = drawer.child(self.changes_foot(c, &name, cx));
                }
            }
            Some(files) => {
                drawer = drawer.child(files_bar(files, kept)).child(self.changes_files(c, files, kept, cx)).child(self.changes_diff(c, &files[c.file.min(files.len() - 1)], kept, cx)).child(self.changes_foot(c, &name, cx));
            }
        }
        if c.discarding {
            drawer = drawer.child(self.discard_confirm(c, &name, cx));
        }
        Some(drawer.into_any_element())
    }

    /// Merged: its work is in, its worktree gone. Clear it, or give it a new job.
    fn merged_body(&self, into: &str, name: &str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .p(px(24.0))
            .text_size(px(13.0))
            .text_color(color(INK_3))
            .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(format!("Merged into {into}")))
            .child(div().text_center().child(format!("Its worktree and branch are removed. Clear it from the list, or give {name} a new task.")))
            .child(hbtn("ir-clear", "Clear from list").on_click(cx.listener(|this, _, window, cx| {
                let Some(index) = this.changes_seat() else { return };
                this.changes = None;
                this.fleet.remove(index);
                if let Some(i) = this.fleet.focused {
                    this.fleet.focus(i, window, cx);
                }
                cx.notify();
            })))
    }

    fn changes_head(&self, c: &Changes, seat: &Seat, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let files = c.files.as_deref().unwrap_or(&[]);
        let (add, del) = diff_totals(files);
        let count = if c.files.is_none() { "Reading the changes…".to_string() } else { format!("{} file{} changed", files.len(), if files.len() == 1 { "" } else { "s" }) };
        let where_ = if c.own {
            let br = if c.branch.is_empty() { seat.spec.branch.clone().unwrap_or_default() } else { c.branch.clone() };
            if c.base.is_empty() { br } else { format!("{br} → {}", c.base) }
        } else {
            seat.spec.run_dir().to_string()
        };
        let chip = match (&c.check, &c.conflict) {
            (Some(LandResult::Ok { base, .. }), _) if c.own => Some((WORK, "✓", format!("Merges cleanly into {base}"))),
            (_, Some(k)) if c.own => Some((DANGER, "!", format!("Conflicts with {} in {} file{}", k.base, k.files.len(), if k.files.len() == 1 { "" } else { "s" }))),
            _ => None,
        };
        div()
            .flex_none()
            .pt(px(18.0))
            .px(px(18.0))
            .pb(px(12.0))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().mb(px(4.0)).text_size(px(12.0)).text_color(color(INK_3)).child(if c.own { "Changes on this branch" } else { "Changes in the project folder" }))
                            .child(
                                div()
                                    .mb(px(4.0))
                                    .flex()
                                    .flex_wrap()
                                    .items_baseline()
                                    .gap(px(8.0))
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(count)
                                    .when(!files.is_empty(), |h| h.child(mono().text_size(px(13.0)).text_color(color(WORK)).child(format!("+{add}"))).child(mono().text_size(px(13.0)).text_color(color(DANGER)).child(format!("−{del}")))),
                            )
                            .when(!where_.is_empty(), |h| h.child(mono().truncate().text_size(px(12.0)).text_color(color(INK_3)).child(where_))),
                    )
                    .child(hbtn("ir-close", "Close").mt(px(-4.0)).tooltip(tip("Close (Esc)")).on_click(cx.listener(|this, _, window, cx| this.close_changes(window, cx)))),
            )
            .children(chip.map(|(tone, mark, text)| {
                div().mt(px(12.0)).flex().child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .py(px(3.0))
                        .px(px(9.0))
                        .rounded_full()
                        .bg(white(0.05))
                        .text_size(px(12.0))
                        .text_color(color(INK_2))
                        .child(div().text_color(color(tone)).child(mark))
                        .child(text),
                )
            }))
    }

    fn changes_files(&self, c: &Changes, files: &[DiffFile], kept: Option<&Kept>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rows: Vec<AnyElement> = files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let cur = i == c.file;
                let seen = kept.is_some_and(|k| k.viewed.contains(&f.path));
                let path = f.path.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .pr(px(8.0))
                    .rounded(px(10.0))
                    .when(cur, |r| r.bg(color(RAISE)))
                    .when(!cur, |r| r.hover(|r| r.bg(white(0.03))))
                    .child(
                        div()
                            .id(("ir-pick", i))
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .py(px(7.0))
                            .px(px(8.0))
                            .text_size(px(12.5))
                            .font_family("Geist Mono")
                            .text_color(color(if cur { INK } else { INK_2 }))
                            .tooltip(tip(f.path.clone()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(c) = &mut this.changes {
                                    c.file = i;
                                    c.show_all = false;
                                    c.editing = None;
                                    c.anchor = None;
                                    c.line = None;
                                }
                                cx.notify();
                            }))
                            .child(div().flex_1().min_w_0().truncate().when(seen, |p| p.text_color(color(INK_3)).line_through()).child(f.path.clone()))
                            .child(div().flex_none().text_color(color(WORK)).child(format!("+{}", f.additions)))
                            .child(div().flex_none().text_color(color(DANGER)).child(format!("−{}", f.deletions))),
                    )
                    .child(
                        div()
                            .id(("ir-seen", i))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .text_size(px(11.5))
                            .text_color(color(INK_3))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let Some(id) = this.changes.as_ref().map(|c| c.id.clone()) else { return };
                                let viewed = &mut this.kept.entry(id).or_default().viewed;
                                if !viewed.remove(&path) {
                                    viewed.insert(path.clone());
                                }
                                cx.notify();
                            }))
                            .child(tick_box(seen))
                            .child("Viewed"),
                    )
                    .into_any_element()
            })
            .collect();
        div().id("ir-files").flex_none().min_h(px(64.0)).max_h(relative(0.3)).overflow_y_scroll().px(px(10.0)).pb(px(8.0)).children(rows)
    }
}

/// `.rempty`: nothing to show yet, or at all.
fn empty(title: Option<&str>, text: String) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .p(px(24.0))
        .text_size(px(13.0))
        .text_color(color(INK_3))
        .children(title.map(|t| div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(t.to_string())))
        .child(div().text_center().child(text))
}

fn files_bar(files: &[DiffFile], kept: Option<&Kept>) -> impl IntoElement {
    let n = files.iter().filter(|f| kept.is_some_and(|k| k.viewed.contains(&f.path))).count();
    let share = if files.is_empty() { 0.0 } else { n as f32 / files.len() as f32 };
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .py(px(6.0))
        .px(px(18.0))
        .text_size(px(12.0))
        .text_color(color(INK_3))
        .child(div().flex().gap(px(4.0)).child(mono().child(format!("{n} / {}", files.len()))).child("files viewed"))
        .child(div().w(px(90.0)).h(px(3.0)).rounded(px(2.0)).bg(white(0.07)).overflow_hidden().child(div().h_full().w(relative(share)).bg(color(REVIEW))))
}

fn tick_box(on: bool) -> impl IntoElement {
    div()
        .size(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.0))
        .when(on, |b| b.bg(color(REVIEW)).text_color(color(GROUND)).text_size(px(10.0)).child("✓"))
        .when(!on, |b| b.shadow(vec![ring(white(0.25))]))
}
