//! The conductor's button in the board's header (condBtn in kanban.ts,
//! .kb-tool and its .on / .on.auto rules in dock.css): it cycles Off →
//! Semi → Auto → Pipeline → Off. Lit when on, pulsing when Auto or Pipeline.

use crate::conductor::Mode;

use super::look::{accent_dim, glow, line_strong};
use super::*;

/// LABELS_C.
fn label(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "Conductor: Off",
        Mode::Semi => "Conductor: Semi",
        Mode::Auto => "Conductor: Auto",
        Mode::Pipeline => "Conductor: Pipeline",
    }
}

/// TITLES_C.
fn title(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "Auto-dispatch is off. Click to have the conductor send approved (To do) cards to free agents.",
        Mode::Semi => "Semi: free agents get the next To-do card automatically. Click for Auto (also approves Proposed).",
        Mode::Auto => "Auto: also approves Proposed → To do to keep agents fed. Click for Pipeline.",
        Mode::Pipeline => "Pipeline: cards flow To do → Build → Test → Review → Done, each stage handed to a free agent. Click to stop (Off).",
    }
}

/// --accent, and --accent-ink on it.
const ACCENT: u32 = 0xf4f4f5;
const ACCENT_INK: u32 = 0x050506;

/// The .kb-tool button.
pub(super) fn conductor_btn(mode: Mode, cx: &mut Context<Inbox>) -> AnyElement {
    let on = mode != Mode::Off;
    let btn = div()
        .id("kb-conductor")
        .flex_none()
        .h(px(24.0))
        .px(px(9.0))
        .flex()
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .map(|d| {
            if on {
                d.border_color(color(ACCENT)).bg(color(ACCENT)).text_color(color(ACCENT_INK))
            } else {
                d.border_color(line_strong()).bg(color(RAISE)).text_color(color(INK_2)).hover(|b| b.border_color(accent_dim()).text_color(color(ACCENT)).bg(glow()))
            }
        })
        .tooltip(tip(title(mode)))
        .on_click(cx.listener(move |this, _, _, cx| {
            let next = this.conductor.mode.next();
            this.conductor_set(next, cx);
        }))
        .child(label(mode));
    if mode != Mode::Auto && mode != Mode::Pipeline {
        return btn.into_any_element();
    }
    // kb-cond-pulse: a 3px white ring breathing in and out over 1.8 s.
    btn.with_animation("kb-cond-pulse", Animation::new(Duration::from_millis(1800)).repeat(), |d, t| {
        let k = (1.0 - (t * std::f32::consts::TAU).cos()) / 2.0;
        d.shadow(vec![BoxShadow { color: white(0.28 * k), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(3.0 * k), inset: false }])
    })
    .into_any_element()
}
