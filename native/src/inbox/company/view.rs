//! The Company page, dressed as the Inbox: the same top bar (Home, a headline,
//! the way back to the Inbox), the floors in one panel with the job box at its
//! foot, the rail in another beside it, and the Inbox's dock below. A short or
//! narrow window gives the box its own row under the rail, which then scrolls
//! as one.

use super::look::*;
use super::*;

/// How much of the job box's reach the picture moves over for (shell.ts JOB_BOX_SHARE).
const JOB_BOX_SHARE: f32 = 0.38;

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
        // What is cut below the fold is read from the last layout: after a resize or a
        // new card, draw once more so the count is of this one.
        let key = (size, snap.model.as_ref().map_or(0, |m| m.needs.len()));
        if self.company.laid_out != key {
            self.company.laid_out = key;
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }

        let office3d = crate::office3d::view::demo_wanted() || (snap.model.is_some() && office::office3d_on());
        let floors = match &snap.model {
            // P0 of the 3D office: the spike in place of the floors, its cost written over it.
            _ if crate::office3d::spike::wanted() => div()
                .relative()
                .size_full()
                .child(crate::office3d::spike::view())
                .child(text(META, INK_2).absolute().top(px(S3)).left(px(S4)).child(crate::office3d::spike::line()))
                .into_any_element(),
            // The 3D office with the demo company, to look at while it is ported.
            _ if crate::office3d::view::demo_wanted() => div()
                .relative()
                .size_full()
                .child(crate::office3d::view::demo())
                .child(text(META, INK_2).absolute().top(px(S3)).left(px(S4)).child(crate::office3d::view::line()))
                .into_any_element(),
            Some(m) if office3d => self.co_office3d(m, window, cx),
            Some(m) => self.co_floors(m, cx),
            None => text(BODY, INK_2).p(px(S4)).child(snap.note.clone().unwrap_or_else(|| words::STARTING.into())).into_any_element(),
        };
        let job_box = self.co_job_box(snap.model.as_ref(), compact, cx);
        // The box stands at the stage's foot: over a 2D list it would hide people, as it never did over the building.
        // The Inbox's panels: the queue's and the stage's frame, the dock's room below.
        let panel = || div().flex().flex_col().min_h_0().mb(px(DOCK_H)).rounded(px(R_PANEL)).bg(c(PANEL)).shadow(vec![ring(line())]).overflow_hidden();
        let mut stage = panel().relative().flex_1().min_w_0().ml(px(16.0)).child(div().flex_1().min_h_0().child(floors));
        let mut side = panel().flex_none().w(px(rail_w)).mx(px(16.0));
        let rail = self.co_rail(&snap, compact, cx).p(px(S4)).flex_1().min_h_0();
        if compact {
            let below = self.co_rail_below(cx);
            side = side
                .child(
                    div()
                        .id("co-rail-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.company.rail_scroll)
                        // What waits below follows a scroll.
                        .on_scroll(cx.listener(|_, _, window, cx| {
                            cx.notify();
                            cx.on_next_frame(window, |_, _, cx| cx.notify());
                        }))
                        .child(rail),
                )
                .children(below);
            if let Some(b) = job_box {
                side = side.child(div().flex_none().px(px(S4)).py(px(S3)).border_t_1().border_color(line()).child(b));
            }
        } else {
            side = side.child(rail.overflow_hidden());
            if let Some(b) = job_box {
                let w = (size.width.as_f32() * 0.32).clamp(300.0, 460.0);
                let card = div().w(px(w)).p(px(S4)).rounded(px(R_CARD)).bg(c(RAISED)).shadow(vec![ring(line()), lift()]).child(b);
                if office3d {
                    // Over the building it stands on the picture, and the picture moves over for it (shell.ts clearForJobBox).
                    crate::office3d::view::set_clear(((S4 + w) * JOB_BOX_SHARE) as f64);
                    stage = stage.child(div().absolute().left(px(S4)).bottom(px(S4)).occlude().child(card));
                } else {
                    stage = stage.child(div().flex_none().px(px(S4)).pb(px(S4)).child(card));
                }
            } else if office3d {
                crate::office3d::view::set_clear(0.0);
            }
        }
        // A confirm or a menu for a job that has gone goes with it.
        if let Some(m) = &snap.model {
            let gone = |id: &Option<String>| id.as_ref().is_some_and(|id| !m.jobs.iter().any(|j| &j.id == id));
            if gone(&self.company.confirm) {
                self.company.confirm = None;
            }
            if gone(&self.company.menu) {
                self.company.menu = None;
            }
        }
        let confirm = snap.model.as_ref().and_then(|m| self.co_confirm(m, cx));
        let needs = snap.model.as_ref().map_or(0, |m| m.needs.len());
        // At a floor of the 3D office, the bar says which, and the way back to the building.
        let at_floor = match (&self.company.level3d, &snap.model) {
            (crate::office3d::layout::Level::Floor(id), Some(m)) if office3d => m.floors.iter().find(|f| &f.id == id).map(|f| f.name.clone()),
            _ => None,
        };
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(c(GROUND))
            .text_color(c(INK))
            .font_family("Geist")
            // A press anywhere else closes a job's menu.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.company.menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            // The Inbox's amber haze over the top (agents_view).
            .child(div().absolute().left(relative(0.62)).ml(px(-1100.0)).top(relative(-0.12)).mt(px(-460.0)).w(px(2200.0)).h(px(920.0)).child(img(haze()).size_full().object_fit(ObjectFit::Fill)))
            .child(self.co_bar(needs, at_floor, cx))
            .child(div().flex().flex_1().min_h_0().child(stage).child(side))
            .child(self.dock(cx))
            .children(confirm)
            .into_any_element()
    }

    /// At a person's desk, the crumb back: Company › their name.
    pub(in crate::inbox) fn co_crumb(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.company.desk.as_deref()?;
        let seat = self.fleet.focused_seat().filter(|s| s.spec.person.as_deref() == Some(id))?;
        let back = button("co-crumb", words::COMPANY, Btn::Secondary, false, false)
            .occlude()
            .tooltip(tip(words::BACK_TIP))
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

    /// The Inbox's top bar (chrome.rs top_bar): Home, a headline of what waits,
    /// the way back to the Inbox where the Company door stands there.
    fn co_bar(&self, needs: usize, at_floor: Option<String>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rest = match needs {
            0 => "Nothing needs you.".to_string(),
            1 => "1 needs you.".to_string(),
            n => format!("{n} need you."),
        };
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(68.0))
            .pl(px(12.0))
            .pr(px(4.0))
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .id("co-home")
                    .occlude()
                    .tooltip(tip("Home (keep agents running)"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_company(window, cx);
                        this.open_home(window, cx);
                    }))
                    .size(px(30.0))
                    .rounded(px(7.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|h| h.bg(c(PANEL)))
                    .child(icon("home", 16.0, c(INK_3))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .ml(px(18.0))
                    .truncate()
                    .text_size(px(24.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(INK_3))
                    .child(match at_floor {
                        // Company, then the floor: Company goes back up to the building.
                        Some(name) => div()
                            .flex()
                            .min_w_0()
                            .child(
                                div()
                                    .id("co-building")
                                    .occlude()
                                    .flex_none()
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(INK_2))
                                    .hover(|d| d.text_color(c(INK)))
                                    .tooltip(tip("Back to the building (Esc)"))
                                    .on_click(cx.listener(|this, _, _, cx| this.co_level(crate::office3d::layout::Level::Building, cx)))
                                    .child(words::COMPANY),
                            )
                            .child(div().flex_none().px(px(10.0)).text_color(c(INK_3)).child("\u{203a}"))
                            .child(div().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).text_color(c(INK)).child(name)),
                        None => div()
                            .flex()
                            .min_w_0()
                            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(INK)).child(format!("{}. ", words::COMPANY)))
                            .child(div().min_w_0().truncate().child(rest)),
                    }),
            )
            .child(
                div()
                    .id("co-inbox")
                    .occlude()
                    .tooltip(tip(words::INBOX_TIP))
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(14.0))
                    .mr(px(146.0))
                    .rounded_full()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(INK_2))
                    .hover(|d| d.bg(white_at(0.06)).text_color(c(INK)))
                    .on_click(cx.listener(|this, _, window, cx| this.close_company(window, cx)))
                    .child(words::INBOX),
            )
            .child(div().absolute().top_0().right_0().child(chrome::window_buttons()))
    }
}
