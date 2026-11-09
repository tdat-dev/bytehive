//! Settings → Remote, the phone dashboard's rows in today's words (index.html
//! section data-sec="fleet"; dashboard.ts paintDash, paintPassword,
//! showPasswordForm, removePassword). The Port row is the earlier native
//! app's: this app may run beside the other one, which holds 8477.

use gpui_kit::component::Sizable as _;

use super::rows::{button, row, text, toggle};
use super::sections::section;
use super::*;
use crate::inbox::remote::password_notes;

/// .input: a dark box with a faint ring (modal.css).
fn field(input: &Entity<InputState>) -> Div {
    div()
        .flex()
        .items_center()
        .w_full()
        .max_w(px(320.0))
        .h(px(36.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .bg(hsla(0.0, 0.0, 0.0, 0.3))
        .shadow(vec![ring(white(0.09))])
        .text_size(px(13.0))
        .child(Input::new(input).appearance(false).with_size(px(13.0 / 0.875)))
}

impl Inbox {
    /// Settings opened: the password boxes put away, the Port box filled in.
    pub(super) fn remote_open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = &mut self.remote;
        r.form = None;
        r.confirm_clear = false;
        r.note = None;
        let port = cx.new(|cx| InputState::new(window, cx).default_value(self.prefs.remote_port.to_string()));
        let sub = cx.subscribe_in(&port, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.remote_commit_port(window, cx);
            }
        });
        self.remote.port_input = Some((port, sub));
    }

    fn remote_commit_port(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((input, _)) = &self.remote.port_input else { return };
        let input = input.clone();
        match input.read(cx).value().trim().parse::<u16>() {
            Ok(port) if (1024..=65534).contains(&port) => {
                if port != self.prefs.remote_port {
                    self.prefs.remote_port = port;
                    self.prefs.save();
                    if self.prefs.remote_on {
                        self.remote_apply(cx);
                    }
                }
            }
            _ => {
                let port = self.prefs.remote_port.to_string();
                input.update(cx, |i, cx| i.set_value(port, window, cx));
            }
        }
        cx.notify();
    }

    fn remote_toggle(&mut self, cx: &mut Context<Self>) {
        if self.remote.busy {
            return;
        }
        self.prefs.remote_on = !self.prefs.remote_on;
        self.prefs.save();
        self.remote_apply(cx);
    }

    fn remote_toggle_lan(&mut self, cx: &mut Context<Self>) {
        // Turned off without a password, it can't come back on.
        if self.remote.busy || (!self.remote.has_password && !self.prefs.remote_lan) {
            return;
        }
        self.prefs.remote_lan = !self.prefs.remote_lan;
        self.prefs.save();
        if self.prefs.remote_on {
            // bind on the new interface
            self.remote_apply(cx);
        }
        cx.notify();
    }

    pub(super) fn remote_section(&self, cx: &mut Context<Self>) -> Div {
        let (r, p) = (&self.remote, &self.prefs);
        let where_ = match &r.error {
            Some(e) => e.clone(),
            None if r.info.running => {
                let urls = if r.info.urls.is_empty() { format!("http://127.0.0.1:{}", r.info.port) } else { r.info.urls.join("  ·  ") };
                format!("Open on any device on your network: {urls}")
            }
            None => "Off: turn on to view the fleet from your phone.".to_string(),
        };
        let (pw_note, lan_note) = password_notes(r.has_password, p.remote_port);
        let pw_note = r.note.clone().unwrap_or(pw_note);
        let lan_can = r.has_password || p.remote_lan;

        let mut pw_row = row("Password", pw_note).child(button("dash-pw-set", if r.has_password { "Change" } else { "Set password" }).on_click(cx.listener(|this, _, window, cx| {
            if !this.remote.busy {
                this.remote_open_form(window, cx);
            }
        })));
        if r.has_password {
            pw_row = pw_row.child(button("dash-pw-clear", "Remove").on_click(cx.listener(|this, _, _, cx| {
                this.remote.confirm_clear = true;
                cx.notify();
            })));
        }
        let form = r.form.as_ref().map(|f| {
            let title = if r.has_password { "Change the password" } else { "Set a password" };
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .py(px(13.0))
                .border_b_1()
                .border_color(hairline())
                .child(text(title, "Anyone opening the dashboard signs in with it, this computer included. At least 10 characters."))
                .child(field(&f.first))
                .child(field(&f.again))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(div().flex_1().min_h(px(12.0)).text_size(px(12.0)).text_color(color(0xf87171)).children(f.error.clone()))
                        .child(button("dash-pw-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                            this.remote.form = None;
                            cx.notify();
                        })))
                        .child(button("dash-pw-save", "Save password").when(r.busy, |b| b.opacity(0.5)).on_click(cx.listener(|this, _, window, cx| this.remote_save_password(window, cx)))),
                )
        });
        let port = r.port_input.as_ref().map(|(input, _)| {
            row("Port", "Where the page listens; its live terminal takes the next port too.").child(div().w(px(96.0)).flex_none().child(field(input).font_family("Geist Mono")))
        });

        section("Remote")
            .child(toggle("dash-on", p.remote_on, "Remote fleet dashboard", where_).when(r.busy, |t| t.opacity(0.6)).on_click(cx.listener(|this, _, _, cx| this.remote_toggle(cx))))
            .child(pw_row)
            .children(form)
            .child(toggle("dash-lan", p.remote_lan, "Allow other devices (LAN)", lan_note).when(!lan_can || r.busy, |t| t.opacity(0.5)).on_click(cx.listener(|this, _, _, cx| this.remote_toggle_lan(cx))))
            .children(port)
    }

    /// confirmModal: "Remove the dashboard password?" before it goes.
    pub(super) fn remote_confirm(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if !self.remote.confirm_clear {
            return None;
        }
        let lan_on = self.remote.info.running && self.prefs.remote_lan;
        let message = if lan_on {
            "Other devices can't connect without one, so the dashboard goes back to this computer only."
        } else {
            "The dashboard on this computer opens without signing in. Other devices can't connect until you set one again."
        };
        let pill = |id: &'static str, label: &'static str, danger: bool| {
            div()
                .id(id)
                .h(px(36.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(danger, |b| b.bg(color(DANGER)).text_color(color(GROUND)).hover(|b| b.opacity(0.9)))
                .when(!danger, |b| b.text_color(color(INK_2)).hover(|b| b.bg(white(0.06)).text_color(color(INK))))
                .child(label)
        };
        Some(div().absolute().inset_0().bg(hsla(0.0, 0.0, 0.0, 0.55)).flex().items_center().justify_center().child(
            div()
                .w(px(440.0))
                .flex()
                .flex_col()
                .gap(px(14.0))
                .p(px(22.0))
                .rounded(px(18.0))
                .bg(color(PANEL))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Remove the dashboard password?"))
                .child(div().text_size(px(14.0)).line_height(relative(1.5)).text_color(color(INK_2)).child(message))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(pill("dash-clear-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                            this.remote.confirm_clear = false;
                            cx.notify();
                        })))
                        .child(pill("dash-clear-ok", "Remove", true).on_click(cx.listener(|this, _, _, cx| this.remote_clear_password(cx)))),
                ),
        ))
    }
}
