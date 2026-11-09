//! The Grid's cards: every agent but the one in the terminal, as plain text
//! that reads at any width (renderRail and cardHtml in src/inbox.ts, .rc in
//! inbox.css). A card says who, its state and job, what it is doing and last
//! said, and carries answer buttons while it asks you something.

use supervise::ask::{Ask, AskOption};

use crate::gridpins::rail_groups;

use super::view::small_pill;
use super::*;

/// --ib-needs at 5% over --ib-panel: a card that waits on you.
const NEEDS_PANEL: u32 = 0x191815;

/// Permission prompts with the usual three choices get one row of short
/// buttons; anything else keeps its numbered list (isYesNo).
fn is_yes_no(a: &Ask) -> bool {
    a.kind != AskKind::Question && a.options.len() <= 3 && a.options.iter().filter(|o| !o.deny && !o.always).count() == 1
}

fn short_label(o: &AskOption) -> &'static str {
    if o.deny {
        "Deny"
    } else if o.always {
        "Always allow here"
    } else {
        "Allow"
    }
}

impl Inbox {
    pub(super) fn rail(&self, shown: &Shown, view: &View, cx: &mut Context<Self>) -> AnyElement {
        let home = self.fleet.seats.get(shown.big).map(|s| s.project());
        let list: Vec<(String, usize)> = shown.cards.iter().map(|i| (self.fleet.seats[*i].project(), *i)).collect();
        let groups = rail_groups(list, home.as_deref());
        // A heading names the project above its cards, unless every card shares
        // the terminal's project: then there is nothing to tell apart.
        let heads = groups.len() > 1 || groups.first().is_some_and(|g| Some(&g.0) != home.as_ref());
        let mut grid = div().grid().grid_cols(shown.cols as u16).gap(px(12.0));
        for (gi, (project, members)) in groups.into_iter().enumerate() {
            if heads {
                let mine = Some(&project) == home.as_ref();
                grid = grid.child(
                    div()
                        .col_span_full()
                        .flex()
                        .items_baseline()
                        .gap(px(8.0))
                        .min_w_0()
                        .when(gi > 0, |h| h.pt(px(6.0)))
                        .px(px(4.0))
                        .child(div().min_w_0().truncate().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(project))
                        .when(mine, |h| h.child(div().flex_none().text_size(px(12.0)).text_color(color(INK_3)).child("the terminal's project"))),
                );
            }
            for index in members {
                grid = grid.child(self.card(index, view, cx));
            }
        }
        div().id("rail").size_full().child(grid).overflow_y_scrollbar().into_any_element()
    }

    fn card(&self, index: usize, view: &View, cx: &mut Context<Self>) -> AnyElement {
        let seat = &self.fleet.seats[index];
        let status = &view.statuses[index];
        let state = status.state;
        let g = self.grid.glances.get(&seat.spec.id);
        let ask = status.ask.as_ref().filter(|a| state == State::Needs && self.grid.answered.get(&seat.spec.id) != Some(&ask_key(a)));
        let needs = state == State::Needs;
        let body = match ask {
            Some(a) => self.card_ask(index, a, cx),
            None => {
                let doing = if state == State::Working && status.limit.is_none() {
                    if !g.doing.is_empty() {
                        g.doing.clone()
                    } else if !g.asked.is_empty() {
                        "Working on what you asked".to_string()
                    } else {
                        "Working".to_string()
                    }
                } else {
                    said_line(status, view.worked[index])
                };
                let tone = if status.limit.is_some() { NEEDS } else if state == State::Working { WORK } else { INK_2 };
                let said = if !g.said.is_empty() {
                    Some(div().line_clamp(5).text_color(color(INK_2)).child(g.said.clone()))
                } else if !g.asked.is_empty() {
                    Some(div().line_clamp(5).text_color(color(INK_2)).child(div().flex().gap(px(4.0)).child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(color(INK_3)).child("You")).child(div().min_w_0().child(g.asked.clone()))))
                } else {
                    None
                };
                div().flex().flex_col().gap(px(8.0)).child(div().truncate().text_size(px(13.0)).text_color(color(tone)).child(doing)).children(said).into_any_element()
            }
        };
        let (name, project) = (seat.spec.name.clone(), seat.project());
        div()
            .id(("card", index))
            .tooltip(tip(format!("{name} in {project} \u{00b7} click to work with it in the terminal, right-click for more")))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .pt(px(14.0))
            .px(px(16.0))
            .pb(px(16.0))
            .rounded(px(16.0))
            .bg(color(if needs { NEEDS_PANEL } else { PANEL }))
            .shadow(vec![ring(if needs { tint(NEEDS, 0.3) } else { hairline() })])
            .text_size(px(14.0))
            .line_height(relative(1.5))
            .text_color(color(INK))
            .cursor_pointer()
            .when(!needs, |c| c.hover(|c| c.bg(color(RAISE))))
            .on_click(cx.listener(move |this, _, window, cx| this.set_current(index, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.menu = Some(Menu { index, at: event.position });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .min_w_0()
                    .child(self.mark(&seat.command(), 26.0, 7.0))
                    .child(div().min_w_0().truncate().text_size(px(14.5)).font_weight(FontWeight::SEMIBOLD).child(name))
                    .child(div().flex_none().text_size(px(12.5)).text_color(color(INK_3)).child(format!("\u{00b7} {}", cli_label(&seat.spec.cli))))
                    .child(small_pill(state).ml_auto()),
            )
            .children(seat.spec.title.clone().map(|t| div().truncate().font_weight(FontWeight::MEDIUM).child(t)))
            .child(body)
            .into_any_element()
    }

    /// .rc-ask: what it wants, and the buttons that answer it in place.
    fn card_ask(&self, index: usize, a: &Ask, cx: &mut Context<Self>) -> AnyElement {
        let lead = match a.kind {
            AskKind::Run => "Wants to run",
            AskKind::Edit => "Wants to edit",
            AskKind::Question => "Asks",
        };
        let what = match (&a.detail, a.kind) {
            (Some(d), k) if k != AskKind::Question => {
                div().max_w_full().max_h(px(60.0)).overflow_hidden().px(px(8.0)).py(px(4.0)).rounded(px(6.0)).bg(color(0x0a0a0c)).font_family("Geist Mono").text_size(px(12.5)).child(d.clone())
            }
            _ => div().child(a.prompt.clone()),
        };
        let rank = |o: &AskOption| if o.always { 0 } else if o.deny { 1 } else { 2 };
        let yes_no = is_yes_no(a);
        let mut options: Vec<AskOption> = a.options.clone();
        if yes_no {
            options.sort_by_key(rank);
        }
        let key = ask_key(a);
        let buttons = options.into_iter().map(|o| {
            let (bg, ink, hover) = match (yes_no, o.always, o.deny) {
                (true, true, _) => (hsla(0.0, 0.0, 0.0, 0.0), color(INK_2), white(0.13)),
                (true, _, true) => (tint(DANGER, 0.12), color(DANGER), white(0.13)),
                (true, _, _) => (color(0xffffff), color(0x000000), color(0xe9e9ec)),
                _ => (white(0.07), color(INK), white(0.13)),
            };
            let label = if yes_no { short_label(&o).to_string() } else { format!("{}. {}", o.n, o.label) };
            let key = key.clone();
            div()
                .id(SharedString::from(format!("opt-{index}-{}", o.n)))
                .when(yes_no, |b| b.tooltip(tip(o.label.clone())))
                .h(px(30.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .rounded_full()
                .bg(bg)
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ink)
                .hover(move |b| b.bg(hover))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.answer_card(index, &key, &o, cx);
                }))
                .child(label)
        });
        div()
            .flex()
            .flex_col()
            .child(div().flex().flex_col().items_start().gap(px(6.0)).mb(px(10.0)).child(div().text_size(px(12.5)).text_color(color(NEEDS)).child(lead)).child(what))
            .child(div().flex().flex_wrap().gap(px(6.0)).children(buttons))
            .into_any_element()
    }

    /// A card's button picks that option in its agent's CLI.
    fn answer_card(&mut self, index: usize, key: &str, option: &AskOption, cx: &mut Context<Self>) {
        let Some(seat) = self.fleet.seats.get(index) else { return };
        self.grid.answered.insert(seat.spec.id.clone(), key.to_string());
        if let Some(t) = &seat.terminal {
            t.read(cx).write_raw(option.key.clone().into_bytes());
        }
        cx.notify();
    }
}
