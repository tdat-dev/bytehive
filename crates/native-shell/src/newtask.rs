//! The New task sheet's model, ported from the approved Agent Inbox design
//! (`design/redesign/maestro-inbox.html`) and the web app's `spawnAgents`:
//! one prompt, one folder, one CLI, 1..4 agents. Several agents on one task
//! is a race, each on its own branch, and you keep the best diff. Pure, so
//! names, branches and the help line are checked without a window.

use crate::sessions_ui::Cli;

/// The most agents one task starts: a race of four is already a lot to review.
pub const MAX_AGENTS: usize = 4;

/// Short, neutral persona names given on spawn, same list as the web app.
pub const PERSONAS: [&str; 20] = [
    "Ana", "Bob", "Cid", "Dot", "Eve", "Fin", "Gio", "Hux", "Ivy", "Jax", "Kim", "Lux", "Mei", "Nix", "Oz", "Pia", "Rue", "Sol", "Tao", "Uma",
];

/// The flag that lets a CLI skip its permission prompts (full access), as the
/// web app's presets have them. Opencode only configures this in a file.
pub fn skip_perms_args(cli: Cli) -> Vec<String> {
    let flag = match cli {
        Cli::Claude => "--dangerously-skip-permissions",
        Cli::Codex | Cli::Muse => "--yolo",
        Cli::Opencode => return Vec::new(),
    };
    vec![flag.to_string()]
}

/// The next persona nobody here uses yet; past the list, "Agent N".
pub fn persona(taken: &[String]) -> String {
    PERSONAS
        .iter()
        .find(|p| !taken.iter().any(|t| t == *p))
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("Agent {}", taken.len() + 1))
}

/// The branch an agent works on: `ana/fix-the-flaky-upload`, the task slugged
/// to 22 characters like the design's.
pub fn branch_for(name: &str, task: &str) -> String {
    let mut slug = String::new();
    for c in task.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let full = slug.trim_matches('-');
    let mut slug: String = full.chars().take(22).collect();
    // Cut at a word edge, not mid-word ("upload-t").
    if full.chars().nth(22).is_some_and(|c| c != '-') {
        if let Some(edge) = slug.rfind('-') {
            slug.truncate(edge);
        }
    }
    let slug = slug.trim_end_matches('-');
    let who: String = name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!("{}/{}", if who.is_empty() { "agent" } else { &who }, if slug.is_empty() { "task" } else { slug })
}

/// The sheet's one-line explanation of what Start will do.
pub fn help_line(count: usize, worktree: bool) -> String {
    if count > 1 && !worktree {
        return format!("{count} agents in one checkout will overwrite each other. Turn on Own worktree.");
    }
    let who = if count == 1 { "One agent".to_string() } else { format!("{count} agents race") };
    let place = match (worktree, count == 1) {
        (true, true) => "on its own branch. Nothing touches your checkout until you merge",
        (true, false) => "on separate branches, then you compare the diffs and keep one",
        (false, _) => "in your checkout (no isolation)",
    };
    format!("{who} {place}.")
}

/// What the sheet holds.
#[derive(Clone)]
pub struct NewTask {
    pub prompt: String,
    pub dir: String,
    pub cli: Cli,
    pub count: usize,
    pub worktree: bool,
    pub skip_perms: bool,
}

/// One agent to start for a task.
#[derive(Clone, Debug, PartialEq)]
pub struct Launch {
    pub name: String,
    pub cli: Cli,
    /// The branch to work on in its own worktree; none works in the folder.
    pub branch: Option<String>,
    pub extra_args: Vec<String>,
    pub task: String,
}

impl NewTask {
    pub fn new(dir: String) -> NewTask {
        NewTask { prompt: String::new(), dir, cli: Cli::Claude, count: 1, worktree: true, skip_perms: false }
    }

    pub fn set_count(&mut self, n: usize) {
        self.count = n.clamp(1, MAX_AGENTS);
    }

    /// Why Start can't go yet, or nothing when it can.
    pub fn problem(&self) -> Option<String> {
        if self.prompt.trim().is_empty() {
            return Some("Write what should get done first.".to_string());
        }
        if self.dir.trim().is_empty() {
            return Some("Pick a project folder.".to_string());
        }
        if self.count > 1 && !self.worktree {
            return Some(help_line(self.count, false));
        }
        None
    }

    /// The agents to start, named after nobody in `taken`.
    pub fn launches(&self, taken: &[String]) -> Vec<Launch> {
        let mut taken = taken.to_vec();
        let task = self.prompt.trim().to_string();
        let extra = if self.skip_perms { skip_perms_args(self.cli) } else { Vec::new() };
        (0..self.count)
            .map(|_| {
                let name = persona(&taken);
                taken.push(name.clone());
                let branch = self.worktree.then(|| branch_for(&name, &task));
                Launch { name, cli: self.cli, branch, extra_args: extra.clone(), task: task.clone() }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn personas_skip_names_in_use() {
        assert_eq!(persona(&[]), "Ana");
        assert_eq!(persona(&taken(&["Ana", "Cid"])), "Bob");
        let all: Vec<String> = PERSONAS.iter().map(|s| s.to_string()).collect();
        assert_eq!(persona(&all), "Agent 21");
    }

    #[test]
    fn branches_slug_the_task() {
        assert_eq!(branch_for("Ana", "Fix the flaky upload test"), "ana/fix-the-flaky-upload");
        assert_eq!(branch_for("Bob", "  !!  "), "bob/task");
        assert_eq!(branch_for("Oz", "Sửa lỗi login"), "oz/s-a-l-i-login");
    }

    #[test]
    fn help_line_warns_a_race_without_worktrees() {
        assert_eq!(help_line(1, true), "One agent on its own branch. Nothing touches your checkout until you merge.");
        assert!(help_line(3, true).starts_with("3 agents race on separate branches"));
        assert_eq!(help_line(1, false), "One agent in your checkout (no isolation).");
        assert!(help_line(2, false).contains("Turn on Own worktree"));
    }

    #[test]
    fn a_task_needs_words_and_a_race_needs_worktrees() {
        let mut t = NewTask::new("D:/app".to_string());
        assert!(t.problem().is_some(), "empty prompt");
        t.prompt = "Fix the login form".to_string();
        assert!(t.problem().is_none());
        t.set_count(3);
        t.worktree = false;
        assert!(t.problem().unwrap().contains("Own worktree"));
        t.set_count(99);
        assert_eq!(t.count, MAX_AGENTS);
    }

    #[test]
    fn a_race_gets_distinct_names_and_branches() {
        let mut t = NewTask::new("D:/app".to_string());
        t.prompt = "Fix the flaky upload test".to_string();
        t.set_count(3);
        t.skip_perms = true;
        let l = t.launches(&taken(&["Ana"]));
        let names: Vec<&str> = l.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Bob", "Cid", "Dot"]);
        assert_eq!(l[0].branch.as_deref(), Some("bob/fix-the-flaky-upload"));
        assert_eq!(l[0].extra_args, ["--dangerously-skip-permissions"]);
        t.worktree = false;
        t.set_count(1);
        t.cli = Cli::Opencode;
        let l = t.launches(&[]);
        assert_eq!(l[0].branch, None);
        assert!(l[0].extra_args.is_empty(), "opencode has no flag");
    }
}
