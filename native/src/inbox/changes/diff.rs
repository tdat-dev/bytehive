//! One file's diff in Changes: its lines, a number that takes a comment,
//! and the comments under their lines.

use crate::review::text::{Kind, comment_where, number_hunk};

use super::view::hbtn;
use super::*;

impl Inbox {
    pub(super) fn changes_diff(&self, c: &Changes, f: &DiffFile, kept: Option<&Kept>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mine: Vec<&LineComment> = kept.map(|k| k.comments.iter().filter(|x| x.path == f.path).collect()).unwrap_or_default();
        let mut out: Vec<AnyElement> = Vec::new();
        if f.binary {
            out.push(div().py(px(10.0)).px(px(18.0)).text_color(color(INK_3)).child("Binary file, not shown.").into_any_element());
        }
        let mut drawn = 0usize;
        'hunks: for h in &f.hunks {
            if !c.show_all && drawn >= MAX_LINES {
                break;
            }
            out.push(div().py(px(5.0)).px(px(18.0)).truncate().text_color(color(INK_3)).child(h.header.clone()).into_any_element());
            for l in number_hunk(&h.header, &h.lines) {
                if !c.show_all && drawn >= MAX_LINES {
                    break 'hunks;
                }
                drawn += 1;
                // A removed line is commented by its old number, every other by its new one.
                let side = if l.kind == Kind::Del { Side::Old } else { Side::New };
                let num = if side == Side::Old { l.old } else { l.new }.unwrap_or(0);
                let on = mine.iter().any(|x| x.side == side && num >= x.start && num <= x.end);
                out.push(diff_line(l.kind, l.new, &l.text, on, cx.listener(move |this, e: &MouseDownEvent, window, cx| this.start_comment(side, num, e.modifiers.shift, window, cx))));
                for x in mine.iter().filter(|x| x.side == side && x.end == num) {
                    let editing_this = c.editing.as_ref().is_some_and(|e| e.id.as_deref() == Some(x.id.as_str()));
                    out.push(if editing_this { self.comment_box(c, &f.path, cx) } else { comment_note(x, cx) });
                }
                if c.editing.as_ref().is_some_and(|e| e.id.is_none() && e.side == side && e.end == num) {
                    out.push(self.comment_box(c, &f.path, cx));
                }
            }
        }
        let total: usize = f.hunks.iter().map(|h| h.lines.len()).sum();
        if !c.show_all && total > MAX_LINES {
            out.push(
                div()
                    .m(px(10.0))
                    .mx(px(18.0))
                    .flex()
                    .child(hbtn("ir-more", format!("Show all {total} lines")).on_click(cx.listener(|this, _, _, cx| {
                        if let Some(c) = &mut this.changes {
                            c.show_all = true;
                        }
                        cx.notify();
                    })))
                    .into_any_element(),
            );
        }
        div()
            .id("ir-diff")
            .flex_1()
            .min_h(px(96.0))
            .overflow_scroll()
            .py(px(6.0))
            .border_t_1()
            .border_color(hairline())
            .font_family("Geist Mono")
            .text_size(px(12.5))
            .line_height(relative(1.65))
            .child(div().flex().flex_col().items_start().min_w_full().children(out))
    }
}

/// One diff line: its number takes a comment (a + shows on hover), then the code.
fn diff_line(kind: Kind, new: Option<u32>, text: &str, commented: bool, on_pick: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> AnyElement {
    let (bg, ink, sign) = match kind {
        Kind::Add => (Some(tint(WORK, 0.1)), color(0xc5ecd3), "+"),
        Kind::Del => (Some(tint(DANGER, 0.1)), color(0xf6d0ca), "-"),
        Kind::Ctx => (None, color(INK_2), " "),
    };
    div()
        .group("dl")
        .flex()
        .min_w_full()
        .pr(px(18.0))
        .when_some(bg, |r, bg| r.bg(bg))
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(44.0))
                .pr(px(10.0))
                .flex()
                .justify_end()
                .cursor_pointer()
                .text_color(color(if commented { REVIEW } else { INK_3 }))
                .when(commented, |n| n.bg(tint(REVIEW, 0.14)))
                .group_hover("dl", |n| n.text_color(color(INK)))
                .on_mouse_down(MouseButton::Left, on_pick)
                .child(div().absolute().left(px(7.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(REVIEW)).invisible().group_hover("dl", |p| p.visible()).child("+"))
                .child(new.map(|n| n.to_string()).unwrap_or_default()),
        )
        .child(div().flex_none().whitespace_nowrap().text_color(ink).child(format!("{sign}{}", text.replace('\t', "    "))))
        .into_any_element()
}

/// A saved comment under its last line: where, what you said, Edit and Remove.
fn comment_note(x: &LineComment, cx: &mut Context<Inbox>) -> AnyElement {
    let (edit, del) = (x.id.clone(), x.id.clone());
    comment_card()
        .child(mono().mb(px(6.0)).text_size(px(12.0)).text_color(color(INK_3)).child(comment_where(&x.path, x.side, x.start, x.end)))
        .child(div().text_color(color(INK)).child(x.text.clone()))
        .child(
            div()
                .mt(px(8.0))
                .flex()
                .justify_end()
                .gap(px(6.0))
                .child(hbtn(SharedString::from(format!("rc-edit-{}", x.id)), "Edit").on_click(cx.listener(move |this, _, window, cx| this.edit_comment(&edit, window, cx))))
                .child(hbtn(SharedString::from(format!("rc-del-{}", x.id)), "Remove").on_click(cx.listener(move |this, _, _, cx| this.drop_comment(&del, cx)))),
        )
        .into_any_element()
}

/// `.rc-box, .rc-note`: a card under the line, in the reading font.
pub(super) fn comment_card() -> Div {
    div()
        .mt(px(6.0))
        .mb(px(8.0))
        .ml(px(44.0))
        .mr(px(18.0))
        .w(px(480.0))
        .py(px(10.0))
        .px(px(12.0))
        .rounded(px(12.0))
        .bg(color(RAISE))
        .shadow(vec![ring(white(0.09))])
        .font_family("Geist")
        .text_size(px(13.0))
        .line_height(relative(1.5))
        .whitespace_normal()
}
