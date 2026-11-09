//! Starting agents, the one way both New agent and Home do it (spawnAgents in
//! today's app): N agents on the same job race, the job is typed in once each
//! CLI is ready, the folder becomes the current project, the first agent
//! takes the stage.

use super::*;

/// What to start.
pub(super) struct Launch {
    pub program: String,
    pub args: Vec<String>,
    pub dir: String,
    pub count: usize,
    pub job: String,
    /// How New agent and Home remember the CLI: a preset id or "custom:<command>".
    pub remembered: String,
    /// Asked for by an agent (agent_spawn): the person's choices and the
    /// stage stay as they are.
    pub quiet: bool,
}

impl Inbox {
    pub(super) fn launch(&mut self, l: Launch, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        if !l.quiet {
            if let Some(custom) = l.remembered.strip_prefix("custom:") {
                self.prefs.last_custom = custom.to_string();
            }
            self.prefs.last_cli = Some(l.remembered.clone());
            self.prefs.open_project(&l.dir);
        }
        let title = presets::task_title(&l.job);
        let race_id = (l.count > 1).then(|| format!("race-{}", now_ms()));
        let mut first = None;
        let mut waiting = Vec::new();
        for n in 1..=l.count.max(1) {
            let race = race_id.clone().map(|id| Race { id, n, of: l.count });
            let index = if self.prefs.worktree {
                // Settings → Own branch for every agent: it starts once its worktree is made.
                let index = self.fleet.reserve(&l.program, l.args.clone(), &l.dir, title.clone(), race);
                waiting.push(self.fleet.seats[index].spec.id.clone());
                index
            } else {
                self.fleet.add(&l.program, l.args.clone(), &l.dir, title.clone(), race, cx).map_err(|e| format!("Could not start {}: {e}", l.program))?
            };
            first.get_or_insert(index);
            if !l.job.is_empty() {
                let id = self.fleet.seats[index].spec.id.clone();
                self.queue_job(id, l.job.clone(), cx);
            }
        }
        if !waiting.is_empty() {
            self.isolate(l.dir.clone(), waiting, window, cx);
        }
        if l.quiet {
            cx.notify();
            return Ok(());
        }
        self.home = false;
        if let Some(index) = first {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
        Ok(())
    }

    /// Make each waiting agent its worktree off the UI thread, then start it
    /// there. Outside a git repo, or when git says no, it starts in the
    /// project folder, and a failure is said.
    fn isolate(&mut self, dir: String, ids: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let names: Vec<(String, String)> = ids.iter().filter_map(|id| self.fleet.seats.iter().find(|s| &s.spec.id == id).map(|s| (id.clone(), s.spec.name.clone()))).collect();
        let made = cx.background_spawn(async move {
            let root = agent_core::git::repo_root(std::path::Path::new(&dir));
            names
                .into_iter()
                .map(|(id, name)| {
                    let Some(root) = &root else { return (id, None) };
                    let branch = crate::worktree::branch_name(&name, &id);
                    let made = crate::worktree::add(root, &branch).map(|path| (path, branch)).map_err(|why| format!("{name} has no worktree and works in the project folder: {why}"));
                    (id, Some(made))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn_in(window, async move |this, cx| {
            let made = made.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let mut started = Vec::new();
                for (id, made) in made {
                    let Some(index) = this.fleet.seats.iter().position(|s| s.spec.id == id) else { continue };
                    match made {
                        Some(Ok((path, branch))) => {
                            let spec = &mut this.fleet.seats[index].spec;
                            spec.worktree = Some(path);
                            spec.branch = Some(branch);
                        }
                        Some(Err(why)) => this.toast(why, cx),
                        None => {}
                    }
                    match this.fleet.start(index, cx) {
                        Ok(()) => started.push(index),
                        Err(e) => {
                            let name = this.fleet.seats[index].spec.name.clone();
                            this.toast(format!("Could not start {name}: {e}"), cx);
                        }
                    }
                }
                // The one on the stage takes the keyboard now that it has a
                // terminal, unless something is open over it.
                let overlay = this.home || this.sheet.is_some() || this.settings.is_some() || this.sessions.is_some() || this.palette.is_some();
                if let Some(index) = this.fleet.focused.filter(|i| started.contains(i) && !overlay) {
                    this.fleet.focus(index, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Start a stopped agent again; why it could not, as a toast.
    pub(super) fn start_seat(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        match self.fleet.start(index, cx) {
            Ok(()) => true,
            Err(e) => {
                self.toast(e.to_string(), cx);
                false
            }
        }
    }

    /// The CLI's args for a preset, with its skip flag when asked.
    pub(super) fn preset_command(id: &str, skip_perms: bool) -> Option<(String, Vec<String>)> {
        let p = presets::preset(id)?;
        let args = if skip_perms { p.skip_args.iter().map(|a| a.to_string()).collect() } else { Vec::new() };
        Some((p.program.to_string(), args))
    }

    /// A typed command as program and args.
    pub(super) fn custom_command(typed: &str) -> Option<(String, Vec<String>)> {
        let mut words = typed.split_whitespace().map(str::to_string);
        let program = words.next()?;
        Some((program, words.collect()))
    }

    /// The project New agent starts in: the current one, the stage's, the last.
    pub(super) fn project_dir(&self) -> Option<String> {
        self.prefs
            .project
            .clone()
            .or_else(|| self.fleet.focused_seat().or(self.fleet.seats.last()).map(|s| s.spec.cwd.clone()))
            .or_else(|| self.prefs.recents.first().cloned())
    }
}
