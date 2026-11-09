//! Work done off the render: plan limits, what each agent used, jobs typed into new agents.

use super::*;

impl Inbox {
    /// Type each waiting job once its agent is ready, as deliverTask does:
    /// after the start delay, not while it asks something, once it is quiet.
    pub(super) fn deliver(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        // Settings → Wait before giving the job.
        let delay = self.prefs.job_delay * 1000;
        let mut done: Vec<String> = Vec::new();
        for job in &mut self.jobs {
            let Some(seat) = self.fleet.seats.iter().find(|s| s.spec.id == job.id) else {
                done.push(job.id.clone());
                continue;
            };
            let Some(terminal) = seat.terminal.clone() else {
                // Its worktree is being made: the wait starts when it does.
                if seat.preparing {
                    job.started = now;
                } else {
                    done.push(job.id.clone());
                }
                continue;
            };
            let t = terminal.read(cx);
            if t.exit_code().is_some() {
                done.push(job.id.clone());
                continue;
            }
            let screen = t.screen_text();
            if job.typer.is_none() {
                let waited = now.saturating_sub(job.started);
                let ready = waited >= delay && t.quiet_for_ms() >= 1500 && parse_ask(&screen).is_none();
                if !ready && waited < PROMPT_WAIT_MS {
                    continue;
                }
                job.typer = Some(Typer::new(&job.text, t.bracketed_paste(), now));
            }
            if let Some(typer) = &mut job.typer {
                match typer.step(now, &screen) {
                    TypeStep::Write(bytes) => t.write_raw(bytes),
                    TypeStep::Wait => {}
                    TypeStep::Done(_) => done.push(job.id.clone()),
                }
            }
        }
        self.jobs.retain(|j| !done.contains(&j.id));
    }

    pub(super) fn queue_job(&mut self, id: String, text: String, cx: &mut Context<Self>) {
        self.queue_typing(Job { id, text, started: now_ms(), typer: None }, cx);
    }

    /// A message for a running agent: no start delay, only the wait until
    /// it is quiet and not asking anything.
    pub(super) fn queue_message(&mut self, id: String, text: String, cx: &mut Context<Self>) {
        let started = now_ms().saturating_sub(self.prefs.job_delay * 1000);
        self.queue_typing(Job { id, text, started, typer: None }, cx);
    }

    pub(super) fn queue_typing(&mut self, job: Job, cx: &mut Context<Self>) {
        self.jobs.push(job);
        if self.typing {
            return;
        }
        self.typing = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let more = this.update(cx, |this, cx| {
                    this.deliver(cx);
                    this.typing = !this.jobs.is_empty();
                    this.typing
                });
                if !matches!(more, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    /// Ask both plans now, off the UI thread.
    pub(super) fn ask_plans(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async move { (plan::ask_claude(), plan::ask_codex()) });
        cx.spawn(async move |this, cx| {
            let (claude, codex) = ask.await;
            let _ = this.update(cx, |this, cx| {
                let now = now_ms();
                match claude {
                    Ok(u) => {
                        this.plans.0.usage = Some(u);
                        this.plans.0.problem = None;
                    }
                    Err(why) => {
                        if why == "not signed in" {
                            this.plans.0.usage = None;
                        }
                        this.plans.0.problem = Some(why);
                    }
                }
                this.plans.0.asked_at = now;
                if codex.is_some() {
                    this.plans.1.usage = codex;
                    this.plans.1.asked_at = now;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Read again what an agent has used, when it printed something new and
    /// the last read is older than REFRESH_MS (agentcost.ts).
    pub(super) fn refresh_usage(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        for seat in &self.fleet.seats {
            let cli = seat.spec.cli.to_ascii_lowercase();
            if !matches!(cli.as_str(), "claude" | "codex" | "opencode") && seat.terminal.is_none() {
                continue;
            }
            let last_output = seat.terminal.as_ref().map(|t| t.read(cx).last_output_ms()).unwrap_or(0);
            let entry = self.uses.entry(seat.spec.id.clone()).or_default();
            let moved = last_output > entry.at;
            if entry.busy || !(entry.at == 0 || (moved && now - entry.at > usage::REFRESH_MS)) {
                continue;
            }
            entry.busy = true;
            entry.at = now;
            let (id, dir, session, since) = (seat.spec.id.clone(), seat.spec.cwd.clone(), seat.spec.session_id.clone(), seat.spec.started_ms);
            let screen = seat.terminal.as_ref().map(|t| t.read(cx).screen_text()).unwrap_or_default();
            let read = cx.background_executor().spawn(async move { usage::read(&cli, &dir, session.as_deref(), since, &screen) });
            cx.spawn(async move |this, cx| {
                let u = read.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(entry) = this.uses.get_mut(&id) {
                        entry.busy = false;
                        if entry.usage.as_ref() != Some(&u) {
                            entry.usage = Some(u);
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
    }
}
