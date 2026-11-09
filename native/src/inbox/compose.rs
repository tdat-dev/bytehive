//! Home's composer (composerHTML and start() in inboxstart.ts, .st-compose in
//! inbox.css): say what an agent should do, pick the folder, the CLI and how
//! many, and Start. Enter starts, Shift+Enter is a new line.

use super::dropdown::{Pick, Spot, count_label, folder_name, track};
use super::*;
use gpui_kit::component::Sizable as _;

pub(super) struct Compose {
    pub job: Entity<TextareaState>,
    pub custom: Entity<InputState>,
    /// None: "Choose a folder…", asked for on Start.
    pub dir: Option<String>,
    pub cli: Choice,
    pub count: usize,
    pub missing: HashSet<&'static str>,
    pub error: Option<String>,
    /// Where each chip was last drawn, so its list opens under it (by Pick).
    pub spots: [Spot; 3],
    _enter: Subscription,
}

/// The FOLDER_SVG chip glyph and the select chevron (selectmenu.ts).
const FOLDER: &str = "folder";

/// The CLI to offer first: the last one used, if it is still installed.
pub(super) fn last_choice(prefs: &Prefs, missing: &HashSet<&'static str>) -> Choice {
    // Settings → New agents start as: a CLI, or the last one used.
    let last = if prefs.default_cli == "last" { prefs.last_cli.clone().unwrap_or_else(|| "claude".to_string()) } else { prefs.default_cli.clone() };
    if last.starts_with("custom:") {
        return Choice::Custom;
    }
    PRESETS
        .iter()
        .find(|p| p.id == last && !missing.contains(p.id))
        .or_else(|| PRESETS.iter().find(|p| !missing.contains(p.id)))
        .map(|p| Choice::Preset(p.id))
        .unwrap_or(Choice::Custom)
}

impl Inbox {
    /// Make the composer the first time Home shows; keep what was typed after.
    pub(super) fn ensure_compose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let recent = self.prefs.recents.first().cloned();
        if let Some(c) = &mut self.compose {
            // A folder that left the recents falls back to the newest one.
            if c.dir.as_ref().is_none_or(|d| !self.prefs.recents.iter().any(|r| presets::same_folder(r, d))) {
                c.dir = recent;
            }
            return;
        }
        let missing: HashSet<&'static str> = PRESETS.iter().filter(|p| !presets::installed(p.program)).map(|p| p.id).collect();
        let cli = last_choice(&self.prefs, &missing);
        let job = cx.new(|cx| TextareaState::new(window, cx).placeholder("What should an agent do?").auto_grow(3, 12).submit_on_enter(true));
        // Enter starts, Shift+Enter is a new line, as in a chat box.
        let enter = cx.subscribe_in(&job, window, |this, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { secondary: false, shift: false } = event {
                this.compose_start(window, cx);
            }
        });
        let custom_value = self.prefs.last_custom.clone();
        let custom = cx.new(|cx| InputState::new(window, cx).placeholder("Command, e.g. ollama run llama3.1").default_value(custom_value));
        job.update(cx, |input, cx| input.focus(window, cx));
        crate::keys::set_active(false);
        self.compose = Some(Compose { job, custom, dir: recent, cli, count: self.prefs.default_count, missing, error: None, spots: Default::default(), _enter: enter });
    }

    pub(super) fn compose_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &self.compose else { return };
        let (program, args, remembered) = match c.cli {
            Choice::Preset(id) => match Self::preset_command(id, self.prefs.skip_perms) {
                Some((p, a)) => (p, a, id.to_string()),
                None => return,
            },
            Choice::Custom => {
                let typed = c.custom.read(cx).value().trim().to_string();
                match Self::custom_command(&typed) {
                    Some((p, a)) => (p, a, format!("custom:{typed}")),
                    None => {
                        c.custom.update(cx, |input, cx| input.focus(window, cx));
                        return;
                    }
                }
            }
        };
        let job = c.job.read(cx).value().trim().to_string();
        let count = c.count;
        let Some(dir) = c.dir.clone() else {
            // "Choose a folder…": ask now, start once it is picked.
            let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose the project folder".into()) });
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(Ok(Some(paths))) = picked.await {
                    if let Some(path) = paths.first() {
                        let dir = path.display().to_string();
                        let _ = this.update_in(cx, |this, window, cx| this.compose_launch(Launch { program, args, dir, count, job, remembered, quiet: false }, window, cx));
                    }
                }
            })
            .detach();
            return;
        };
        self.compose_launch(Launch { program, args, dir, count, job, remembered, quiet: false }, window, cx);
    }

    fn compose_launch(&mut self, l: Launch, window: &mut Window, cx: &mut Context<Self>) {
        let dir = l.dir.clone();
        let result = self.launch(l, window, cx);
        if let Some(c) = &mut self.compose {
            match result {
                Ok(()) => {
                    c.error = None;
                    c.dir = Some(dir);
                    c.job.update(cx, |input, cx| input.set_value("", window, cx));
                }
                Err(e) => c.error = Some(e),
            }
        }
        cx.notify();
    }

    pub(super) fn compose_view(&self, c: &Compose, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = c.job.focus_handle(cx).contains_focused(window, cx);
        let open = self.popup.as_ref().map(|p| p.pick);
        let chip = |id: &'static str, pick: Pick, glyph: Option<&'static str>, label: String, tooltip: Option<&'static str>| {
            let lit = open == Some(pick);
            div()
                .id(id)
                .relative()
                .flex()
                .items_center()
                .gap(px(6.0))
                .h(px(32.0))
                .px(px(10.0))
                .rounded_full()
                .bg(white(if lit { 0.12 } else { 0.06 }))
                .text_size(px(13.0))
                .text_color(color(if lit { INK } else { INK_2 }))
                .hover(|chip| chip.bg(white(0.1)).text_color(color(INK)))
                .when_some(tooltip.filter(|_| !lit), |chip, t| chip.tooltip(tip(t)))
                .on_mouse_down(MouseButton::Left, {
                    let spot = c.spots[pick as usize].clone();
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| this.open_popup(pick, spot.get(), cx))
                })
                .child(track(&c.spots[pick as usize]))
                .when_some(glyph, |chip, g| chip.child(icon(g, 15.0, color(if lit { INK } else { INK_2 }))))
                .child(div().min_w_0().truncate().child(label))
                .child(icon("chev-down", 10.0, color(INK_2)).opacity(0.7))
        };
        let cli_label = match c.cli {
            Choice::Preset(id) => presets::preset(id).map(|p| p.label.to_string()).unwrap_or_default(),
            Choice::Custom => "Custom…".to_string(),
        };
        let dir_label = c.dir.as_deref().map(folder_name).unwrap_or_else(|| "Choose a folder…".to_string());
        div()
            .id("st-compose")
            .w_full()
            .rounded(px(20.0))
            .bg(color(PANEL))
            .shadow(vec![
                BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(30.0)), blur_radius: px(80.0), spread_radius: px(0.0), inset: false },
                ring(white(if focused { 0.28 } else { 0.1 })),
            ])
            .child(
                div()
                    .id("st-job")
                    .min_h(px(92.0))
                    .pt(px(20.0))
                    .px(px(22.0))
                    .pb(px(8.0))
                    .text_size(px(17.0))
                    .line_height(relative(1.5))
                    .text_color(color(INK))
                    .child(Textarea::new(&c.job).appearance(false)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .pt(px(10.0))
                    .pr(px(12.0))
                    .pb(px(12.0))
                    .pl(px(14.0))
                    .child(chip("st-dir", Pick::Dir, Some(FOLDER), dir_label, Some("The folder it works in")))
                    .child(chip("st-cli", Pick::Cli, None, cli_label, None))
                    .when(c.cli == Choice::Custom, |bar| {
                        bar.child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w(px(200.0))
                                .items_center()
                                .h(px(32.0))
                                .px(px(12.0))
                                .rounded_full()
                                .bg(hsla(0.0, 0.0, 0.0, 0.35))
                                .shadow(vec![ring(white(0.09))])
                                .font_family("Geist Mono")
                                .text_size(px(13.0))
                                .child(Input::new(&c.custom).appearance(false).with_size(px(13.0 / 0.875))),
                        )
                    })
                    .child(chip("st-count", Pick::Count, None, count_label(c.count), None))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("st-go")
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .h(px(36.0))
                            .pl(px(16.0))
                            .pr(px(12.0))
                            .rounded_full()
                            .bg(color(0xffffff))
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(0x000000))
                            .hover(|b| b.bg(color(0xe9e9ec)))
                            .on_click(cx.listener(|this, _, window, cx| this.compose_start(window, cx)))
                            .child("Start")
                            .child(mono().text_size(px(11.0)).px(px(5.0)).py(px(1.0)).rounded(px(5.0)).shadow(vec![ring(hsla(0.0, 0.0, 0.0, 0.2))]).text_color(color(0x555555)).font_weight(FontWeight::NORMAL).child("↵")),
                    ),
            )
            .children(c.error.clone().map(|e| div().px(px(22.0)).pb(px(14.0)).text_size(px(13.0)).text_color(color(DANGER)).child(e)))
    }
}
