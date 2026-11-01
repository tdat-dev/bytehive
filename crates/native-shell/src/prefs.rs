//! The native shell's settings: one typed store with a default for every
//! value, so a fresh install behaves well. Lives in
//! `~/.maestro/native-prefs.json`; a missing or broken file is the defaults,
//! and each value is clamped on load like the web app's `prefs.ts` does.
//! Only settings the native shell acts on are here: no dead switches.

use std::path::{Path, PathBuf};

use eframe::egui::Color32;
use serde_json::{json, Value};

use crate::sessions_ui::Cli;
use crate::taskopts::{Flow, Perm};

/// The colour behind the terminals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Backdrop {
    #[default]
    Cinema,
    Graphite,
    Midnight,
    Warm,
}

impl Backdrop {
    pub const ALL: [Backdrop; 4] = [Backdrop::Cinema, Backdrop::Graphite, Backdrop::Midnight, Backdrop::Warm];

    pub fn label(self) -> &'static str {
        match self {
            Backdrop::Cinema => "Cinema",
            Backdrop::Graphite => "Graphite",
            Backdrop::Midnight => "Midnight",
            Backdrop::Warm => "Warm dark",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            // The terminal default background, so cells blend in.
            Backdrop::Cinema => Color32::from_rgb(11, 13, 18),
            Backdrop::Graphite => Color32::from_rgb(22, 23, 26),
            Backdrop::Midnight => Color32::from_rgb(10, 16, 32),
            Backdrop::Warm => Color32::from_rgb(24, 19, 16),
        }
    }

    fn from_label(s: &str) -> Backdrop {
        Backdrop::ALL.into_iter().find(|b| b.label() == s).unwrap_or_default()
    }
}

/// A saved crew: how a New task starts, under a name.
#[derive(Clone, Debug, PartialEq)]
pub struct Crew {
    pub name: String,
    pub cli: Cli,
    pub count: usize,
    pub worktree: bool,
    pub perm: Perm,
    pub flow: Flow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// Terminal glyph size before the density zoom.
    pub term_size: f32,
    pub backdrop: Backdrop,
    /// CLI a New task starts with; none means the last one used.
    pub default_cli: Option<Cli>,
    pub last_cli: Cli,
    pub default_count: usize,
    pub worktree: bool,
    pub perm: Perm,
    pub flow: Flow,
    /// Seconds a new CLI gets to reach its prompt before its task is typed.
    pub job_delay_s: u64,
    /// Reopen last run's projects on start.
    pub reopen_projects: bool,
    /// Tell an agent stopped by its plan's limit to carry on once it lifts.
    pub auto_resume: bool,
    pub crews: Vec<Crew>,
    /// New tasks set to start at a time of day.
    pub schedules: Vec<crate::schedule::Schedule>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            term_size: crate::theme::TERM_SIZE,
            backdrop: Backdrop::Cinema,
            default_cli: None,
            last_cli: Cli::Claude,
            default_count: 1,
            worktree: true,
            perm: Perm::Ask,
            flow: Flow::Direct,
            job_delay_s: crate::deliver::JOB_DELAY_MS / 1000,
            reopen_projects: true,
            auto_resume: true,
            crews: Vec::new(),
            schedules: Vec::new(),
        }
    }
}

pub fn cli_from_key(k: &str) -> Option<Cli> {
    Cli::all().into_iter().find(|c| c.program() == k)
}

fn crew_json(c: &Crew) -> Value {
    json!({ "name": c.name, "cli": c.cli.program(), "count": c.count, "worktree": c.worktree, "perm": c.perm.key(), "flow": c.flow.key() })
}

fn crew_from(v: &Value) -> Option<Crew> {
    Some(Crew {
        name: v["name"].as_str().filter(|s| !s.trim().is_empty())?.to_string(),
        cli: cli_from_key(v["cli"].as_str()?)?,
        count: (v["count"].as_u64().unwrap_or(1) as usize).clamp(1, crate::newtask::MAX_AGENTS),
        worktree: v["worktree"].as_bool().unwrap_or(true),
        perm: Perm::from_key(v["perm"].as_str().unwrap_or("")),
        flow: Flow::from_key(v["flow"].as_str().unwrap_or("")),
    })
}

impl Prefs {
    /// The CLI a New task opens with.
    pub fn start_cli(&self) -> Cli {
        self.default_cli.unwrap_or(self.last_cli)
    }

    pub fn job_delay_ms(&self) -> u64 {
        self.job_delay_s * 1000
    }

    pub fn to_json(&self) -> Value {
        json!({
            "termSize": self.term_size,
            "backdrop": self.backdrop.label(),
            "defaultCli": self.default_cli.map_or("last", |c| c.program()),
            "lastCli": self.last_cli.program(),
            "defaultCount": self.default_count,
            "worktree": self.worktree,
            "perm": self.perm.key(),
            "flow": self.flow.key(),
            "jobDelay": self.job_delay_s,
            "reopenProjects": self.reopen_projects,
            "autoResume": self.auto_resume,
            "crews": self.crews.iter().map(crew_json).collect::<Vec<_>>(),
            "schedules": self.schedules.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
        })
    }

    /// Every value from `v` when it has the right type, clamped; else the default.
    pub fn from_json(v: &Value) -> Prefs {
        let d = Prefs::default();
        Prefs {
            term_size: v["termSize"].as_f64().map_or(d.term_size, |x| (x as f32).clamp(11.0, 22.0)),
            backdrop: v["backdrop"].as_str().map_or(d.backdrop, Backdrop::from_label),
            default_cli: v["defaultCli"].as_str().and_then(cli_from_key),
            last_cli: v["lastCli"].as_str().and_then(cli_from_key).unwrap_or(d.last_cli),
            default_count: v["defaultCount"].as_u64().map_or(d.default_count, |n| (n as usize).clamp(1, crate::newtask::MAX_AGENTS)),
            worktree: v["worktree"].as_bool().unwrap_or(d.worktree),
            perm: v["perm"].as_str().map_or(d.perm, Perm::from_key),
            flow: v["flow"].as_str().map_or(d.flow, Flow::from_key),
            job_delay_s: v["jobDelay"].as_u64().map_or(d.job_delay_s, |n| n.clamp(1, 20)),
            reopen_projects: v["reopenProjects"].as_bool().unwrap_or(d.reopen_projects),
            auto_resume: v["autoResume"].as_bool().unwrap_or(d.auto_resume),
            crews: v["crews"].as_array().into_iter().flatten().filter_map(crew_from).collect(),
            schedules: v["schedules"].as_array().into_iter().flatten().filter_map(crate::schedule::Schedule::from_json).collect(),
        }
    }

    pub fn load(path: &Path) -> Prefs {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).map(|v| Prefs::from_json(&v)).unwrap_or_default()
    }

    /// Best effort: a failed write only loses the change for next time.
    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, serde_json::to_string_pretty(&self.to_json()).unwrap_or_default());
    }

    /// Save a crew under `name`, replacing one with the same name.
    pub fn save_crew(&mut self, crew: Crew) {
        self.crews.retain(|c| !c.name.eq_ignore_ascii_case(&crew.name));
        self.crews.push(crew);
    }
}

/// Where the settings live: `~/.maestro/native-prefs.json`.
pub fn prefs_path() -> Option<PathBuf> {
    crate::projects::store_path().map(|p| p.with_file_name("native-prefs.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let mut p = Prefs::default();
        p.default_cli = Some(Cli::Codex);
        p.perm = Perm::AcceptEdits;
        p.backdrop = Backdrop::Midnight;
        p.save_crew(Crew { name: "Race".into(), cli: Cli::Claude, count: 3, worktree: true, perm: Perm::Bypass, flow: Flow::Relay });
        assert_eq!(Prefs::from_json(&p.to_json()), p);
    }

    #[test]
    fn bad_values_fall_back_or_clamp() {
        let p = Prefs::from_json(&json!({ "termSize": 99, "jobDelay": 0, "defaultCount": 9, "defaultCli": "nope", "worktree": "yes", "crews": [{ "name": "" }, { "name": "x", "cli": "codex" }] }));
        assert_eq!(p.term_size, 22.0);
        assert_eq!(p.job_delay_s, 1);
        assert_eq!(p.default_count, crate::newtask::MAX_AGENTS);
        assert_eq!(p.default_cli, None, "unknown CLI means the last one used");
        assert!(p.worktree, "wrong type keeps the default");
        assert_eq!(p.crews.len(), 1, "a nameless crew is dropped");
        assert_eq!(Prefs::load(Path::new("Z:\\no\\prefs.json")), Prefs::default());
    }

    #[test]
    fn a_crew_with_the_same_name_is_replaced() {
        let mut p = Prefs::default();
        let crew = |n: usize| Crew { name: "Duo".into(), cli: Cli::Claude, count: n, worktree: true, perm: Perm::Ask, flow: Flow::Direct };
        p.save_crew(crew(2));
        p.save_crew(crew(3));
        assert_eq!(p.crews.len(), 1);
        assert_eq!(p.crews[0].count, 3);
    }

    #[test]
    fn start_cli_prefers_the_default_then_the_last() {
        let mut p = Prefs::default();
        p.last_cli = Cli::Muse;
        assert_eq!(p.start_cli(), Cli::Muse);
        p.default_cli = Some(Cli::Opencode);
        assert_eq!(p.start_cli(), Cli::Opencode);
    }
}
