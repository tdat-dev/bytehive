//! The Company page (office.css .of): a bar with where you are and the way back
//! to the Inbox, the floors where the building stood with the job box over
//! them, and the rail beside. A short or narrow window gives the box its own
//! row under the rail, which then scrolls as one.

use super::look::*;
use super::*;

impl Inbox {
    pub(in crate::inbox) fn company_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let snap = self.company_snapshot(cx);
        let size = window.viewport_size();
        let narrow = size.width < px(1000.0);
        let short = size.height < px(640.0);
        let compact = narrow || short;
        let rail_w = if narrow { RAIL_W_NARROW } else { RAIL_W };
        let first_floor = snap.model.as_ref().and_then(|m| m.floors.first().map(|f| f.name.clone()));
        self.co_hint(first_floor.as_deref(), window, cx);

        let floors = match &snap.model {
            Some(m) => self.co_floors(m, cx),
            None => text(BODY, INK_2).p(px(S4)).child(snap.note.clone().unwrap_or_else(|| words::STARTING.into())).into_any_element(),
        };
        let job_box = self.co_job_box(snap.model.as_ref(), compact, cx);
        let mut stage = div().relative().flex_1().min_w_0().min_h_0().child(floors);
        let mut side = div().flex().flex_col().flex_none().w(px(rail_w)).min_h_0().bg(c(PANEL)).shadow(vec![BoxShadow { color: line(), offset: point(px(1.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(0.0), inset: true }]);
        let rail = self.co_rail(&snap, cx).p(px(S4)).flex_1().min_h_0();
        if compact {
            side = side.child(div().id("co-rail-scroll").flex_1().min_h_0().overflow_y_scroll().child(rail));
            if let Some(b) = job_box {
                side = side.child(div().flex_none().px(px(S4)).py(px(S3)).border_t_1().border_color(line()).child(b));
            }
        } else {
            side = side.child(rail.overflow_hidden());
            if let Some(b) = job_box {
                let w = (size.width.as_f32() * 0.32).clamp(300.0, 460.0);
                stage = stage.child(div().absolute().bottom(px(S4)).left(px(S4)).w(px(w)).p(px(S4)).rounded(px(R_CARD)).bg(c(PANEL)).shadow(vec![ring(line()), lift()]).child(b));
            }
        }
        let confirm = snap.model.as_ref().and_then(|m| self.co_confirm(m, cx));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c(GROUND))
            .text_color(c(INK))
            .font_family("Geist")
            .child(self.co_bar(cx))
            .child(div().flex().flex_1().min_h_0().child(stage).child(side))
            .children(confirm)
            .into_any_element()
    }

    /// At a person's desk, the crumb back: Company › their name.
    pub(in crate::inbox) fn co_crumb(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.company.desk.as_deref()?;
        let seat = self.fleet.focused_seat().filter(|s| s.spec.person.as_deref() == Some(id))?;
        let back = button("co-crumb", words::COMPANY, Btn::Secondary, false, false)
            .tooltip(tip("Back to the Company (Ctrl+Shift+O)"))
            .on_click(cx.listener(|this, _, window, cx| this.open_company(window, cx)));
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(S1))
                .ml(px(S3))
                .child(back)
                .child(text(ROW, INK_3).child("\u{203a}"))
                .child(text(ROW, INK).font_weight(FontWeight::SEMIBOLD).child(seat.spec.name.clone()))
                .into_any_element(),
        )
    }

    /// .of-bar: where you are on the left, the way back to the Inbox on the right.
    fn co_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let crumbs = div()
            .flex()
            .items_center()
            .gap(px(S2))
            .px(px(S1))
            .child(img(logo::logo()).size(px(22.0)).flex_none())
            .child(text(ROW, INK).font_weight(FontWeight::SEMIBOLD).child("ByteHive"))
            .child(text(ROW, INK_3).child("\u{203a}"))
            .child(text(ROW, INK).font_weight(FontWeight::SEMIBOLD).child(words::COMPANY));
        let inbox = button("co-inbox", words::INBOX, Btn::Secondary, false, false)
            .tooltip(tip("Inbox (Ctrl+Shift+O)"))
            .on_click(cx.listener(|this, _, window, cx| this.close_company(window, cx)));
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(S3))
            .h(px(BAR_H))
            .pl(px(S3))
            .pr(px(150.0))
            .bg(c(PANEL))
            .border_b_1()
            .border_color(line())
            .window_control_area(WindowControlArea::Drag)
            .child(crumbs)
            .child(inbox)
            .child(div().absolute().top_0().right_0().child(chrome::window_buttons()))
    }
}
