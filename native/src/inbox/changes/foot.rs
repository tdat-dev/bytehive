//! The foot of Changes (.rfoot): what is going on, the note for Send back,
//! and the ways to end the job. Also the comment box and "Discard branch?".

use gpui_kit::component::Sizable as _;

use crate::review::text::comment_where;

use super::diff::comment_card;
use super::view::{file_list, hbtn};
use super::*;

/// `.btn`: the foot's buttons, white for the main one, red words for Discard.
fn btn(id: &'static str, primary: bool, danger: bool, off: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(36.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .rounded_full()
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .when(primary, |b| b.bg(color(0xffffff)).text_color(color(0x000000)).when(!off, |b| b.hover(|b| b.bg(color(0xe9e9ec)))))
        .when(!primary, |b| b.bg(white(0.07)).text_color(color(if danger { DANGER } else { INK })).when(!off, |b| b.hover(|b| b.bg(white(0.12)))))
        .when(off, |b| b.opacity(0.45))
}

fn kbd(text: &'static str, on_white: bool) -> impl IntoElement {
    mono()
        .py(px(2.0))
        .px(px(5.0))
        .rounded(px(5.0))
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color(if on_white { 0x555555 } else { INK_3 }))
        .shadow(vec![ring(if on_white { hsla(0.0, 0.0, 0.0, 0.2) } else { white(0.09) })])
        .child(text)
}

/// The note fields' frame (.note).
fn field() -> Div {
    div().w_full().py(px(8.0)).px(px(12.0)).rounded(px(12.0)).bg(hsla(0.0, 0.0, 0.0, 0.25)).shadow(vec![ring(white(0.09))]).text_size(px(13.0)).line_height(relative(1.45))
}

impl Inbox {
    pub(super) fn changes_foot(&self, c: &Changes, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let into = if c.base.is_empty() { "main".to_string() } else { c.base.clone() };
        let line = match (&c.msg, &c.conflict, &c.check) {
            (Some((text, tone)), _, _) => Some((text.clone(), *tone)),
            (None, Some(k), _) => Some((format!("Merging into {} would conflict in {}. Nothing was merged. Send back asks {name} to merge {} in and resolve them.", k.base, file_list(&k.files), k.base), Tone::Plain)),
            (None, None, Some(LandResult::Error { message })) if c.own => Some((message.clone(), Tone::Plain)),
            (None, None, _) if !c.own => Some((format!("{name} works in the project folder, not on a branch of its own, so there is nothing to merge."), Tone::Plain)),
            _ => None,
        };
        let off = c.busy;
        let n = self.kept.get(&c.id).map_or(0, |k| k.comments.len());
        let back = btn("ir-back", false, false, off)
            .child("Send back")
            .when(n > 0, |b| b.child(mono().min_w(px(18.0)).h(px(18.0)).px(px(5.0)).flex().items_center().justify_center().rounded_full().bg(white(0.14)).text_size(px(11.0)).child(n.to_string())))
            .when(!off, |b| b.on_click(cx.listener(|this, _, window, cx| this.send_back(window, cx))));
        let mut row = div().flex().flex_wrap().gap(px(8.0));
        if c.own {
            row = row
                .child(btn("ir-merge", true, false, off).child(format!("Merge into {into}")).child(kbd("M", true)).when(!off, |b| b.on_click(cx.listener(|this, _, window, cx| this.merge_changes(window, cx)))))
                .child(back)
                .child(btn("ir-discard", false, true, off).child("Discard branch").when(!off, |b| {
                    b.on_click(cx.listener(|this, _, _, cx| {
                        if let Some(c) = &mut this.changes {
                            c.discarding = true;
                        }
                        cx.notify();
                    }))
                }));
        } else {
            row = row.child(back);
        }
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .pt(px(12.0))
            .px(px(18.0))
            .pb(px(18.0))
            .border_t_1()
            .border_color(hairline())
            .children(line.map(|(text, tone)| {
                let ink = match tone {
                    Tone::Err => DANGER,
                    Tone::Busy => INK_2,
                    Tone::Plain if c.conflict.is_some() && c.msg.is_none() => NEEDS,
                    Tone::Plain => INK_3,
                };
                div().text_size(px(12.5)).line_height(relative(1.5)).text_color(color(ink)).child(text)
            }))
            .child(field().child(Textarea::new(&c.note).appearance(false).with_size(px(13.0 / 0.875))))
            .child(row)
            .into_any_element()
    }

    /// The box a comment is written in, under the line (or last line of the run) it is about.
    pub(super) fn comment_box(&self, c: &Changes, path: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some(e) = &c.editing else { return div().into_any_element() };
        let editing = e.id.is_some();
        comment_card()
            .child(mono().mb(px(6.0)).text_size(px(12.0)).text_color(color(INK_3)).child(format!("Comment on {}", comment_where(path, e.side, e.start, e.end))))
            .child(field().min_h(px(64.0)).child(Textarea::new(&c.comment).appearance(false).with_size(px(13.0 / 0.875))))
            .child(
                div()
                    .mt(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().mr_auto().flex().items_center().gap(px(4.0)).text_size(px(12.0)).text_color(color(INK_3)).child(kbd("Ctrl Enter", false)).child("saves,").child(kbd("Esc", false)).child("cancels"))
                    .child(hbtn("rc-cancel", "Cancel").on_click(cx.listener(|this, _, window, cx| this.cancel_comment(window, cx))))
                    .child(btn("rc-save", true, false, false).h(px(30.0)).child(if editing { "Save" } else { "Comment" }).on_click(cx.listener(|this, _, window, cx| this.save_comment(window, cx)))),
            )
            .into_any_element()
    }

    /// "Discard <branch>?" before anything is deleted.
    pub(super) fn discard_confirm(&self, c: &Changes, name: &str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let br = if c.branch.is_empty() { "its branch".to_string() } else { c.branch.clone() };
        div().absolute().inset_0().bg(hsla(0.0, 0.0, 0.0, 0.55)).flex().items_center().justify_center().p(px(16.0)).child(
            div()
                .w(px(440.0))
                .max_w_full()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .p(px(22.0))
                .rounded(px(18.0))
                .bg(color(PANEL))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child(format!("Discard {br}?")))
                .child(div().text_size(px(14.0)).line_height(relative(1.5)).text_color(color(INK_2)).child(format!("{name}'s worktree and the branch {br} are deleted, with every change on them. This cannot be undone.")))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(btn("discard-cancel", false, false, false).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(c) = &mut this.changes {
                                c.discarding = false;
                            }
                            cx.notify();
                        })).child("Cancel"))
                        .child(btn("discard-ok", false, false, false).bg(color(DANGER)).text_color(color(GROUND)).on_click(cx.listener(|this, _, window, cx| this.discard_changes(window, cx))).child("Discard branch")),
                ),
        )
    }
}
