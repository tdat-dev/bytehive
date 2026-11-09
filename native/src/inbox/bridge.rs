//! The bridge to bytehive-mcp, run every 1.5 s (initFleetBridge in today's
//! bridges.ts): each project's roster out, fleet_send messages and
//! agent_spawn requests in. The files are read and written off the UI thread.

use std::sync::{Arc, Mutex};

use crate::bridge::{BridgeAgent, BridgeFiles, OutboxLine, SpawnLine, bytehive_dir, delivery_targets, parse_outbox_line, parse_spawn_line, serialize_fleet};

use super::launch::Launch;
use super::*;

const EVERY: Duration = Duration::from_millis(1500);
/// Screen text an agent's roster entry carries, for agent_output.
const SCREEN_CHARS: usize = 2000;
/// Words of a message the hand-off toast shows.
const TOAST_MESSAGE_MAX: usize = 90;

/// What one tick read: per project, the new messages and spawn requests.
type Read = Vec<(String, Vec<OutboxLine>, Vec<SpawnLine>)>;

fn status_word(state: State) -> &'static str {
    match state {
        State::Needs | State::Blocked => "needs",
        State::Working => "active",
        State::Stopped | State::Merged => "stopped",
        _ => "idle",
    }
}

/// "Claude Code" runs as claude: the CLI as the roster names it.
fn cli_word(cli: &str) -> String {
    let program = cli.rsplit(['\\', '/']).next().unwrap_or(cli).to_ascii_lowercase();
    program.strip_suffix(".exe").or_else(|| program.strip_suffix(".cmd")).unwrap_or(&program).to_string()
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    format!("{}…", t.chars().take(max - 1).collect::<String>().trim_end())
}

impl Inbox {
    /// Start the bridge once, from the first render: spawns need the window.
    pub(super) fn start_bridge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.bridge.is_some() {
            return;
        }
        let files: Arc<Mutex<BridgeFiles>> = Arc::default();
        self.bridge = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(EVERY).await;
                let Ok(rosters) = this.update(cx, |this, cx| this.rosters(cx)) else { break };
                let files = files.clone();
                let read: Read = cx
                    .background_spawn(async move {
                        let mut files = files.lock().unwrap();
                        let mut read = Vec::new();
                        for (dir, content) in rosters {
                            files.publish(&dir, content);
                            let out = files.drain(&bytehive_dir(&dir).join("outbox.jsonl")).iter().filter_map(|l| parse_outbox_line(l)).collect();
                            let spawn = files.drain(&bytehive_dir(&dir).join("spawn-requests.jsonl")).iter().filter_map(|l| parse_spawn_line(l)).collect();
                            read.push((dir, out, spawn));
                        }
                        read
                    })
                    .await;
                if this.update_in(cx, |this, window, cx| this.bridge_apply(read, window, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    /// Each project with agents, and its roster as fleet.json holds it.
    fn rosters(&self, cx: &App) -> Vec<(String, String)> {
        let mut dirs: Vec<String> = Vec::new();
        for seat in &self.fleet.seats {
            if !seat.spec.cwd.is_empty() && !dirs.iter().any(|d| presets::same_folder(d, &seat.spec.cwd)) {
                dirs.push(seat.spec.cwd.clone());
            }
        }
        dirs.into_iter()
            .map(|dir| {
                let agents: Vec<BridgeAgent> = self
                    .fleet
                    .seats
                    .iter()
                    .filter(|s| presets::same_folder(&s.spec.cwd, &dir))
                    .map(|s| {
                        let running = s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
                        let screen = s.terminal.as_ref().map(|t| t.read(cx).screen_text().trim_end().to_string()).unwrap_or_default();
                        let cut = screen.char_indices().rev().nth(SCREEN_CHARS - 1).map_or(0, |(i, _)| i);
                        BridgeAgent {
                            id: s.spec.id.clone(),
                            name: s.spec.name.clone(),
                            status: status_word(s.state(cx)),
                            screen: screen[cut..].to_string(),
                            cli: cli_word(&s.spec.cli),
                            session: s.spec.session_id.clone(),
                            folder: s.spec.run_dir().to_string(),
                            since: running.then_some(s.spec.started_ms),
                        }
                    })
                    .collect();
                let content = serialize_fleet(&crate::fleet::project_of(&dir), &agents);
                (dir, content)
            })
            .collect()
    }

    fn bridge_apply(&mut self, read: Read, window: &mut Window, cx: &mut Context<Self>) {
        for (dir, messages, spawns) in read {
            for m in messages {
                self.deliver_message(&dir, m, cx);
            }
            for s in spawns {
                self.spawn_for_agent(&dir, s, window, cx);
            }
        }
    }

    /// Type a fleet_send message into its agents, once each is ready for it.
    fn deliver_message(&mut self, dir: &str, m: OutboxLine, cx: &mut Context<Self>) {
        let here: Vec<usize> = (0..self.fleet.seats.len()).filter(|&i| presets::same_folder(&self.fleet.seats[i].spec.cwd, dir)).collect();
        let roll: Vec<(String, bool)> = here
            .iter()
            .map(|&i| {
                let s = &self.fleet.seats[i];
                (s.spec.name.clone(), s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none()))
            })
            .collect();
        let targets = delivery_targets(&roll, m.from.as_deref(), m.to.as_deref());
        let mut names = Vec::new();
        for k in targets {
            let seat = &self.fleet.seats[here[k]];
            names.push(seat.spec.name.clone());
            self.queue_message(seat.spec.id.clone(), m.message.clone(), cx);
        }
        // delegation.ts: who handed what to whom, for a moment.
        if let Some(from) = m.from.as_deref().filter(|f| !f.trim().is_empty() && !names.is_empty()) {
            let to = if names.len() <= 2 { names.join(" & ") } else { format!("{} +{}", names[0], names.len() - 1) };
            self.toast_for(format!("{} → {to}: {}", from.trim(), truncate(&m.message, TOAST_MESSAGE_MAX)), 2600, cx);
        }
    }

    /// agent_spawn: new agents in the asking agent's project, given its task.
    /// The one on screen keeps the stage.
    fn spawn_for_agent(&mut self, dir: &str, s: SpawnLine, window: &mut Window, cx: &mut Context<Self>) {
        let Some((program, args)) = Self::preset_command(&s.cli, false).or_else(|| Self::custom_command(&s.cli)) else { return };
        let launch = Launch { program, args, dir: dir.to_string(), count: s.count, job: s.task.unwrap_or_default(), remembered: String::new(), quiet: true };
        if let Err(e) = self.launch(launch, window, cx) {
            self.toast(e, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cli_word, truncate};

    #[test]
    fn names_the_cli_and_cuts_long_messages() {
        assert_eq!(cli_word("claude"), "claude");
        assert_eq!(cli_word(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"), "powershell");
        assert_eq!(truncate("short", 90), "short");
        assert_eq!(truncate(&"x".repeat(100), 10), format!("{}…", "x".repeat(9)));
    }
}
