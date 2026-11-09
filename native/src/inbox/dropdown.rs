//! The app's drop-down (selectmenu.ts, selectmenu.css): the floating list the
//! Home composer's chips open in place of a Windows popup.

use super::*;

/// Which chip's list is open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pick {
    Dir,
    Cli,
    Count,
    /// Settings → New agents start as.
    DefaultCli,
}

pub(super) struct Popup {
    pub pick: Pick,
    pub at: Point<Pixels>,
}

/// What choosing a line sets.
#[derive(Clone)]
enum Val {
    /// None: "Choose a folder…", asked for on Start.
    Dir(Option<String>),
    Cli(Choice),
    Count(usize),
    DefaultCli(String),
}

/// One line: .sm-opt with its tick, label, mono sub and note.
struct Opt {
    label: String,
    sub: Option<String>,
    note: Option<&'static str>,
    sep: bool,
    on: bool,
    disabled: bool,
    val: Val,
}

impl Inbox {
    fn options(&self, pick: Pick) -> Vec<Opt> {
        let opt = |label: String, on: bool, val: Val| Opt { label, sub: None, note: None, sep: false, on, disabled: false, val };
        if pick == Pick::DefaultCli {
            let now = &self.prefs.default_cli;
            return std::iter::once(opt("Last used".to_string(), now == "last", Val::DefaultCli("last".to_string())))
                .chain(PRESETS.iter().map(|p| opt(p.label.to_string(), now == p.id, Val::DefaultCli(p.id.to_string()))))
                .collect();
        }
        let Some(c) = &self.compose else { return Vec::new() };
        match pick {
            // dirOptions: recent projects with their path, then a way to pick another.
            Pick::Dir => {
                let mut out: Vec<Opt> = self
                    .prefs
                    .recents
                    .iter()
                    .map(|d| Opt { sub: Some(presets::short_path(d)), ..opt(folder_name(d), c.dir.as_deref().is_some_and(|x| presets::same_folder(x, d)), Val::Dir(Some(d.clone()))) })
                    .collect();
                out.push(Opt { sep: !out.is_empty(), ..opt("Choose a folder…".to_string(), c.dir.is_none(), Val::Dir(None)) });
                out
            }
            Pick::Cli => {
                let mut out: Vec<Opt> = PRESETS
                    .iter()
                    .map(|p| {
                        let missing = c.missing.contains(p.id);
                        Opt { note: missing.then_some("Not installed"), disabled: missing, ..opt(p.label.to_string(), c.cli == Choice::Preset(p.id), Val::Cli(Choice::Preset(p.id))) }
                    })
                    .collect();
                out.push(opt("Custom…".to_string(), c.cli == Choice::Custom, Val::Cli(Choice::Custom)));
                out
            }
            Pick::Count => (1..=3).map(|n| opt(count_label(n), c.count == n, Val::Count(n))).collect(),
            Pick::DefaultCli => Vec::new(),
        }
    }

    fn choose(&mut self, val: Val, window: &mut Window, cx: &mut Context<Self>) {
        self.popup = None;
        if let Val::DefaultCli(id) = val {
            self.prefs.default_cli = id;
            self.prefs.save();
            cx.notify();
            return;
        }
        let Some(c) = &mut self.compose else { return };
        match val {
            Val::DefaultCli(_) => {}
            Val::Dir(dir) => c.dir = dir,
            Val::Cli(choice) => {
                c.cli = choice;
                if choice == Choice::Custom {
                    c.custom.update(cx, |input, cx| input.focus(window, cx));
                }
            }
            Val::Count(n) => c.count = n,
        }
        cx.notify();
    }

    pub(super) fn popup_view(&self, popup: &Popup, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let lines = self.options(popup.pick).into_iter().enumerate().map(|(i, o)| {
            let Opt { label, sub, note, sep, on, disabled, val } = o;
            let line = div()
                .id(("sm-opt", i))
                .flex()
                .items_center()
                .gap(px(8.0))
                .min_h(px(34.0))
                .pl(px(8.0))
                .pr(px(12.0))
                .py(px(6.0))
                .rounded(px(9.0))
                .text_color(color(if disabled { INK_3 } else { INK }))
                .child(div().flex_none().size(px(16.0)).flex().items_center().justify_center().when(on, |t| t.child(icon("check", 14.0, color(INK)))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(div().truncate().when(on, |l| l.font_weight(FontWeight::SEMIBOLD)).child(label))
                        .children(sub.map(|s| mono().truncate().text_size(px(11.5)).text_color(color(INK_3)).child(s))),
                )
                .children(note.map(|n| div().flex_none().ml(px(12.0)).text_size(px(11.5)).text_color(color(INK_3)).child(n)))
                .when(!disabled, |line| line.hover(|l| l.bg(white(0.08))).on_click(cx.listener(move |this, _, window, cx| this.choose(val.clone(), window, cx))));
            div().when(sep, |d| d.child(div().h(px(1.0)).mx(px(8.0)).my(px(5.0)).bg(white(0.08)))).child(line)
        });
        let close = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            this.popup = None;
            cx.notify();
        });
        div().absolute().inset_0().on_mouse_down(MouseButton::Left, close).child(
            deferred(
                anchored().position(popup.at).snap_to_window().child(
                    div()
                        .id("sm-pop")
                        .min_w(px(220.0))
                        .max_w(px(360.0))
                        .p(px(5.0))
                        .rounded(px(14.0))
                        .bg(color(0x16171b))
                        .shadow(vec![
                            BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(22.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false },
                            BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.4), offset: point(px(0.0), px(2.0)), blur_radius: px(6.0), spread_radius: px(0.0), inset: false },
                            ring(white(0.1)),
                        ])
                        .text_size(px(13.5))
                        .line_height(relative(1.3))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .children(lines),
                ),
            )
            .with_priority(1),
        )
    }
}

impl Inbox {
    /// selectmenu.ts: a list opens under its button, 6px below, left edges
    /// lined up; the same button again closes it.
    pub(super) fn open_popup(&mut self, pick: Pick, under: Bounds<Pixels>, cx: &mut Context<Self>) {
        self.popup = if self.popup.as_ref().is_some_and(|p| p.pick == pick) { None } else { Some(Popup { pick, at: point(under.left(), under.bottom() + px(6.0)) }) };
        cx.notify();
    }
}

/// Where a list button was last drawn, so its list opens under it.
pub(super) type Spot = std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>;

/// An invisible layer over a button that keeps its Spot current.
pub(super) fn track(spot: &Spot) -> impl IntoElement + use<> {
    let spot = spot.clone();
    canvas(move |bounds, _, _| spot.set(bounds), |_, _, _, _| {}).absolute().size_full()
}

/// The last path segment, for a project's name (folderName).
pub(super) fn folder_name(dir: &str) -> String {
    let trimmed = dir.trim_end_matches(['\\', '/']);
    trimmed.rsplit(['\\', '/']).next().filter(|s| !s.is_empty()).unwrap_or(dir).to_string()
}

pub(super) fn count_label(n: usize) -> String {
    if n == 1 { "1 agent".to_string() } else { format!("{n} agents, same job") }
}
