//! How a new agent starts: its permission mode (the approved design's
//! "Default permission mode") and the task's flow ("Just do it", "Build,
//! Test, Review", "Plan first, wait for me"). Pure, so the flags and the
//! shaped task text are checked without a window.

use crate::sessions_ui::Cli;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Perm {
    /// The CLI asks before running commands (its own default).
    #[default]
    Ask,
    /// Edits go through, commands still ask. Claude and Codex have a flag.
    AcceptEdits,
    /// No prompts at all: full access.
    Bypass,
}

impl Perm {
    pub const ALL: [Perm; 3] = [Perm::Ask, Perm::AcceptEdits, Perm::Bypass];

    pub fn label(self) -> &'static str {
        match self {
            Perm::Ask => "Ask before commands",
            Perm::AcceptEdits => "Accept edits",
            Perm::Bypass => "Bypass permissions",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Perm::Ask => "ask",
            Perm::AcceptEdits => "edits",
            Perm::Bypass => "bypass",
        }
    }

    pub fn from_key(k: &str) -> Perm {
        Perm::ALL.into_iter().find(|p| p.key() == k).unwrap_or_default()
    }

    /// Launch flags for `cli` in this mode. A CLI without such a flag starts
    /// in its own default (Opencode only configures this in a file).
    pub fn args(self, cli: Cli) -> Vec<String> {
        let flags: &[&str] = match (self, cli) {
            (Perm::Ask, _) => &[],
            (Perm::AcceptEdits, Cli::Claude) => &["--permission-mode", "acceptEdits"],
            (Perm::AcceptEdits, Cli::Codex) => &["--full-auto"],
            (Perm::Bypass, Cli::Claude) => &["--dangerously-skip-permissions"],
            (Perm::Bypass, Cli::Codex | Cli::Muse) => &["--yolo"],
            _ => &[],
        };
        flags.iter().map(|s| s.to_string()).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Flow {
    #[default]
    Direct,
    Relay,
    Plan,
}

impl Flow {
    pub const ALL: [Flow; 3] = [Flow::Direct, Flow::Relay, Flow::Plan];

    pub fn label(self) -> &'static str {
        match self {
            Flow::Direct => "Just do it",
            Flow::Relay => "Build \u{2192} Test \u{2192} Review",
            Flow::Plan => "Plan first, wait for me",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Flow::Direct => "direct",
            Flow::Relay => "relay",
            Flow::Plan => "plan",
        }
    }

    pub fn from_key(k: &str) -> Flow {
        Flow::ALL.into_iter().find(|f| f.key() == k).unwrap_or_default()
    }

    /// What the help line adds about this flow.
    pub fn help(self) -> &'static str {
        match self {
            Flow::Direct => "",
            Flow::Relay => " It moves through build, test and review stages.",
            Flow::Plan => " It writes a plan and waits for your go-ahead.",
        }
    }

    /// The task as the agent receives it: the words you wrote, plus how to
    /// work when the flow asks for more than doing it.
    pub fn shape(self, task: &str) -> String {
        match self {
            Flow::Direct => task.to_string(),
            Flow::Relay => format!(
                "{task}\n\nWork in three stages and say which one you are in: build it, then test it (run the tests, add one for the change), then review your own diff and fix what you find. Report when all three are done."
            ),
            Flow::Plan => format!("{task}\n\nBefore you change any file, write a short plan and stop. Wait for my go-ahead, then do it."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_flags_per_cli() {
        assert!(Perm::Ask.args(Cli::Claude).is_empty());
        assert_eq!(Perm::AcceptEdits.args(Cli::Claude), ["--permission-mode", "acceptEdits"]);
        assert_eq!(Perm::AcceptEdits.args(Cli::Codex), ["--full-auto"]);
        assert!(Perm::AcceptEdits.args(Cli::Muse).is_empty());
        assert_eq!(Perm::Bypass.args(Cli::Claude), ["--dangerously-skip-permissions"]);
        assert_eq!(Perm::Bypass.args(Cli::Muse), ["--yolo"]);
        assert!(Perm::Bypass.args(Cli::Opencode).is_empty());
    }

    #[test]
    fn keys_round_trip_and_unknown_is_default() {
        for p in Perm::ALL {
            assert_eq!(Perm::from_key(p.key()), p);
        }
        for f in Flow::ALL {
            assert_eq!(Flow::from_key(f.key()), f);
        }
        assert_eq!(Perm::from_key("?"), Perm::Ask);
        assert_eq!(Flow::from_key(""), Flow::Direct);
    }

    #[test]
    fn flows_shape_the_task() {
        assert_eq!(Flow::Direct.shape("Fix it"), "Fix it");
        assert!(Flow::Plan.shape("Fix it").starts_with("Fix it\n\nBefore you change any file"));
        assert!(Flow::Relay.shape("Fix it").contains("build it, then test it"));
    }
}
