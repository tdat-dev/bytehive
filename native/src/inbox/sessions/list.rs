//! The left column of Sessions: which sessions, a search box, the list by day.

use chrono::{Local, TimeZone as _};
use gpui_kit::component::Sizable as _;

use super::*;

/// dayLabel: "Today", "Yesterday", or "Mon 21 Sep".
pub(in crate::inbox) fn day_label(at_ms: u64, now_ms: u64) -> String {
    let day = |ms: u64| Local.timestamp_millis_opt(ms as i64).single().map(|d| d.date_naive());
    match (day(at_ms), day(now_ms)) {
        (Some(at), Some(now)) => match (now - at).num_days() {
            0 => "Today".to_string(),
            1 => "Yesterday".to_string(),
            _ => at.format("%a %-d %b").to_string(),
        },
        _ => String::new(),
    }
}

/// clock: "09:41".
pub(in crate::inbox) fn clock(at_ms: u64) -> String {
    Local.timestamp_millis_opt(at_ms as i64).single().map(|d| d.format("%H:%M").to_string()).unwrap_or_default()
}

/// placeOf: where a session ran, said for the list. Nothing for the project
/// itself, an agent's worktree by its name, else the folder's last two parts.
pub(in crate::inbox) fn place_of(cwd: &str, root: Option<&str>) -> String {
    if cwd.is_empty() || root.is_some_and(|r| presets::same_folder(cwd, r)) {
        return String::new();
    }
    let p = cwd.replace('\\', "/");
    let parts: Vec<&str> = p.trim_end_matches('/').split('/').collect();
    let n = parts.len();
    for marker in [".bytehive-worktrees", ".maestro-worktrees"] {
        if n >= 3 && parts[n - 3] == marker {
            // <repo>-<hash>/<branch>-<hash>: the branch, without its hash.
            let leaf = parts[n - 1];
            let name = match leaf.rsplit_once('-') {
                Some((name, hash)) if hash.len() == 6 && hash.chars().all(|c| c.is_ascii_alphanumeric()) => name,
                _ => leaf,
            };
            return format!("worktree {name}");
        }
    }
    if n >= 3 && parts[n - 3] == ".claude" && parts[n - 2] == "worktrees" {
        return format!("worktree {}", parts[n - 1]);
    }
    parts[n.saturating_sub(2)..].join("/")
}

impl Inbox {
    pub(super) fn sessions_side(&self, s: &Sessions, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shown = s.shown(cx);
        let typed = !s.query.read(cx).value().trim().is_empty();
        let count = if s.listing {
            String::new()
        } else if typed {
            format!("{} of {}", shown.len(), s.list.len())
        } else {
            format!("{} session{}", s.list.len(), if s.list.len() == 1 { "" } else { "s" })
        };
        let root = (s.scope == Scope::Project).then(|| s.root.as_ref().map(|(d, _)| d.clone())).flatten();
        let reading = s.reading.as_ref().map(|r| r.s.id.clone());
        let now = now_ms();

        let mut rows: Vec<AnyElement> = Vec::new();
        let empty = |text: &'static str| div().my(px(18.0)).mx(px(10.0)).text_size(px(13.0)).text_color(color(INK_3)).child(text).into_any_element();
        if s.listing && s.list.is_empty() {
            rows.push(empty("Looking for sessions…"));
        } else if shown.is_empty() {
            rows.push(empty(if typed {
                "No session matches that."
            } else if s.scope == Scope::Project {
                "No Claude sessions in this project yet."
            } else {
                "No Claude sessions yet."
            }));
        }
        let mut day = String::new();
        for (i, info) in shown.iter().enumerate() {
            let d = day_label(info.modified_ms, now);
            if d != day {
                rows.push(div().mt(px(12.0)).mb(px(4.0)).mx(px(10.0)).text_size(px(12.0)).font_weight(FontWeight::MEDIUM).text_color(color(INK_3)).child(d.clone()).into_any_element());
                day = d;
            }
            let on = reading.as_deref() == Some(info.id.as_str());
            let place = place_of(&info.cwd, root.as_deref());
            let pick = (*info).clone();
            rows.push(
                div()
                    .id(("sv-item", i))
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .py(px(9.0))
                    .px(px(10.0))
                    .rounded(px(10.0))
                    .text_color(color(if on { INK } else { INK_2 }))
                    .when(on, |r| r.bg(white(0.09)))
                    .when(!on, |r| r.hover(|r| r.bg(white(0.05)).text_color(color(INK))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.sessions.as_ref().and_then(|s| s.reading.as_ref()).is_none_or(|r| r.s.id != pick.id) {
                            this.read_session(pick.clone(), false, cx);
                        }
                    }))
                    .child(div().text_size(px(13.5)).line_height(relative(1.35)).line_clamp(2).child(if info.title.is_empty() { "Untitled".to_string() } else { info.title.clone() }))
                    .child(
                        div()
                            .flex()
                            .gap(px(10.0))
                            .min_w_0()
                            .text_size(px(12.0))
                            .text_color(color(INK_3))
                            .child(mono().flex_none().child(clock(info.modified_ms)))
                            .child(div().flex_none().child(format!("{} message{}", info.messages, if info.messages == 1 { "" } else { "s" })))
                            .when(!place.is_empty(), |m| m.child(div().min_w_0().truncate().child(place))),
                    )
                    .into_any_element(),
            );
        }

        let scope_button = |id: &'static str, label: String, on: bool, enabled: bool| {
            div()
                .id(id)
                .flex_1()
                .min_w_0()
                .h(px(28.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .text_size(px(12.5))
                .text_color(color(if on { INK } else { INK_3 }))
                .when(on, |b| b.bg(white(0.1)))
                .when(!enabled, |b| b.opacity(0.4))
                .child(div().truncate().child(label))
        };
        let project_name = s.root.as_ref().map(|(_, n)| n.clone()).unwrap_or_else(|| "This project".to_string());
        let project_tip = s.root.as_ref().map(|(d, _)| d.clone()).unwrap_or_default();

        div()
            .w(px(340.0))
            .flex_none()
            .flex()
            .flex_col()
            .min_h_0()
            .border_r_1()
            .border_color(hairline())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .pt(px(16.0))
                    .pr(px(14.0))
                    .pb(px(10.0))
                    .pl(px(18.0))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_baseline()
                            .gap(px(10.0))
                            .child(div().text_size(px(17.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Sessions"))
                            .child(div().text_size(px(12.0)).text_color(color(INK_3)).child(count)),
                    )
                    .child(
                        div()
                            .id("sv-close")
                            .tooltip(tip("Close (Esc)"))
                            .h(px(28.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .bg(white(0.06))
                            .text_size(px(12.5))
                            .text_color(color(INK_2))
                            .hover(|b| b.bg(white(0.12)).text_color(color(INK)))
                            .on_click(cx.listener(|this, _, window, cx| this.close_sessions(window, cx)))
                            .child("Close"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .mx(px(14.0))
                    .mb(px(10.0))
                    .p(px(3.0))
                    .rounded(px(10.0))
                    .bg(hsla(0.0, 0.0, 0.0, 0.3))
                    .child(
                        scope_button("sv-project", project_name, s.scope == Scope::Project, s.root.is_some())
                            .when(!project_tip.is_empty(), |b| b.tooltip(tip(project_tip)))
                            .on_click(cx.listener(|this, _, _, cx| this.set_scope(Scope::Project, cx))),
                    )
                    .child(scope_button("sv-all", "All folders".to_string(), s.scope == Scope::All, true).on_click(cx.listener(|this, _, _, cx| this.set_scope(Scope::All, cx)))),
            )
            .child(
                div()
                    .mx(px(14.0))
                    .mb(px(8.0))
                    .h(px(34.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .rounded(px(10.0))
                    .bg(hsla(0.0, 0.0, 0.0, 0.35))
                    .shadow(vec![ring(white(0.09))])
                    .text_size(px(13.5))
                    .child(Input::new(&s.query).appearance(false).with_size(px(13.5 / 0.875))),
            )
            .child(div().id("sv-list").flex_1().min_h_0().overflow_y_scroll().px(px(8.0)).pb(px(10.0)).children(rows))
            .child(
                div()
                    .pt(px(10.0))
                    .px(px(18.0))
                    .pb(px(14.0))
                    .border_t_1()
                    .border_color(hairline())
                    .text_size(px(11.5))
                    .line_height(relative(1.4))
                    .text_color(color(INK_3))
                    .child("Claude Code sessions, read only. Nothing here starts or resumes an agent."),
            )
    }
}
