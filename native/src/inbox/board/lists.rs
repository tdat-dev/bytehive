//! The board itself (.kb-board): lists side by side, each with its head,
//! its cards and its "+ Add a card" box, then "+ Add a list".

use gpui_kit::component::Sizable as _;

use super::look::{CardDrag, Ghost, LIST_W, ListDrag, MUTED_2, SURFACE_3, accent_dim, add_btn, field_box, glow, line, line_strong};
use super::*;

impl Inbox {
    pub(super) fn board_lists(&self, b: &BoardView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let lists: Vec<AnyElement> = b.board.lists.iter().enumerate().map(|(i, list)| self.board_list(b, i, list, cx)).collect();
        div()
            .id("kb-board")
            .flex_1()
            .min_h_0()
            .flex()
            .items_start()
            .gap(px(10.0))
            .p(px(12.0))
            .overflow_x_scroll()
            .children(lists)
            .child(self.board_add_list(b, cx))
    }

    fn board_list(&self, b: &BoardView, i: usize, list: &model::List, cx: &mut Context<Self>) -> AnyElement {
        let id = list.id.clone();
        let cards: Vec<AnyElement> = list.cards.iter().map(|c| self.board_card(&list.id, c, cx)).collect();
        let (drop_card, drop_list) = (id.clone(), id.clone());
        div()
            .id(("kb-list", i))
            .flex_none()
            .w(px(LIST_W))
            .max_h(relative(1.0))
            .flex()
            .flex_col()
            .min_h_0()
            .bg(color(PANEL))
            .border_1()
            .border_color(line())
            .rounded(px(12.0))
            .drag_over::<ListDrag>(|s, _, _, _| s.border_color(accent_dim()))
            .on_drop(cx.listener(move |this, d: &CardDrag, window, cx| this.board_drop_card(d.id.clone(), drop_card.clone(), None, window, cx)))
            .on_drop(cx.listener(move |this, d: &ListDrag, window, cx| this.board_drop_list(d.id.clone(), Some(drop_list.clone()), window, cx)))
            .child(self.board_list_head(b, i, list, cx))
            .child(div().id(("kb-cards", i)).flex_shrink(1.0).min_h(px(44.0)).overflow_y_scroll().py(px(4.0)).px(px(8.0)).flex().flex_col().gap(px(7.0)).children(cards))
            .child(self.board_composer(b, &id, cx))
            .into_any_element()
    }

    /// .kb-list-h: title (click to rename), count, delete on hover; the
    /// head is what a list is dragged by.
    fn board_list_head(&self, b: &BoardView, i: usize, list: &model::List, cx: &mut Context<Self>) -> AnyElement {
        let group: SharedString = format!("kb-h-{i}").into();
        let renaming = b.renaming.as_ref() == Some(&list.id);
        let (rename_id, rename_title, delete_id) = (list.id.clone(), list.title.clone(), list.id.clone());
        let title: AnyElement = if renaming {
            field_box(true, accent_dim(), false)
                .flex_1()
                .min_w_0()
                .py(px(3.0))
                .px(px(7.0))
                .rounded(px(8.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(Input::new(&b.inputs.rename).appearance(false).with_size(px(12.5 / 0.875)))
                .into_any_element()
        } else {
            div()
                .id(("kb-list-t", i))
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color(INK))
                .cursor_text()
                .tooltip(tip("Rename list"))
                .on_click(cx.listener(move |this, _, window, cx| this.board_start_rename(rename_id.clone(), rename_title.clone(), window, cx)))
                .child(list.title.clone())
                .into_any_element()
        };
        let drag = ListDrag { id: list.id.clone(), title: list.title.clone().into() };
        div()
            .id(("kb-list-h", i))
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(8.0))
            .pt(px(9.0))
            .pb(px(9.0))
            .pl(px(12.0))
            .pr(px(8.0))
            .cursor_grab()
            .when(!renaming, |h| h.on_drag(drag, |d, _, _, cx| cx.new(|_| Ghost { title: d.title.clone(), list: true })))
            .child(title)
            .child(mono().flex_none().py(px(1.0)).px(px(6.0)).rounded(px(4.0)).bg(color(SURFACE_3)).text_size(px(9.5)).text_color(color(MUTED_2)).child(list.cards.len().to_string()))
            .child(
                div()
                    .id(("kb-list-x", i))
                    .group(format!("kb-x-{i}"))
                    .flex_none()
                    .size(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .opacity(0.0)
                    .group_hover(group, |s| s.opacity(1.0))
                    .hover(|s| s.bg(tint(DANGER, 0.16)))
                    .tooltip(tip("Delete list"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.board_delete_list(delete_id.clone(), window, cx);
                    }))
                    .child(svg().path("icons/kb-trash.svg").size(px(13.0)).text_color(color(MUTED_2)).group_hover(format!("kb-x-{i}"), |s| s.text_color(color(DANGER)))),
            )
            .into_any_element()
    }

    /// .kb-composer: "+ Add a card"; open, it grows with what is typed and
    /// shows "Add card" under it.
    fn board_composer(&self, b: &BoardView, list: &str, cx: &mut Context<Self>) -> AnyElement {
        let open = b.composing.as_deref() == Some(list);
        let frame = |focused: bool| field_box(focused, line(), true).w_full().py(px(7.0)).px(px(9.0)).rounded(px(10.0)).text_size(px(12.5)).line_height(relative(1.4));
        let body: AnyElement = if open {
            div()
                .child(frame(b.focused == Some(Field::Composer)).child(Textarea::new(&b.inputs.composer).appearance(false).with_size(px(12.5 / 0.875))))
                .child(div().mt(px(6.0)).flex().child(add_btn("kb-add-btn", "Add card").on_click(cx.listener(|this, _, window, cx| this.board_add_card(window, cx)))))
                .into_any_element()
        } else {
            let list = list.to_string();
            frame(false)
                .id(SharedString::from(format!("kb-new-{list}")))
                .cursor_text()
                .text_color(color(MUTED_2))
                .on_click(cx.listener(move |this, _, window, cx| this.board_start_card(list.clone(), window, cx)))
                .child("+ Add a card")
                .into_any_element()
        };
        div().pt(px(6.0)).px(px(8.0)).pb(px(9.0)).child(body).into_any_element()
    }

    /// .kb-addlist: the dashed "+ Add a list", or its title field.
    fn board_add_list(&self, b: &BoardView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let inner: AnyElement = if b.adding_list {
            field_box(true, accent_dim(), false)
                .w_full()
                .py(px(10.0))
                .px(px(12.0))
                .rounded(px(12.0))
                .bg(color(PANEL))
                .font_weight(FontWeight::SEMIBOLD)
                .child(Input::new(&b.inputs.new_list).appearance(false).with_size(px(12.5 / 0.875)))
                .into_any_element()
        } else {
            div()
                .id("kb-addlist-btn")
                .w_full()
                .py(px(10.0))
                .px(px(12.0))
                .rounded(px(12.0))
                .bg(color(PANEL))
                .border_1()
                .border_dashed()
                .border_color(line_strong())
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color(INK_3))
                .hover(|s| s.text_color(color(INK)).border_color(accent_dim()).bg(glow()))
                .on_click(cx.listener(|this, _, window, cx| this.board_start_list(window, cx)))
                .child("+ Add a list")
                .into_any_element()
        };
        div()
            .id("kb-addlist")
            .flex_none()
            .w(px(LIST_W))
            .on_drop(cx.listener(|this, d: &ListDrag, window, cx| this.board_drop_list(d.id.clone(), None, window, cx)))
            .child(inner)
    }
}
