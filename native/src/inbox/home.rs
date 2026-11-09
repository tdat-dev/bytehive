//! Home, the start screen (inboxstart.ts, .inbox-start and .st-* in
//! inbox.css): a greeting with one live sentence about what waits on you,
//! the composer, a few starter jobs, the agents waiting on you, and recent
//! projects. First run swaps the projects for three steps.

use chrono::Timelike as _;

use super::dropdown::folder_name;
use super::*;

/// starters.ts: a short label, and the job it fills in.
const STARTERS: [(&str, &str); 4] = [
    ("Explain this codebase", "Explain how this codebase is put together, and where to start reading"),
    ("Find and fix a bug", "Find a bug, fix it, and add a test that would have caught it"),
    ("Add missing tests", "Add tests for the code that has none"),
    ("Review the last commit", "Review the last commit and point out anything risky"),
];

const STEPS: [(&str, &str); 3] = [
    ("Choose a folder", "Any project on your disk. Each agent gets its own git branch there, so your checkout stays clean."),
    ("Say what you want done", "In plain words, in the box above. Pick Claude Code, Codex, Gemini or another CLI you have installed."),
    ("Answer when it asks", "When an agent wants to run a command or has a question, it moves to the top of the list. Answer it in its terminal."),
];

/// The page's column (.st-in) and the grid gap of its cards.
const COLUMN: f32 = 760.0;
const GAP: f32 = 10.0;

pub(super) fn greeting(hour: u32) -> &'static str {
    match hour {
        0..5 => "Working late",
        5..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    }
}

/// The one sentence under the greeting: what, if anything, waits on you.
pub(super) fn status_sentence(names: &[&str], states: &[State], first_run: bool) -> String {
    let needs: Vec<&str> = names.iter().zip(states).filter(|(_, s)| waits(**s)).map(|(n, _)| *n).collect();
    let working = states.iter().filter(|s| **s == State::Working).count();
    let review = states.iter().filter(|s| **s == State::Review).count();
    match needs.len() {
        1 => return format!("{} is waiting on you.", needs[0]),
        n if n > 1 => return format!("{n} agents are waiting on you."),
        _ => {}
    }
    if review > 0 {
        return if review == 1 { "One agent has finished and is ready to review.".to_string() } else { format!("{review} agents have finished and are ready to review.") };
    }
    if working > 0 {
        return if working == 1 { "One agent is working. Nothing needs you yet.".to_string() } else { format!("{working} agents are working. Nothing needs you yet.") };
    }
    if first_run { "Pick a folder, say what you want done, and an agent starts on it.".to_string() } else { "Nothing is running. What should an agent do next?".to_string() }
}

/// Cards per row for `repeat(auto-fill, minmax(min, 1fr))`, and their width.
fn grid(inner: f32, min: f32) -> (usize, f32) {
    let cols = (((inner + GAP) / (min + GAP)).floor() as usize).max(1);
    // Floored: a hair too wide and the last card of a row wraps.
    (cols, ((inner - GAP * (cols - 1) as f32) / cols as f32).floor())
}

impl Inbox {
    pub(super) fn open_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.home = true;
        self.sheet = None;
        self.menu = None;
        self.palette = None;
        self.popup = None;
        self.ensure_compose(window, cx);
        if let Some(c) = &self.compose {
            c.job.update(cx, |input, cx| input.focus(window, cx));
        }
        crate::keys::set_active(false);
        cx.notify();
    }

    /// Back to the agents, the one on stage taking the keys again.
    pub(super) fn leave_home(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.home = false;
        self.popup = None;
        if let Some(index) = index.or(self.fleet.focused).filter(|i| *i < self.fleet.seats.len()) {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    /// openProject: make the folder the project, then offer New agent when it has none.
    fn open_project(&mut self, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        self.prefs.open_project(&dir);
        let first = self.fleet.seats.iter().position(|s| presets::same_folder(&s.spec.cwd, &dir));
        self.leave_home(first, window, cx);
        if first.is_none() {
            self.open_sheet(window, cx);
        }
    }

    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose the project folder".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                if let Some(path) = paths.first() {
                    let dir = path.display().to_string();
                    let _ = this.update_in(cx, |this, window, cx| this.open_project(dir, window, cx));
                }
            }
        })
        .detach();
    }

    /// quickTerminal: a shell, no agent, in the newest project.
    fn plain_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.prefs.recents.first().cloned().or_else(|| std::env::var("USERPROFILE").ok()).unwrap_or_else(|| ".".to_string());
        if let Ok(index) = self.fleet.add("powershell.exe", Vec::new(), &dir, None, None, cx) {
            self.leave_home(Some(index), window, cx);
        }
    }

    pub(super) fn home_view(&self, view: &View, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let seats = &self.fleet.seats;
        let states: Vec<State> = view.statuses.iter().map(|s| s.state).collect();
        let names: Vec<&str> = seats.iter().map(|s| s.spec.name.as_str()).collect();
        let first_run = self.prefs.recents.is_empty() && seats.is_empty();
        let sentence = status_sentence(&names, &states, first_run);
        let any_waits = states.iter().any(|s| waits(*s));
        let inner = (f32::from(window.viewport_size().width) - 48.0).clamp(240.0, COLUMN);
        let projects: HashSet<String> = seats.iter().map(|s| s.project()).collect();
        let compose = self.compose.as_ref().map(|c| self.compose_view(c, window, cx));

        let h2 = |text: &'static str| div().mb(px(12.0)).text_size(px(13.0)).font_weight(FontWeight::MEDIUM).text_color(color(INK_3)).child(text);
        let card = |id: ElementId, w: f32| div().id(id).w(px(w)).flex().flex_col().items_start().gap(px(4.0)).p(px(14.0)).rounded(px(14.0)).min_w_0();

        let waiting = seats.iter().enumerate().filter(|(i, _)| states.get(*i).is_some_and(|s| waits(*s))).map(|(i, seat)| {
            let line = row_line(&view.statuses[i], view.worked[i]);
            div()
                .id(("st-w", i))
                .flex()
                .items_center()
                .gap(px(10.0))
                .py(px(12.0))
                .px(px(14.0))
                .rounded(px(14.0))
                .bg(tint(NEEDS, 0.12))
                .shadow(vec![ring(tint(NEEDS, 0.3))])
                .hover(|w| w.shadow(vec![ring(color(NEEDS))]))
                .text_size(px(14.0))
                .text_color(color(INK))
                .on_click(cx.listener(move |this, _, window, cx| this.leave_home(Some(i), window, cx)))
                .child(div().w(px(14.0)).flex_none().child(icon("diamond", 10.0, color(NEEDS))))
                .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(seat.spec.name.clone()))
                .child(div().flex_none().text_size(px(12.5)).text_color(color(INK_3)).child(seat.project()))
                .child(div().flex_1().min_w_0().truncate().text_size(px(13.0)).text_color(color(INK_2)).child(line))
                .child(div().flex_none().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(color(NEEDS)).child("Answer"))
        });
        let waiting: Vec<_> = waiting.collect();

        let live = if first_run {
            let (_, w) = grid(inner, 210.0);
            div().mt(px(40.0)).child(h2("Getting started")).child(div().flex().flex_wrap().gap(px(GAP)).children(STEPS.iter().enumerate().map(|(n, (title, text))| {
                div()
                    .w(px(w))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(16.0))
                    .rounded(px(14.0))
                    .bg(color(PANEL))
                    .shadow(vec![ring(hairline())])
                    .child(mono().size(px(22.0)).rounded_full().flex().items_center().justify_center().bg(white(0.08)).text_size(px(12.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK_2)).child((n + 1).to_string()))
                    .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(*title))
                    .child(div().text_size(px(13.0)).line_height(relative(1.45)).text_color(color(INK_3)).child(*text))
            })))
        } else {
            let (_, w) = grid(inner, 220.0);
            let ic = |glyph: Option<&'static str>, text: &'static str| {
                mono()
                    .size(px(28.0))
                    .mb(px(6.0))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(white(0.06))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(INK_2))
                    .map(|d| match glyph {
                        Some(g) => d.child(icon(g, 15.0, color(INK_2))),
                        None => d.child(text),
                    })
            };
            let rows = self.prefs.recents.iter().enumerate().map(|(i, dir)| {
                let mine: Vec<usize> = (0..seats.len()).filter(|j| presets::same_folder(&seats[*j].spec.cwd, dir)).collect();
                let needs = mine.iter().filter(|j| states.get(**j).is_some_and(|s| waits(*s))).count();
                let n = if needs > 0 {
                    format!("{needs} waiting on you")
                } else if !mine.is_empty() {
                    format!("{} agent{}", mine.len(), if mine.len() == 1 { "" } else { "s" })
                } else {
                    "Not open".to_string()
                };
                let open = dir.clone();
                card(("st-proj", i).into(), w)
                    .bg(color(PANEL))
                    .shadow(vec![ring(hairline())])
                    .hover(|c| c.bg(color(RAISE)))
                    .on_click(cx.listener(move |this, _, window, cx| this.open_project(open.clone(), window, cx)))
                    .child(ic(Some("folder"), ""))
                    .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(folder_name(dir)))
                    .child(mono().max_w_full().truncate().text_size(px(12.0)).text_color(color(INK_3)).child(presets::short_path(dir)))
                    .child(div().mt(px(6.0)).text_size(px(12.5)).text_color(color(if needs > 0 { NEEDS } else { INK_3 })).child(n))
            });
            let add = |id: &'static str, glyph: &'static str, title: &'static str, sub: &'static str| {
                card(id.into(), w)
                    .shadow(vec![ring(white(0.1))])
                    .hover(|c| c.bg(color(RAISE)))
                    .child(ic(None, glyph))
                    .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(title))
                    .child(div().text_size(px(12.0)).text_color(color(INK_3)).child(sub))
            };
            div().mt(px(40.0)).child(h2("Projects")).child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(GAP))
                    .children(rows)
                    .child(add("st-choose", "+", "Open a folder", "Start a new project").on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx))))
                    .child(add("st-shell", "›_", "Plain terminal", "A shell, no agent").on_click(cx.listener(|this, _, window, cx| this.plain_terminal(window, cx)))),
            )
        };

        let starters = STARTERS.iter().enumerate().map(|(i, (label, job))| {
            let job = *job;
            div()
                .id(("st-starter", i))
                .tooltip(tip(job))
                .py(px(7.0))
                .px(px(13.0))
                .rounded_full()
                .text_size(px(13.0))
                .line_height(relative(1.35))
                .text_color(color(INK_2))
                .shadow(vec![ring(hairline())])
                .hover(|s| s.text_color(color(INK)).bg(white(0.04)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(c) = &this.compose {
                        c.job.update(cx, |input, cx| {
                            input.set_value(job, window, cx);
                            input.focus(window, cx);
                        });
                    }
                }))
                .child(*label)
        });

        let back = (!seats.is_empty()).then(|| {
            let n = projects.len();
            div().absolute().top(px(9.0)).left_0().right_0().flex().justify_center().child(
                div()
                    .id("home-resume")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .py(px(8.0))
                    .px(px(13.0))
                    .rounded_full()
                    .bg(color(RAISE))
                    .shadow(float())
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(INK))
                    .hover(|b| b.bg(color(0x1d1e23)))
                    .on_click(cx.listener(|this, _, window, cx| this.leave_home(None, window, cx)))
                    .child(icon("back", 15.0, color(INK)))
                    .child("Back to your agents")
                    .child(mono().text_size(px(11.0)).text_color(color(INK_3)).child(format!("{n} project{}", if n == 1 { "" } else { "s" }))),
            )
        });

        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .child(div().absolute().left(relative(0.5)).ml(px(-900.0)).top(relative(-0.08)).mt(px(-420.0)).w(px(1800.0)).h(px(840.0)).child(img(logo::home_haze()).size_full().object_fit(ObjectFit::Fill)))
            .child(
                div().id("home-page").absolute().inset_0().overflow_y_scroll().child(
                    div().flex().flex_col().items_center().pt(px(64.0)).px(px(24.0)).pb(px(56.0)).child(
                        div()
                            .w(px(inner))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .mb(px(36.0))
                                    .text_size(px(22.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(INK))
                                    .child(
                                        div()
                                            .relative()
                                            .size(px(84.0))
                                            .mt(px(-16.0))
                                            .mb(px(-16.0))
                                            .ml(px(-16.0))
                                            .mr(px(-10.0))
                                            .child(div().absolute().left(px(-33.0)).top(px(-33.0)).size(px(150.0)).child(img(logo::glow()).size_full()))
                                            .child(img(logo::logo()).absolute().inset_0().size(px(84.0))),
                                    )
                                    .child("ByteHive"),
                            )
                            .child(div().text_size(px(40.0)).font_weight(FontWeight::SEMIBOLD).line_height(relative(1.1)).text_color(color(INK)).child(format!("{}.", greeting(chrono::Local::now().hour()))))
                            .child(div().mt(px(10.0)).mb(px(28.0)).text_size(px(17.0)).line_height(relative(1.45)).text_color(color(if any_waits { NEEDS } else { INK_3 })).child(sentence))
                            .children(compose)
                            .child(div().flex().flex_wrap().gap(px(8.0)).mt(px(14.0)).children(starters))
                            .child(
                                div().flex().mt(px(14.0)).child(
                                    div()
                                        .id("st-help")
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .py(px(4.0))
                                        .px(px(2.0))
                                        .text_size(px(13.0))
                                        .text_color(color(INK_3))
                                        .hover(|h| h.text_color(color(INK)))
                                        .child("See everything ByteHive can do")
                                        .child(mono().text_size(px(11.0)).px(px(6.0)).py(px(1.0)).rounded(px(6.0)).shadow(vec![ring(white(0.14))]).child("?")),
                                ),
                            )
                            .when(!waiting.is_empty(), |col| col.child(div().mt(px(40.0)).child(h2("Waiting on you")).child(div().flex().flex_col().gap(px(6.0)).children(waiting))))
                            .child(live),
                    ),
                ),
            )
            .child(div().absolute().top_0().left_0().right_0().h(px(38.0)).window_control_area(WindowControlArea::Drag))
            .children(back)
            .child(div().absolute().top_0().right_0().child(window_buttons()))
    }
}
