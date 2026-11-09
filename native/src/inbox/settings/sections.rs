//! What Settings holds, section by section, in today's words (index.html
//! #settingsModal). Only rows whose setting does something here yet are
//! shown; the rest come with their features.

use super::rows::{button, row, select, seg, seg_button, step_button, stepper, toggle};
use super::*;

/// .sec: a heading over its rows.
pub(super) fn section(title: &'static str) -> Div {
    div().mb(px(28.0)).child(div().mb(px(12.0)).text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(color(INK_3)).child(title))
}

impl Inbox {
    pub(super) fn sections(&self, s: &Settings, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = &self.prefs;
        let open = self.popup.as_ref().is_some_and(|x| x.pick == Pick::DefaultCli);
        let cli = if p.default_cli == "last" { "Last used".to_string() } else { presets::preset(&p.default_cli).map(|x| x.label.to_string()).unwrap_or_else(|| p.default_cli.clone()) };
        let spot = s.cli_spot.clone();

        let agents = section("Agents")
            .child(
                row("New agents start as", "The CLI New agent picks for you. \"Last used\" remembers your last choice.").child(
                    select("pref-cli", cli, open)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.open_popup(Pick::DefaultCli, spot.get(), cx)))
                        .child(track(&s.cli_spot)),
                ),
            )
            .child(row("Agents per job", "How many New agent offers by default. Two or three is a race: compare their changes, keep the best.").child(seg().children((1..=3).map(|n| {
                seg_button(("pref-count", n), n.to_string(), p.default_count == n).on_click(cx.listener(move |this, _, _, cx| {
                    this.prefs.default_count = n;
                    this.prefs.save();
                    cx.notify();
                }))
            }))))
            .child(
                toggle("pref-ask", !p.skip_perms, "Ask before running commands or editing files", "Off starts agents with permission prompts skipped (Claude --dangerously-skip-permissions, Codex/Gemini --yolo). Only turn off on code you trust.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.skip_perms = !this.prefs.skip_perms;
                    this.prefs.save();
                    cx.notify();
                })),
            )
            .child(
                toggle("pref-worktree", p.worktree, "Own branch for every agent", "Each agent works in its own git worktree, so Changes, Compare and merge see exactly what it did. Needs the project to be a git repo.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.worktree = !this.prefs.worktree;
                    this.prefs.save();
                    cx.notify();
                })),
            )
            .child(row("Wait before giving the job", "Time a new CLI gets to reach its prompt before ByteHive types the job in. Raise it if jobs get lost on a slow machine.").child(stepper(
                format!("{} s", p.job_delay),
                step_button("delay-less", "−", p.job_delay > presets::DELAY_MIN).on_click(cx.listener(|this, _, _, cx| this.set_delay(-1, cx))),
                step_button("delay-more", "+", p.job_delay < presets::DELAY_MAX).on_click(cx.listener(|this, _, _, cx| this.set_delay(1, cx))),
            )));

        let inbox = section("Inbox")
            .child(
                toggle("pref-notify", p.notify_needs, "Notify me when an agent needs me", "A Windows notification that says what it wants (\u{201c}Ana wants to run npm test\u{201d}), only while ByteHive is in the background.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.notify_needs = !this.prefs.notify_needs;
                    this.prefs.save();
                    cx.notify();
                })),
            )
            .child(
                toggle("pref-notify-done", p.notify_done, "Tell me when an agent finishes", "A Windows notification when an agent is done, has changes to review, or stops with an error. Not for quick replies, and not for the agent you are looking at. Click it to open that agent.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.notify_done = !this.prefs.notify_done;
                    this.prefs.save();
                    cx.notify();
                })),
            )
            .child(
                toggle("pref-handover", p.auto_handover, "Hand over when an agent runs out of usage", "Another CLI whose plan has room takes the task over at once: an agent already open in the same folder first, else a new one. Off, the agent waits for its limit to reset.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.auto_handover = !this.prefs.auto_handover;
                    this.prefs.save();
                    cx.notify();
                })),
            )
            .child(
                toggle("pref-resume", p.auto_resume, "Carry on when a usage limit resets", "An agent that stopped at its plan's 5-hour or weekly limit is told to carry on once the limit resets, so a task doesn't sit waiting for you to notice.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.auto_resume = !this.prefs.auto_resume;
                    this.prefs.save();
                    cx.notify();
                })),
            );

        let terminal = section("Terminal").child(row("Font size", "Applies live to every agent's terminal.").child(stepper(
            p.font_size.to_string(),
            step_button("font-less", "−", p.font_size > presets::FONT_MIN).on_click(cx.listener(|this, _, window, cx| this.set_font(-1, window, cx))),
            step_button("font-more", "+", p.font_size < presets::FONT_MAX).on_click(cx.listener(|this, _, window, cx| this.set_font(1, window, cx))),
        )));

        let system = section("System")
            .child(
                toggle("pref-restore", p.restore, "Reopen my projects on start", "ByteHive brings back last session's projects and agents, stopped, so nothing heavy launches by itself. Resume the ones you need.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.restore = !this.prefs.restore;
                    this.prefs.save();
                    cx.notify();
                })),
            )
            .child(
                toggle("pref-tray", p.hide_to_tray, "Hide to tray", "Minimizing tucks ByteHive into the system tray; agents keep running.").on_click(cx.listener(|this, _, _, cx| {
                    this.prefs.hide_to_tray = !this.prefs.hide_to_tray;
                    this.prefs.save();
                    this.show_tray();
                    cx.notify();
                })),
            )
            .child(row("Updates", format!("ByteHive {}", crate::updater::current())).child(button("pref-update", if self.updates.checking { "Checking…" } else { "Check for updates" }).on_click(cx.listener(|this, _, _, cx| this.check_updates(false, cx)))))
            .child(row("Reset settings", "Agents, text size, startup and the tray go back to how ByteHive ships. Projects and agents are not touched.").child(button("pref-reset", "Reset").on_click(cx.listener(|this, _, _, cx| {
                if let Some(s) = &mut this.settings {
                    s.resetting = true;
                }
                cx.notify();
            }))));

        let browser = self.browser_section(cx);
        let remote = self.remote_section(cx);
        vec![agents.into_any_element(), inbox.into_any_element(), terminal.into_any_element(), browser.into_any_element(), remote.into_any_element(), system.into_any_element()]
    }

    fn set_delay(&mut self, by: i64, cx: &mut Context<Self>) {
        self.prefs.job_delay = (self.prefs.job_delay as i64 + by).clamp(presets::DELAY_MIN as i64, presets::DELAY_MAX as i64) as u64;
        self.prefs.save();
        cx.notify();
    }

    pub(super) fn set_font(&mut self, by: i32, window: &mut Window, cx: &mut Context<Self>) {
        self.prefs.font_size = (self.prefs.font_size as i32 + by).clamp(presets::FONT_MIN as i32, presets::FONT_MAX as i32) as u32;
        self.prefs.save();
        crate::terminal::set_font_size(self.prefs.font_size);
        // Every terminal measures its cells again on the next frame.
        window.refresh();
        cx.notify();
    }
}
