//! Token usage & cost (usage.ts, #usageModal in index.html, .modal.usage in
//! modal.css): what Claude has used in the current project, per model, with
//! an estimated cost at API list prices.

use agent_core::usage::ModelUsage;

use crate::project_usage::{claude_usage, fmt_tokens, money, short_model, usage_cost};

use super::dialog::{self, MUTED_2};
use super::*;

pub(super) struct UsageView {
    /// The folder read; None when there is no project, which shows the empty note.
    dir: Option<String>,
    /// None while the transcripts are still being read.
    rows: Option<Vec<ModelUsage>>,
    /// Keys land here, never in a terminal behind.
    focus: FocusHandle,
}

/// The table's number columns: Msgs, Input, Output, Cache, Est. cost.
const COLS: [f32; 5] = [52.0, 72.0, 72.0, 72.0, 84.0];

impl Inbox {
    /// openUsage: the current project's folder, read off the UI thread.
    pub(super) fn open_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.menu = None;
        self.popup = None;
        crate::keys::set_active(false);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let dir = self.project_dir();
        self.usage_view = Some(UsageView { dir: dir.clone(), rows: None, focus });
        if let Some(dir) = dir {
            let read = cx.background_spawn(async move { claude_usage(&dir) });
            cx.spawn(async move |this, cx| {
                let rows = read.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(u) = &mut this.usage_view {
                        u.rows = Some(rows);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub(super) fn close_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.usage_view = None;
        if !self.home {
            if let Some(index) = self.fleet.focused {
                self.fleet.focus(index, window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn usage_modal(&self, u: &UsageView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let crumb = u.dir.as_deref().map(crate::fleet::project_of).unwrap_or_default();
        let empty = u.dir.is_none() || u.rows.as_ref().is_some_and(|r| r.is_empty());
        let table = u.rows.as_ref().filter(|r| !r.is_empty()).map(|rows| usage_table(rows));
        dialog::backdrop("usage-back", cx.listener(|this, _, window, cx| this.close_usage(window, cx))).child(
            dialog::modal("usage", 560.0, &u.focus)
                .child(
                    dialog::head("Token usage & cost")
                        // .replay-crumb
                        .child(mono().ml(px(2.0)).min_w_0().truncate().text_size(px(11.5)).text_color(color(INK_3)).child(crumb))
                        .child(dialog::x("usage-x").on_click(cx.listener(|this, _, window, cx| this.close_usage(window, cx)))),
                )
                .child(
                    dialog::body("usage-body")
                        .children(table)
                        .when(empty, |b| {
                            // .replay-empty
                            b.child(
                                div()
                                    .py(px(26.0))
                                    .px(px(20.0))
                                    .text_center()
                                    .text_size(px(12.5))
                                    .line_height(relative(1.5))
                                    .text_color(color(MUTED_2))
                                    .child("No Claude usage in this project yet. Costs come from Claude Code's own session logs, so they appear once a Claude agent has run here."),
                            )
                        })
                        // .usage-note
                        .child(div().mt(px(14.0)).text_size(px(11.0)).line_height(relative(1.5)).text_color(color(MUTED_2)).child("Estimated from Claude Code's session transcripts. Claude agents only; other CLIs aren't counted. Prices are approximate, so treat the total as a guide, not a bill.")),
                )
                .child(dialog::foot().child(dialog::ghost("usage-close", "Close").on_click(cx.listener(|this, _, window, cx| this.close_usage(window, cx))))),
        )
    }
}

/// One table line: the model cell, then the five number cells.
fn line(first: Div, cells: [Div; 5], border: bool) -> Div {
    let mut row = div().flex().items_center().child(first.flex_1().min_w_0());
    for (cell, w) in cells.into_iter().zip(COLS) {
        row = row.child(cell.w(px(w)).flex_none().flex().justify_end());
    }
    row.when(border, |r| r.border_b_1().border_color(hairline()))
}

/// .usage-tbl: head, a row per model, the estimated total.
fn usage_table(rows: &[ModelUsage]) -> Div {
    let th = |s: &'static str| div().text_size(px(10.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK_3)).child(s);
    let head = line(th("Model"), [th("Msgs"), th("Input"), th("Output"), th("Cache"), th("Est. cost")], true).pb(px(8.0));
    let mut total = 0.0;
    let mut unpriced = false;
    let mut body: Vec<Div> = Vec::new();
    for u in rows {
        let cost = usage_cost(u);
        match cost {
            Some(c) => total += c,
            None => unpriced = true,
        }
        let td = |s: String| div().text_color(color(INK_2)).child(s);
        // No price: a dash where today's app writes an em dash.
        let cost_cell = div().text_color(color(INK)).font_weight(FontWeight::SEMIBOLD).child(cost.map(money).unwrap_or_else(|| "-".to_string()));
        body.push(
            line(
                mono().text_size(px(11.5)).text_color(color(INK)).truncate().child(short_model(&u.model)),
                [td(u.messages.to_string()), td(fmt_tokens(u.input_tokens)), td(fmt_tokens(u.output_tokens)), td(fmt_tokens(u.cache_creation + u.cache_read)), cost_cell],
                true,
            )
            .py(px(9.0)),
        );
    }
    let label = format!("Estimated total{}", if unpriced { " (priced models only)" } else { "" });
    let foot = div()
        .flex()
        .items_center()
        .pt(px(11.0))
        .pb(px(9.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().flex_1().min_w_0().flex().justify_end().text_color(color(INK_3)).child(label))
        .child(div().w(px(COLS[4])).flex_none().flex().justify_end().text_color(color(INK)).child(money(total)));
    div().flex().flex_col().text_size(px(12.5)).child(head).children(body).child(foot)
}
