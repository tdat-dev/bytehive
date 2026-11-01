//! Side tabs and the palette's command catalog: what a command key names and
//! where it leads. Pure, so the catalog is checked without a window; run by
//! [`crate::app::ShellApp::run_command`].

use crate::palette::{PaletteItem, PaletteKind};
use crate::sessions_ui::Cli;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideTab {
    Queue,
    Review,
    History,
    Sessions,
    Flow,
}

impl SideTab {
    /// Starting tab override for screenshots and debugging: MAESTRO_TAB=review.
    pub(crate) fn from_env() -> SideTab {
        Self::from_key(&std::env::var("MAESTRO_TAB").unwrap_or_default().to_lowercase())
    }

    /// The tab a `tab:<name>` command (or `MAESTRO_TAB`) names. Unknown names
    /// fall back to the Queue.
    pub(crate) fn from_key(key: &str) -> SideTab {
        match key {
            "review" => SideTab::Review,
            "history" => SideTab::History,
            "sessions" => SideTab::Sessions,
            "flow" => SideTab::Flow,
            _ => SideTab::Queue,
        }
    }
}

/// The palette's command rows: start a task, spawn a shell or an agent, or
/// jump to a side tab. Keys are read back by `run_command`: `new`, `race`,
/// `shell`, `spawn:<program>`, `tab:<name>`.
pub(crate) fn command_catalog() -> Vec<PaletteItem> {
    let cmd = |key: &str, name: &str, word: &str| PaletteItem {
        key: key.into(),
        name: name.into(),
        word: word.into(),
        kind: PaletteKind::Command,
    };
    let mut v = vec![
        cmd("new", "New task", "Ctrl N"),
        cmd("race", "Race 3 agents on one task", "run"),
        cmd("shell", "New Shell", "run"),
        cmd("settings", "Settings", "Ctrl ,"),
        cmd("files", "Files on this branch", "view"),
        cmd("changes", "Changes (review the diff)", "view"),
        cmd("board", "Board (agents by state)", "view"),
        cmd("plan", "Plan board (approve proposed cards)", "view"),
        cmd("memory", "Project memory (what every agent knows)", "view"),
    ];
    for cli in Cli::all() {
        v.push(cmd(&format!("spawn:{}", cli.program()), &format!("Spawn {}", cli.name()), "run"));
    }
    for (key, label) in [("queue", "Queue"), ("review", "Review"), ("history", "History"), ("sessions", "Sessions"), ("flow", "Flow")] {
        v.push(cmd(&format!("tab:{key}"), &format!("Go to {label}"), "view"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_catalog_covers_tasks_shell_agents_and_tabs() {
        let cat = command_catalog();
        let keys: Vec<&str> = cat.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"new"), "has New task: {keys:?}");
        assert!(keys.contains(&"race"), "has the race shortcut: {keys:?}");
        assert!(keys.contains(&"shell"), "has New Shell: {keys:?}");
        // One spawn command per CLI, keyed by program name.
        for cli in Cli::all() {
            let k = format!("spawn:{}", cli.program());
            assert!(keys.contains(&k.as_str()), "missing {k}: {keys:?}");
        }
        // One jump command per side tab.
        for t in ["tab:queue", "tab:review", "tab:history", "tab:sessions", "tab:flow"] {
            assert!(keys.contains(&t), "missing {t}: {keys:?}");
        }
        assert!(cat.iter().all(|i| i.kind == PaletteKind::Command));
    }

    #[test]
    fn tab_key_maps_to_side_tab() {
        assert!(SideTab::from_key("review") == SideTab::Review);
        assert!(SideTab::from_key("flow") == SideTab::Flow);
        // Unknown names fall back to the Queue.
        assert!(SideTab::from_key("nope") == SideTab::Queue);
    }
}
