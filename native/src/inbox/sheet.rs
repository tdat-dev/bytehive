//! New agent, as today's inboxnew.ts builds it.

use super::*;
use gpui_kit::component::Sizable as _;

impl Inbox {
    pub(super) fn open_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // No project yet: Home is where one is opened, as today's app says.
        let Some(dir) = self.project_dir() else {
            self.open_home(window, cx);
            return;
        };
        let missing: HashSet<&'static str> = PRESETS.iter().filter(|p| !presets::installed(p.program)).map(|p| p.id).collect();
        // The CLI it starts on is the last one used, if still installed.
        let choice = compose::last_choice(&self.prefs, &missing);
        let task = cx.new(|cx| TextareaState::new(window, cx).placeholder("Fix the flaky upload test and add a regression test. Leave empty to give it a job later.").auto_grow(4, 10));
        let custom_value = self.prefs.last_custom.clone();
        let custom = cx.new(|cx| InputState::new(window, cx).placeholder("Command, e.g. ollama run llama3.1").default_value(custom_value));
        task.update(cx, |input, cx| input.focus(window, cx));
        crate::keys::set_active(false);
        let skip_perms = self.prefs.skip_perms;
        self.sheet = Some(Sheet { task, choice, custom, count: 1, skip_perms, dir, missing, saved: false, error: None });
        cx.notify();
    }

    pub(super) fn close_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sheet = None;
        if let Some(index) = self.fleet.focused {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    /// What the sheet would start: program, args, and how it is remembered.
    pub(super) fn sheet_command(&self, sheet: &Sheet, cx: &App) -> Option<(String, Vec<String>, String)> {
        match sheet.choice {
            Choice::Preset(id) => Self::preset_command(id, sheet.skip_perms).map(|(p, a)| (p, a, id.to_string())),
            Choice::Custom => {
                let typed = sheet.custom.read(cx).value().trim().to_string();
                Self::custom_command(&typed).map(|(p, a)| (p, a, format!("custom:{typed}")))
            }
        }
    }

    pub(super) fn start_from_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sheet) = &self.sheet else { return };
        let Some((program, args, remembered)) = self.sheet_command(sheet, cx) else {
            if let Some(sheet) = &mut self.sheet {
                sheet.error = Some("Type the command to run, e.g. ollama run llama3.1".to_string());
            }
            cx.notify();
            return;
        };
        let job = sheet.task.read(cx).value().trim().to_string();
        let (count, dir) = (sheet.count, sheet.dir.clone());
        self.prefs.skip_perms = sheet.skip_perms;
        match self.launch(Launch { program, args, dir, count, job, remembered }, window, cx) {
            Ok(()) => self.sheet = None,
            Err(e) => {
                if let Some(sheet) = &mut self.sheet {
                    sheet.error = Some(e);
                }
            }
        }
        cx.notify();
    }

    /// Save as preset: keep this setup to start again later.
    pub(super) fn save_preset(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &self.sheet else { return };
        let Some((program, _, remembered)) = self.sheet_command(sheet, cx) else { return };
        let label = match sheet.choice {
            Choice::Preset(id) => presets::preset(id).map(|p| p.label.to_string()).unwrap_or(program),
            Choice::Custom => remembered.trim_start_matches("custom:").to_string(),
        };
        let project = crate::fleet::project_of(&sheet.dir);
        let count = sheet.count;
        let name = format!("{}{label} · {project}", if count > 1 { format!("{count}× ") } else { String::new() });
        self.prefs.templates.push(Template { name, cli: remembered, count, dir: sheet.dir.clone(), skip_perms: sheet.skip_perms });
        self.prefs.save();
        if let Some(sheet) = &mut self.sheet {
            sheet.saved = true;
        }
        cx.notify();
    }

    /// inboxnew.ts and the .inbox-modal rules in inbox.css.
    pub(super) fn sheet_view(&self, sheet: &Sheet, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // The owner keeps each CLI's logo in its chip (today's chips are text only).
        let pill = |id: SharedString, on: bool, missing: bool| {
            div()
                .id(id)
                .h(px(32.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .gap(px(7.0))
                .rounded_full()
                .text_size(px(13.0))
                .when(on, |p| p.bg(color(0xffffff)).text_color(color(0x000000)).font_weight(FontWeight::SEMIBOLD))
                .when(!on, |p| p.text_color(color(INK_2)).shadow(vec![ring(white(0.1))]))
                .when(missing, |p| p.opacity(0.4))
        };
        let mut chips: Vec<AnyElement> = Vec::new();
        for p in PRESETS {
            let missing = sheet.missing.contains(p.id);
            let on = sheet.choice == Choice::Preset(p.id);
            let id = p.id;
            let chip = pill(SharedString::from(format!("cli-{id}")), on, missing).pl(px(6.0)).child(self.mark(p.program, 20.0, 6.0)).child(p.label);
            chips.push(
                if missing {
                    chip.tooltip(move |window, cx| Tooltip::new(format!("{id} is not installed")).build(window, cx))
                } else {
                    chip.on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(sheet) = &mut this.sheet {
                            sheet.choice = Choice::Preset(id);
                        }
                        cx.notify();
                    }))
                }
                .into_any_element(),
            );
        }
        chips.push(
            pill("cli-custom".into(), sheet.choice == Choice::Custom, false)
                .child("Custom…")
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some(sheet) = &mut this.sheet {
                        sheet.choice = Choice::Custom;
                        sheet.custom.update(cx, |input, cx| input.focus(window, cx));
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
        let label = |text: &'static str| div().mb(px(8.0)).text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(color(INK_2)).child(text);
        let button = |id: &'static str, text: String, primary: bool| {
            div()
                .id(id)
                .h(px(36.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(primary, |b| b.bg(color(0xffffff)).text_color(color(0x000000)).hover(|b| b.bg(color(0xe9e9ec))))
                .when(!primary, |b| b.bg(white(0.06)).text_color(color(INK_2)).hover(|b| b.bg(white(0.12)).text_color(color(INK))))
                .child(text)
        };
        let seg = (1..=3usize).map(|n| {
            let on = sheet.count == n;
            mono()
                .id(("count", n))
                .w(px(36.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .text_size(px(13.0))
                .text_color(color(if on { INK } else { INK_2 }))
                .when(on, |s| s.bg(white(0.14)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(sheet) = &mut this.sheet {
                        sheet.count = n;
                    }
                    cx.notify();
                }))
                .child(n.to_string())
        });
        let field = |content: Div| content.rounded(px(12.0)).bg(hsla(0.0, 0.0, 0.0, 0.35)).shadow(vec![ring(white(0.09))]);
        let project = crate::fleet::project_of(&sheet.dir);
        let start = if sheet.count > 1 { format!("Start {} agents", sheet.count) } else { "Start agent".to_string() };
        let checked = sheet.skip_perms;
        div()
            .id("sheet-scrim")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .justify_center()
            .items_start()
            .pt(relative(0.14))
            .px(px(16.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_sheet(window, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                if k.key == "escape" {
                    this.close_sheet(window, cx);
                } else if k.key == "enter" && k.modifiers.control {
                    // Ctrl+Enter starts from inside the text box; Enter is a newline.
                    this.start_from_sheet(window, cx);
                }
            }))
            .child(
                div()
                    .id("sheet")
                    .w(px(560.0))
                    .h_auto()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .pt(px(22.0))
                    .px(px(22.0))
                    .pb(px(18.0))
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(4.0))
                                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("New agent"))
                                    .child(
                                        div()
                                            .id("sheet-where")
                                            .flex()
                                            .gap(px(4.0))
                                            .min_w_0()
                                            .text_size(px(12.5))
                                            .text_color(color(INK_3))
                                            .child(format!("In {project} ·"))
                                            .child(mono().min_w_0().truncate().child(sheet.dir.clone())),
                                    ),
                            )
                            .child(
                                div()
                                    .id("sheet-x")
                                    .size(px(30.0))
                                    .flex_none()
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(white(0.05))
                                    .hover(|x| x.bg(white(0.1)))
                                    .on_click(cx.listener(|this, _, window, cx| this.close_sheet(window, cx)))
                                    .child(icon("x", 14.0, color(INK_3))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(label("What should it do?"))
                            .child(field(div().px(px(4.0)).py(px(4.0)).text_size(px(14.0)).line_height(relative(1.5)).min_h(px(96.0))).child(Textarea::new(&sheet.task).appearance(false))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(label("Agent"))
                            .child(div().flex().flex_wrap().gap(px(6.0)).children(chips))
                            .when(sheet.choice == Choice::Custom, |f| f.child(field(div().mt(px(8.0)).h(px(36.0)).px(px(12.0)).flex().items_center().rounded(px(10.0)).font_family("Geist Mono").text_size(px(13.5))).child(Input::new(&sheet.custom).appearance(false).with_size(px(13.5 / 0.875))))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(16.0))
                            .child(div().flex().flex_col().child(label("How many")).child(div().flex().gap(px(2.0)).p(px(3.0)).rounded_full().bg(white(0.05)).children(seg)))
                            .when(sheet.count > 1, |row| row.child(div().mb(px(6.0)).text_size(px(12.5)).text_color(color(INK_3)).child("They all get the same job. Compare their changes in Review and keep the best."))),
                    )
                    .child(
                        div()
                            .id("skip")
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_size(px(13.0))
                            .text_color(color(INK_2))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(sheet) = &mut this.sheet {
                                    sheet.skip_perms = !sheet.skip_perms;
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(16.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(checked, |b| b.bg(color(0xffffff)).child(icon("check", 12.0, color(0x000000))))
                                    .when(!checked, |b| b.shadow(vec![ring(white(0.3))])),
                            )
                            .child("Don't ask before running commands or editing files"),
                    )
                    .when_some(sheet.error.clone(), |s, error| s.child(div().text_size(px(12.5)).text_color(color(DANGER)).child(error)))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .mt(px(4.0))
                            .child(
                                div()
                                    .id("save-preset")
                                    .h(px(36.0))
                                    .px(px(16.0))
                                    .flex()
                                    .items_center()
                                    .rounded_full()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(INK_3))
                                    .when(!sheet.saved, |b| b.hover(|b| b.bg(white(0.06)).text_color(color(INK))).on_click(cx.listener(|this, _, _, cx| this.save_preset(cx))))
                                    .child(if sheet.saved { "Saved" } else { "Save as preset" }),
                            )
                            .child(div().flex_1())
                            .child(button("cancel", "Cancel".to_string(), false).on_click(cx.listener(|this, _, window, cx| this.close_sheet(window, cx))))
                            .child(button("start", start, true).tooltip(|window, cx| Tooltip::new("Start (Ctrl+Enter)").build(window, cx)).on_click(cx.listener(|this, _, window, cx| this.start_from_sheet(window, cx)))),
                    ),
            )
    }
}
