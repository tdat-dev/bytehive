//! The queue: project heads, agent rows, and the plans' limits at its foot.

use super::*;

impl Inbox {
    /// cliMark: the CLI's logo on a dark tile of its colour, a prompt for a
    /// shell, a letter for a CLI with no logo.
    pub(super) fn mark(&self, command: &str, size: f32, radius: f32) -> Div {
        let hex = brand::color_of(command);
        let base = div()
            .relative()
            .flex_none()
            .size(px(size))
            .rounded(px(radius))
            .bg(tile(hex))
            .shadow(vec![ring(tint(hex, 0.38))])
            .flex()
            .items_center()
            .justify_center();
        match brand::mark_of(command) {
            Mark::Logo { image, fills: true, .. } => base.child(img(image).size_full().rounded(px(radius))),
            Mark::Logo { image, .. } => base.child(img(image).size(px(size * 0.68))),
            Mark::Shell => base.child(icon("prompt", size * 0.6, color(hex))),
            Mark::Letter { letter, color: c } => base.child(div().text_size(px(size * 0.42)).font_weight(FontWeight::BOLD).text_color(color(c)).child(letter)),
        }
    }

    pub(super) fn agent_row(&self, index: usize, seat: &Seat, status: &TaskStatus, worked: bool, since: u64, cx: &mut Context<Self>) -> AnyElement {
        let state = status.state;
        let current = self.fleet.focused == Some(index);
        let ring_ground = if current { RAISE } else { PANEL };
        let dot = div().absolute().right(px(-3.0)).bottom(px(-3.0)).size(px(13.0)).rounded_full().bg(color(ring_ground)).flex().items_center().justify_center().child(match state {
            s if ended(s) => div().size(px(9.0)).rounded_full().shadow(vec![ring(color(INK_3))]).into_any_element(),
            s if waits(s) => icon("diamond", 11.0, color(NEEDS)).into_any_element(),
            s => div().size(px(9.0)).rounded_full().bg(color(hue_of(s).unwrap_or(INK_3))).into_any_element(),
        });
        let group: SharedString = format!("row-{index}").into();
        let line = row_line(status, worked);
        let used = self.uses.get(&seat.spec.id).and_then(|u| u.usage.clone());
        div()
            .id(("seat", index))
            .tooltip(tip(format!("{}{} . {} . right-click for more", seat.spec.title.clone().unwrap_or_else(|| seat.spec.name.clone()), if seat.spec.title.is_some() { format!(" . {}", seat.spec.name) } else { String::new() }, seat.project()).replace(" . ", " \u{00b7} ")))
            .group(group.clone())
            .relative()
            .flex()
            .gap(px(10.0))
            .p(px(10.0))
            .rounded(px(12.0))
            .text_color(color(INK))
            .when(waits(state), |row| {
                row.bg(tint(NEEDS, if current { 0.2 } else { 0.12 })).shadow(vec![ring(tint(NEEDS, 0.3))]).hover(|row| row.bg(tint(NEEDS, 0.17)))
            })
            .when(current && !waits(state), |row| row.bg(color(RAISE)).shadow(vec![ring(white(0.1))]))
            .when(!current && !waits(state), |row| row.hover(|row| row.bg(white(0.03))))
            .on_click(cx.listener(move |this, _, window, cx| this.open_agent(index, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.menu = Some(Menu { index, at: event.position });
                    cx.notify();
                }),
            )
            .child(self.mark(&seat.command(), 24.0, 7.0).when(ended(state), |mark| mark.opacity(0.55)).child(dot))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(14.0))
                                    .font_weight(if state == State::Stopped { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                                    .text_color(color(if state == State::Stopped { INK_2 } else { INK }))
                                    // The job is the row's headline when there is one.
                                    .child(seat.spec.title.clone().unwrap_or_else(|| seat.spec.name.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .gap(px(4.0))
                                    .group_hover(group.clone(), |time| time.opacity(0.0))
                                    // .iq-pin while it sits in the Grid on screen.
                                    .when(self.grid.on && self.in_grid(index), |time| time.child(div().id(("in-grid", index)).tooltip(tip("In the Grid")).text_size(px(12.0)).text_color(color(INK_2)).child("\u{25eb}")))
                                    .child(mono().text_size(px(12.0)).text_color(color(INK_3)).child(ago(now_ms().saturating_sub(since)))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .min_w_0()
                            .text_size(px(12.5))
                            .when(state == State::Stopped, |line| line.pr(px(72.0)))
                            .when(seat.spec.title.is_some(), |line| line.child(div().flex_none().text_color(color(INK_3)).child(format!("{} ·", seat.spec.name))))
                            .when_some(seat.spec.race.clone(), |line, race| {
                                line.child(mono().flex_none().px(px(6.0)).py(px(1.0)).rounded(px(5.0)).bg(white(0.07)).text_size(px(11.0)).text_color(color(INK_2)).child(format!("race {}/{}", race.n, race.of)))
                            })
                            .child(div().min_w_0().truncate().text_color(color(hue_of(state).unwrap_or(INK_3))).child(line))
                            // What its conversation has used: money when its CLI
                            // prices it, else tokens; its context once nearly full.
                            .children(used.as_ref().and_then(|u| {
                                let what = u.usd.map(supervise::pricing::money).or_else(|| u.tokens.map(|t| format!("{} tokens", usage::tokens(t))))?;
                                Some(mono().min_w_0().truncate().text_size(px(11.5)).text_color(color(INK_3)).child(format!("· {what}")))
                            }))
                            .children(used.as_ref().and_then(|u| u.context_pct).filter(|p| *p >= 80.0).map(|p| mono().flex_none().text_size(px(11.5)).text_color(color(NEEDS)).child(format!("· {}% context", p.round())))),
                    ),
            )
            .child(
                div()
                    .id(("more", index))
                    .absolute()
                    .top(px(8.0))
                    .right(px(8.0))
                    .size(px(26.0))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(color(0x1a1b20))
                    .shadow(vec![ring(white(0.1))])
                    .opacity(0.0)
                    .group_hover(group, |more| more.opacity(1.0))
                    .hover(|more| more.bg(color(0x24252b)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.menu = Some(Menu { index, at: event.position });
                            cx.notify();
                        }),
                    )
                    .child(icon("dots", 16.0, color(INK_2))),
            )
            .when(state == State::Stopped, |row| {
                row.child(
                    div()
                        .id(("resume-row", index))
                        .tooltip(tip(format!("Start {} again, in the conversation it was last in", seat.spec.name)))
                        .absolute()
                        .right(px(8.0))
                        .bottom(px(8.0))
                        .h(px(24.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(white(0.1))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color(INK))
                        .hover(|pill| pill.bg(color(INK)).text_color(color(GROUND)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| this.resume(index, window, cx)))
                        .child("Resume"),
                )
            })
            .into_any_element()
    }

    pub(super) fn project_head(&self, name: &str, count: usize, needs: usize, cx: &mut Context<Self>) -> AnyElement {
        let folded = self.folded.contains(name);
        let key = name.to_string();
        let project = name.to_string();
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .mt(px(6.0))
            .pt(px(8.0))
            .px(px(4.0))
            .pb(px(4.0))
            .child(
                div()
                    .id(SharedString::from(format!("proj-{name}")))
                    .tooltip(tip(if folded { format!("{name} \u{00b7} unfold") } else { format!("{name} \u{00b7} fold the stopped away") }))
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(7.0))
                    .min_h(px(32.0))
                    .pl(px(6.0))
                    .pr(px(8.0))
                    .rounded(px(9.0))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(INK_2))
                    .hover(|head| head.bg(white(0.04)).text_color(color(INK)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.folded.remove(&key) {
                            this.folded.insert(key.clone());
                        }
                        cx.notify();
                    }))
                    .child(icon(if folded { "chev" } else { "chev-open" }, 10.0, tint(INK_2, 0.7)))
                    .child(div().flex_1().min_w_0().truncate().child(name.to_string()))
                    .when(needs > 0, |head| {
                        head.child(
                            mono()
                                .flex_none()
                                .min_w(px(18.0))
                                .h(px(18.0))
                                .px(px(5.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(color(NEEDS))
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color(0x1a1204))
                                .child(needs.to_string()),
                        )
                    })
                    .child(mono().flex_none().font_weight(FontWeight::NORMAL).text_size(px(12.0)).text_color(color(INK_3)).child(count.to_string())),
            )
            .child(
                div()
                    .id(SharedString::from(format!("mem-{name}")))
                    .tooltip(tip(format!("Project memory: what every agent in {name} knows")))
                    .flex_none()
                    .size(px(30.0))
                    .rounded(px(9.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.7)
                    .hover(|mem| mem.opacity(1.0).bg(white(0.06)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(dir) = this.fleet.seats.iter().find(|s| s.project() == project).map(|s| s.spec.cwd.clone()) {
                            this.open_memory(dir, window, cx);
                        }
                    }))
                    .child(icon("memory", 15.0, color(INK_3))),
            )
            .into_any_element()
    }

    pub(super) fn queue(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // Projects in the order they first appear; nothing moves when a state
        // changes. Folding hides a project's stopped agents.
        let mut projects: Vec<String> = Vec::new();
        for seat in &self.fleet.seats {
            let name = seat.project();
            if !projects.contains(&name) {
                projects.push(name);
            }
        }
        let mut list = div().id("queue").flex().flex_col().flex_1().min_h_0().px(px(8.0)).pt(px(6.0)).pb(px(12.0));
        for project in &projects {
            let members: Vec<usize> = (0..self.fleet.seats.len()).filter(|i| &self.fleet.seats[*i].project() == project).collect();
            let needs = members.iter().filter(|i| waits(view.statuses[**i].state)).count();
            let folded = self.folded.contains(project);
            list = list.child(self.project_head(project, members.len(), needs, cx));
            for index in members {
                if folded && ended(view.statuses[index].state) {
                    continue;
                }
                let row = self.agent_row(index, &self.fleet.seats[index], &view.statuses[index], view.worked[index], view.since[index], cx);
                list = list.child(row);
            }
        }
        if projects.is_empty() {
            list = list.child(div().mx(px(10.0)).mt(px(6.0)).mb(px(6.0)).text_size(px(12.5)).text_color(color(INK_3)).child("No agents yet."));
        }
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(QUEUE_W - 32.0))
            .mx(px(16.0))
            .mb(px(DOCK_H))
            .rounded(px(18.0))
            .bg(color(PANEL))
            .shadow(vec![ring(hairline())])
            .overflow_hidden()
            // A thin thumb over a clear track, as tokens.css draws scrollbars.
            .child(list.overflow_y_scrollbar())
            .children(self.plan_view())
    }

    /// .iq-plan: the plans' limits at the foot of the list, always there once
    /// a plan has something to say.
    pub(super) fn plan_view(&self) -> Option<AnyElement> {
        let now = chrono::Utc::now().timestamp_millis();
        let mut sections: Vec<AnyElement> = Vec::new();
        for src in [&self.plans.0, &self.plans.1] {
            let Some(u) = &src.usage else {
                if src.problem.as_deref() == Some("expired") {
                    sections.push(div().text_size(px(12.0)).line_height(relative(1.5)).text_color(color(INK_3)).child("Claude's sign-in renews the next time an agent runs; its limits show then.").into_any_element());
                }
                continue;
            };
            if plan::all_reset(u, now) {
                continue;
            }
            let rows = plan::rows(u, now);
            if rows.is_empty() {
                continue;
            }
            // How fresh: a Claude answer that failed since, or Codex numbers from its last run.
            let as_of = if src.problem.is_some() { Some(src.asked_at as i64) } else { u.as_of_ms };
            let stale = as_of.filter(|at| *at > 0 && (src.problem.is_some() || now - at > 10 * 60_000));
            let mut section = div().flex().flex_col().gap(px(12.0)).child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .items_baseline()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(INK_3))
                    .child(plan::title(u))
                    .children(stale.map(|at| div().font_weight(FontWeight::NORMAL).child(format!("as of {}", plan::clock(at))))),
            );
            for row in rows {
                let tone = match row.level {
                    plan::Level::Full => Some(DANGER),
                    plan::Level::High => Some(NEEDS),
                    plan::Level::Ok => None,
                };
                section = section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.0))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_baseline()
                                .text_size(px(12.5))
                                .child(div().text_color(color(INK_2)).child(row.label))
                                .child(mono().font_weight(FontWeight::MEDIUM).text_size(px(12.0)).text_color(color(tone.unwrap_or(INK))).child(format!("{}%", row.percent))),
                        )
                        .child(div().h(px(4.0)).rounded_full().bg(white(0.08)).overflow_hidden().child(div().h_full().min_w(px(2.0)).w(relative(row.percent.min(100) as f32 / 100.0)).rounded_full().bg(color(tone.unwrap_or(INK_2))))),
                )
                .when(!row.resets.is_empty(), |s| s.child(div().mt(px(-7.0)).text_size(px(11.5)).text_color(color(INK_3)).child(format!("resets {}", row.resets))));
            }
            sections.push(section.into_any_element());
        }
        if sections.is_empty() {
            return None;
        }
        let mut foot = div().flex().flex_col().flex_none().pt(px(12.0)).px(px(14.0)).pb(px(14.0)).bg(color(PANEL)).rounded_b(px(18.0)).border_t_1().border_color(hairline());
        for (i, section) in sections.into_iter().enumerate() {
            foot = foot.child(div().when(i > 0, |d| d.mt(px(12.0)).pt(px(12.0)).border_t_1().border_color(hairline())).child(section));
        }
        Some(foot.into_any_element())
    }
}
