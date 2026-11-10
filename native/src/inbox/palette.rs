//! Ctrl K: jump to an agent or run an action (inboxpalette.ts).

use super::*;

/// What a palette line does.
#[derive(Clone, Copy)]
pub(super) enum PalRun {
    Agent(usize),
    NewAgent,
    ResumeAll,
    Sessions,
    Settings,
    Files,
    Memory,
    Schedules,
    Usage,
    Board,
    Company,
    Changes,
    History,
    Merge,
    /// Alt+S and Alt+P.
    Grid,
    Pin,
    /// The project of the agent at this index.
    Project(usize),
}

/// One line of Ctrl K (inboxpalette.ts PaletteItem).
pub(super) struct PalItem {
    group: String,
    label: String,
    sub: Option<String>,
    keys: Option<&'static str>,
    agent: Option<String>,
    run: PalRun,
}

/// Ctrl K: the box, and which line is lit.
pub(super) struct Palette {
    query: Entity<InputState>,
    at: usize,
    _sub: Subscription,
}

/// filterItems: every word appears in the label, the second line or the group.
pub(super) fn filter_items(items: Vec<PalItem>, query: &str) -> Vec<PalItem> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|it| {
            let hay = format!("{} {} {}", it.label, it.sub.clone().unwrap_or_default(), it.group).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

impl Inbox {
    /// paletteItems: every agent in queue order, then what the native app can
    /// already do (actions arrive here as their features land).
    pub(super) fn palette_items(&self, view: &View) -> Vec<PalItem> {
        let mut items: Vec<PalItem> = self
            .fleet
            .seats
            .iter()
            .enumerate()
            .map(|(i, seat)| {
                let line = row_line(&view.statuses[i], view.worked[i]);
                let title = seat.spec.title.clone().map(|t| format!("{t} \u{00b7} ")).unwrap_or_default();
                PalItem {
                    group: label_of(view.statuses[i].state).to_string(),
                    label: seat.spec.name.clone(),
                    sub: Some(format!("{title}{} \u{00b7} {line}", seat.project())),
                    keys: None,
                    agent: Some(seat.command()),
                    run: PalRun::Agent(i),
                }
            })
            .collect();
        items.push(PalItem { group: "Actions".into(), label: "New agent".into(), sub: Some("Give 2 or 3 the same job to race them".into()), keys: Some("Ctrl+Shift+T"), agent: None, run: PalRun::NewAgent });
        if view.statuses.iter().any(|s| s.state == State::Stopped) {
            items.push(PalItem { group: "Actions".into(), label: "Resume all".into(), sub: Some("Resume every stopped agent".into()), keys: None, agent: None, run: PalRun::ResumeAll });
        }
        let act = |label: &str, keys: &'static str, sub: Option<&str>, run: PalRun| PalItem { group: "Actions".into(), label: label.into(), sub: sub.map(str::to_string), keys: Some(keys), agent: None, run };
        items.push(if self.grid.on {
            act("Back to Focus: one agent, full size", "Alt+S", None, PalRun::Grid)
        } else {
            act("Grid: one agent and the others beside it", "Alt+S", Some("The one you work with as a terminal, every other one on a card that answers in place"), PalRun::Grid)
        });
        items.push(act("Add the current agent to the Grid, or take it out", "Alt+P", None, PalRun::Pin));
        if let Some(seat) = self.fleet.focused_seat() {
            items.push(act("Changes of the current agent", "Alt+R", Some("Merge them, or send it back with a note"), PalRun::Changes));
            if seat.spec.worktree.is_some() && view.statuses.get(self.fleet.focused.unwrap_or(0)).is_some_and(|s| s.state != State::Working) {
                items.push(act(&format!("Merge {}'s branch", seat.spec.name), "M", Some("From Changes: its worktree is removed after"), PalRun::Merge));
            }
            items.push(act("History of the current agent", "Alt+H", Some("What it did and what you answered"), PalRun::History));
        }
        items.push(act("Board", "Ctrl+Shift+K", Some("Cards the agents move as they work"), PalRun::Board));
        items.push(act("Company", "Ctrl+Shift+O", Some("Give the Founder a job, and see what needs you"), PalRun::Company));
        items.push(act("Sessions", "Alt+O", Some("Read back earlier Claude conversations"), PalRun::Sessions));
        if let Some(dir) = self.project_dir() {
            let name = crate::fleet::project_of(&dir);
            items.push(PalItem { group: "Actions".into(), label: "Project memory".into(), sub: Some(format!("What every agent in {name} knows: read, edit, or ask an agent to fix it")), keys: None, agent: None, run: PalRun::Memory });
        }
        items.push(act("Files", "Ctrl+Shift+E", Some("Folder tree; files open in VS Code"), PalRun::Files));
        items.push(act("Settings", "Ctrl+,", None, PalRun::Settings));
        items.push(PalItem { group: "Actions".into(), label: "Scheduled agents".into(), sub: Some("Start a preset at a set time".into()), keys: None, agent: None, run: PalRun::Schedules });
        items.push(PalItem { group: "Actions".into(), label: "Usage".into(), sub: Some("Tokens and estimated cost".into()), keys: None, agent: None, run: PalRun::Usage });
        // Every other project with agents, by its first agent.
        let here = self.prefs.project.clone().unwrap_or_default();
        let mut seen: Vec<&str> = Vec::new();
        for (i, seat) in self.fleet.seats.iter().enumerate() {
            let dir = seat.spec.cwd.as_str();
            if presets::same_folder(dir, &here) || seen.iter().any(|d| presets::same_folder(d, dir)) {
                continue;
            }
            seen.push(dir);
            let n = self.fleet.seats.iter().filter(|s| presets::same_folder(&s.spec.cwd, dir)).count();
            let sub = format!("{n} agent{} · {dir}", if n == 1 { "" } else { "s" });
            items.push(PalItem { group: "Projects".into(), label: seat.project(), sub: Some(sub), keys: None, agent: None, run: PalRun::Project(i) });
        }
        items
    }

    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Jump to an agent or run a command"));
        let sub = cx.subscribe(&query, |this, _, _: &InputEvent, cx| {
            if let Some(p) = &mut this.palette {
                p.at = 0;
            }
            cx.notify();
        });
        query.update(cx, |input, cx| input.focus(window, cx));
        crate::keys::set_active(false);
        self.menu = None;
        self.palette = Some(Palette { query, at: 0, _sub: sub });
        cx.notify();
    }

    pub(super) fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        if let Some(index) = self.fleet.focused {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    pub(super) fn run_palette(&mut self, run: PalRun, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        match run {
            PalRun::Agent(index) => self.open_agent(index, window, cx),
            PalRun::Grid => self.set_grid(!self.grid.on, window, cx),
            PalRun::Pin => self.pin_current(window, cx),
            PalRun::NewAgent => self.open_sheet(window, cx),
            PalRun::Sessions => self.open_sessions(window, cx),
            PalRun::Settings => self.open_settings(window, cx),
            PalRun::Files => self.toggle_files(window, cx),
            PalRun::Schedules => self.open_schedules(window, cx),
            PalRun::Usage => self.open_usage(window, cx),
            PalRun::Board => self.open_board(window, cx),
            PalRun::Company => self.open_company(window, cx),
            PalRun::Memory => {
                if let Some(dir) = self.project_dir() {
                    self.open_memory(dir, window, cx);
                }
            }
            PalRun::Changes | PalRun::Merge => {
                if let Some(index) = self.fleet.focused {
                    self.open_changes(index, window, cx);
                    if matches!(run, PalRun::Merge) {
                        self.merge_changes(window, cx);
                    }
                }
            }
            PalRun::History => {
                if let Some(index) = self.fleet.focused {
                    self.open_history(index, window, cx);
                }
            }
            PalRun::Project(index) => {
                if let Some(dir) = self.fleet.seats.get(index).map(|s| s.spec.cwd.clone()) {
                    self.open_project(dir, window, cx);
                }
            }
            PalRun::ResumeAll => {
                for index in 0..self.fleet.seats.len() {
                    if self.fleet.seats[index].state(cx) == State::Stopped {
                        self.start_seat(index, cx);
                    }
                }
                if let Some(index) = self.fleet.focused {
                    self.fleet.focus(index, window, cx);
                }
            }
        }
        cx.notify();
    }

    /// inboxpalette.ts and the .inbox-pal rules of inbox.css.
    pub(super) fn palette_view(&self, p: &Palette, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = p.query.read(cx).value().to_string();
        let shown = filter_items(self.palette_items(view), &query);
        let at = p.at.min(shown.len().saturating_sub(1));
        let count = shown.len();
        let runs: Vec<PalRun> = shown.iter().map(|it| it.run).collect();
        let kbd = |text: &'static str| mono().px(px(6.0)).py(px(2.0)).rounded(px(6.0)).text_size(px(11.0)).text_color(color(INK_3)).shadow(vec![ring(white(0.12))]).child(text);
        let mut list = div().id("pal-list").flex().flex_col().flex_1().min_h_0().p(px(6.0));
        let mut group = String::new();
        for (i, it) in shown.into_iter().enumerate() {
            if it.group != group {
                group = it.group.clone();
                list = list.child(div().pt(px(10.0)).px(px(10.0)).pb(px(4.0)).text_size(px(11.5)).text_color(color(INK_3)).child(group.clone()));
            }
            let run = it.run;
            list = list.child(
                div()
                    .id(("pal", i))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .py(px(8.0))
                    .px(px(10.0))
                    .rounded(px(10.0))
                    .when(i == at, |row| row.bg(white(0.07)))
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(p) = &mut this.palette {
                            if p.at != i {
                                p.at = i;
                                cx.notify();
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| this.run_palette(run, window, cx)))
                    .child(match &it.agent {
                        Some(command) => self.mark(command, 22.0, 6.0).into_any_element(),
                        None => div().size(px(22.0)).flex_none().into_any_element(),
                    })
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_baseline()
                            .truncate()
                            .text_size(px(13.5))
                            .text_color(color(INK))
                            .child(div().flex_none().child(it.label))
                            .children(it.sub.map(|s| div().ml(px(8.0)).min_w_0().truncate().text_size(px(12.5)).text_color(color(INK_3)).child(s))),
                    )
                    .children(it.keys.map(kbd)),
            );
        }
        if count == 0 {
            list = list.child(div().my(px(18.0)).mx(px(12.0)).text_size(px(13.0)).text_color(color(INK_3)).child(format!("Nothing matches \u{201c}{query}\u{201d}.")));
        }
        div()
            .id("pal-back")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .justify_center()
            .items_start()
            .pt(relative(0.12))
            .px(px(16.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_palette(window, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let n = runs.len();
                match event.keystroke.key.as_str() {
                    "escape" => this.close_palette(window, cx),
                    "down" => {
                        if let Some(p) = &mut this.palette {
                            p.at = (p.at + 1).min(n.saturating_sub(1));
                        }
                        cx.notify();
                    }
                    "up" => {
                        if let Some(p) = &mut this.palette {
                            p.at = p.at.saturating_sub(1);
                        }
                        cx.notify();
                    }
                    "enter" => {
                        let at = this.palette.as_ref().map(|p| p.at.min(n.saturating_sub(1))).unwrap_or(0);
                        if let Some(run) = runs.get(at).copied() {
                            this.run_palette(run, window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .id("pal")
                    .w(px(620.0))
                    .max_h(relative(0.62))
                    .flex()
                    .flex_col()
                    .rounded(px(16.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(10.0))
                            .h(px(52.0))
                            .px(px(16.0))
                            .border_b_1()
                            .border_color(hairline())
                            .child(icon("search", 16.0, color(INK_3)))
                            .child(div().flex_1().text_size(px(15.0)).child(Input::new(&p.query).appearance(false))),
                    )
                    .child(list.overflow_y_scrollbar())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(16.0))
                            .py(px(10.0))
                            .px(px(16.0))
                            .border_t_1()
                            .border_color(hairline())
                            .text_size(px(12.0))
                            .text_color(color(INK_3))
                            .child(div().flex().items_center().gap(px(3.0)).child(kbd("\u{2191}")).child(kbd("\u{2193}")).child(" to move"))
                            .child(div().flex().items_center().gap(px(3.0)).child(kbd("\u{21b5}")).child(" to open"))
                            .child(div().flex().items_center().gap(px(3.0)).child(kbd("Esc")).child(" to close"))
                            .child(div().ml_auto().child(format!("{count} result{}", if count == 1 { "" } else { "s" }))),
                    ),
            )
    }
}
