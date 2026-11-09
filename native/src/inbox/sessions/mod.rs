//! Sessions (sessionsview.ts, sessions.css): every earlier Claude conversation
//! of a project, its agents' worktrees included, read back. Only to read:
//! nothing here starts, resumes or types into an agent.

pub(super) mod list;
mod reader;

use agent_core::claude::SessionInfo;

use super::dropdown::folder_name;
use super::*;

pub(super) use reader::Reading;

/// Which sessions the list shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Project,
    All,
}

pub(super) struct Sessions {
    /// The project it opened on, and its name.
    root: Option<(String, String)>,
    scope: Scope,
    list: Vec<SessionInfo>,
    listing: bool,
    query: Entity<InputState>,
    reading: Option<Reading>,
    /// Bumped on every new list or read, so one that finishes late is dropped.
    list_no: u64,
    read_no: u64,
    _query: Subscription,
}

/// The folder Claude Code keeps every project's transcripts in.
fn projects_root() -> Option<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    Some(std::path::Path::new(&home).join(".claude").join("projects"))
}

/// claude_sessions_in: the project and its agents' worktrees, kept outside
/// the repo under a folder named after a hash of the path as git wrote it
/// (D:/x or D:\x), so both are looked under.
fn sessions_of(root: &str) -> Vec<SessionInfo> {
    let Some(projects) = projects_root() else { return Vec::new() };
    let mut roots = vec![root.to_string()];
    for form in [root.replace('/', "\\"), root.replace('\\', "/")] {
        let w = agent_core::git::worktrees_of(&form).to_string_lossy().into_owned();
        if !roots.contains(&w) {
            roots.push(w);
        }
    }
    let roots: Vec<&str> = roots.iter().map(String::as_str).collect();
    agent_core::claude::sessions_under(&projects, &roots, 300)
}

impl Inbox {
    /// Open the sessions of the current project, or of every folder without one.
    pub(super) fn open_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = &self.sessions {
            s.query.update(cx, |q, cx| q.focus(window, cx));
            return;
        }
        self.palette = None;
        self.menu = None;
        self.popup = None;
        crate::keys::set_active(false);
        let root = self.project_dir().map(|d| {
            let name = folder_name(&d);
            (d, name)
        });
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search by title or folder"));
        query.update(cx, |q, cx| q.focus(window, cx));
        let sub = cx.subscribe_in(&query, window, |this, _, event: &InputEvent, _, cx| match event {
            InputEvent::Change => cx.notify(),
            // Enter reads the first session that matches.
            InputEvent::PressEnter { .. } => {
                let first = this.sessions.as_ref().and_then(|s| s.shown(cx).first().map(|x| (*x).clone()));
                if let Some(first) = first {
                    this.read_session(first, false, cx);
                }
            }
            _ => {}
        });
        let scope = if root.is_some() { Scope::Project } else { Scope::All };
        self.sessions = Some(Sessions { root, scope, list: Vec::new(), listing: true, query, reading: None, list_no: 0, read_no: 0, _query: sub });
        self.load_sessions(cx);
        cx.notify();
    }

    pub(super) fn close_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sessions = None;
        if self.home {
            if let Some(c) = &self.compose {
                c.job.update(cx, |input, cx| input.focus(window, cx));
            }
        } else if let Some(index) = self.fleet.focused {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    fn load_sessions(&mut self, cx: &mut Context<Self>) {
        let Some(s) = &mut self.sessions else { return };
        s.list_no += 1;
        s.listing = true;
        let no = s.list_no;
        let root = (s.scope == Scope::Project).then(|| s.root.as_ref().map(|(d, _)| d.clone())).flatten();
        let found = cx.background_spawn(async move {
            match root {
                Some(root) => sessions_of(&root),
                None => agent_core::claude::sessions_everywhere(200),
            }
        });
        cx.spawn(async move |this, cx| {
            let list = found.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(s) = this.sessions.as_mut().filter(|s| s.list_no == no) {
                    s.list = list;
                    s.listing = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        let Some(s) = &mut self.sessions else { return };
        if s.scope == scope || (scope == Scope::Project && s.root.is_none()) {
            return;
        }
        s.scope = scope;
        s.list.clear();
        self.load_sessions(cx);
        cx.notify();
    }

    pub(super) fn sessions_view(&self, s: &Sessions, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let side = self.sessions_side(s, cx);
        let read = self.sessions_reader(s, window, cx);
        div()
            .id("sv-back")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .justify_center()
            .py(px(28.0))
            .px(px(24.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_sessions(window, cx)))
            .child(
                div()
                    .id("sv")
                    .w(px(1280.0))
                    .max_w_full()
                    .h_full()
                    .flex()
                    .overflow_hidden()
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(side)
                    .child(read),
            )
    }
}

impl Sessions {
    /// matching(): the sessions whose title or folder has every word typed.
    fn shown(&self, cx: &App) -> Vec<&SessionInfo> {
        let q = self.query.read(cx).value().to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        self.list
            .iter()
            .filter(|s| {
                let hay = format!("{} {}", s.title, s.cwd).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect()
    }
}
