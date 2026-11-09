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
}

impl Inbox {
    pub(super) fn launch(&mut self, l: Launch, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(custom) = l.remembered.strip_prefix("custom:") {
            self.prefs.last_custom = custom.to_string();
        }
        self.prefs.last_cli = Some(l.remembered.clone());
        self.prefs.open_project(&l.dir);
        let title = presets::task_title(&l.job);
        let race_id = (l.count > 1).then(|| format!("race-{}", now_ms()));
        let mut first = None;
        for n in 1..=l.count.max(1) {
            let race = race_id.clone().map(|id| Race { id, n, of: l.count });
            let index = self.fleet.add(&l.program, l.args.clone(), &l.dir, title.clone(), race, cx).map_err(|e| format!("Could not start {}: {e}", l.program))?;
            first.get_or_insert(index);
            if !l.job.is_empty() {
                let id = self.fleet.seats[index].spec.id.clone();
                self.queue_job(id, l.job.clone(), cx);
            }
        }
        self.home = false;
        if let Some(index) = first {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
        Ok(())
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
