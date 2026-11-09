//! The CLIs New agent offers (src/crew.ts CLI_PRESETS, shells left out as
//! today's agentPresets() leaves them out), whether each is installed, and the
//! choices New agent remembers in ~/.bytehive/native/prefs.json.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub program: &'static str,
    /// What "Don't ask before running commands or editing files" adds.
    pub skip_args: &'static [&'static str],
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "claude", label: "Claude Code", program: "claude", skip_args: &["--dangerously-skip-permissions"] },
    Preset { id: "codex", label: "Codex", program: "codex", skip_args: &["--yolo"] },
    Preset { id: "gemini", label: "Gemini", program: "gemini", skip_args: &["--yolo"] },
    Preset { id: "aider", label: "Aider", program: "aider", skip_args: &["--yes-always"] },
    Preset { id: "cursor", label: "Cursor Agent", program: "cursor-agent", skip_args: &[] },
    Preset { id: "opencode", label: "opencode", program: "opencode", skip_args: &[] },
    Preset { id: "qwen", label: "Qwen Code", program: "qwen", skip_args: &["--yolo"] },
    Preset { id: "copilot", label: "GitHub Copilot", program: "copilot", skip_args: &["--allow-all-tools"] },
    Preset { id: "goose", label: "Goose", program: "goose", skip_args: &[] },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Whether `program` is on PATH, as cmd.exe would find it.
pub fn installed(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return true };
    let exts = [".exe", ".cmd", ".bat", ".com", ".ps1", ""];
    std::env::split_paths(&path).any(|dir| exts.iter().any(|ext| dir.join(format!("{program}{ext}")).is_file()))
}

/// A task's first line, short enough for a queue row and a header (crew.ts taskTitle).
pub fn task_title(task: &str) -> Option<String> {
    let line = task.lines().map(str::trim).find(|l| !l.is_empty())?;
    let max = 64;
    if line.chars().count() > max {
        Some(format!("{}…", line.chars().take(max - 1).collect::<String>().trim_end()))
    } else {
        Some(line.to_string())
    }
}

/// The same folder however it is spelt: case, slashes, a trailing separator.
pub fn same_folder(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase();
    norm(a) == norm(b)
}

/// A path as people say it: the home folder reads as ~ (shortPath).
pub fn short_path(dir: &str) -> String {
    match std::env::var("USERPROFILE") {
        Ok(home) if dir.len() >= home.len() && same_folder(&dir[..home.len()], &home) => format!("~{}", &dir[home.len()..]),
        _ => dir.to_string(),
    }
}

/// A setup kept with Save as preset.
#[derive(Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub cli: String,
    pub count: usize,
    pub dir: String,
    pub skip_perms: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Prefs {
    /// A preset id, or "custom:<command>".
    #[serde(default)]
    pub last_cli: Option<String>,
    #[serde(default)]
    pub last_custom: String,
    #[serde(default)]
    pub skip_perms: bool,
    #[serde(default)]
    pub templates: Vec<Template>,
    /// Project folders opened lately, newest first (recents.ts keeps six).
    #[serde(default)]
    pub recents: Vec<String>,
    /// The project New agent starts in.
    #[serde(default)]
    pub project: Option<String>,
    /// Settings, as prefs.ts keeps them (defaults from DEFAULTS there).
    /// The CLI New agent picks: a preset id, or "last" for the last one used.
    #[serde(default = "last_used")]
    pub default_cli: String,
    /// How many agents New agent offers to start on one job.
    #[serde(default = "one")]
    pub default_count: usize,
    /// Seconds a new CLI gets to reach its prompt before its job is typed.
    #[serde(default = "four")]
    pub job_delay: u64,
    /// Terminal text size, in today's app's numbers (10 to 20).
    #[serde(default = "fifteen")]
    pub font_size: u32,
    /// Bring back last session's agents, stopped, at start.
    #[serde(default = "yes")]
    pub restore: bool,
    /// Every new agent in a git repo gets its own worktree and branch.
    #[serde(default)]
    pub worktree: bool,
    /// The first agent of a project with none is its Director.
    #[serde(default)]
    pub director_first: bool,
    /// Minimising tucks the window into the tray; agents keep running.
    #[serde(default)]
    pub hide_to_tray: bool,
    /// An agent out of usage hands its task to another CLI at once.
    #[serde(default = "yes")]
    pub auto_handover: bool,
    /// An agent that stopped at its limit is told to carry on when it resets.
    #[serde(default = "yes")]
    pub auto_resume: bool,
    /// A Windows notification when an agent starts waiting on you.
    #[serde(default = "yes")]
    pub notify_needs: bool,
    /// A Windows notification when an agent finishes.
    #[serde(default = "yes")]
    pub notify_done: bool,
    /// An agent that starts waiting on you comes onto the stage when the one
    /// there is not busy.
    #[serde(default)]
    pub jump_to_needs: bool,
    /// Changes opens when the agent on the stage finishes with changes.
    #[serde(default)]
    pub review_on_done: bool,
    /// Most agents the Grid shows: one terminal, the rest as cards.
    #[serde(default = "nine")]
    pub split_max: usize,
    /// A Windows notification when a plan passes 90% of a limit, and when an
    /// agent runs out.
    #[serde(default = "yes")]
    pub limit_notify: bool,
    /// Settings → Remote: the phone dashboard runs, other devices may reach
    /// it, and the port its page listens on (its terminal takes the next).
    #[serde(default)]
    pub remote_on: bool,
    #[serde(default)]
    pub remote_lan: bool,
    #[serde(default = "dash_port")]
    pub remote_port: u16,
}

fn dash_port() -> u16 {
    dashboard::DEFAULT_PORT
}

fn last_used() -> String {
    "last".to_string()
}
fn one() -> usize {
    1
}
fn four() -> u64 {
    4
}
fn fifteen() -> u32 {
    FONT_DEFAULT
}
fn yes() -> bool {
    true
}
fn nine() -> usize {
    9
}

/// How many agents the Grid may hold (prefs.ts SPLIT_SIZES).
pub const SPLIT_SIZES: [usize; 5] = [2, 3, 4, 6, 9];

/// The terminal text size range and default (settings.ts).
pub const FONT_MIN: u32 = 10;
pub const FONT_MAX: u32 = 20;
pub const FONT_DEFAULT: u32 = 15;
/// The job delay range in seconds (prefs.ts clamp).
pub const DELAY_MIN: u64 = 1;
pub const DELAY_MAX: u64 = 20;

/// recents.ts keeps this many.
pub const RECENTS: usize = 6;

fn store() -> Option<PathBuf> {
    Some(crate::fleet::data_dir()?.join("prefs.json"))
}

impl Default for Prefs {
    fn default() -> Self {
        // An empty object takes every serde default above.
        serde_json::from_str("{}").expect("prefs defaults")
    }
}

impl Prefs {
    pub fn load() -> Self {
        let mut prefs: Self = store().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        prefs.clamp();
        prefs
    }

    /// Keep hand-edited values in range, as prefs.ts clamp() does.
    pub fn clamp(&mut self) {
        if !(1..=3).contains(&self.default_count) {
            self.default_count = 1;
        }
        self.job_delay = self.job_delay.clamp(DELAY_MIN, DELAY_MAX);
        self.font_size = self.font_size.clamp(FONT_MIN, FONT_MAX);
        self.remote_port = self.remote_port.clamp(1024, 65534);
        if !SPLIT_SIZES.contains(&self.split_max) {
            self.split_max = nine();
        }
    }

    /// Settings back to how ByteHive ships; projects and presets stay.
    pub fn reset_settings(&mut self) {
        let fresh = Self::default();
        self.default_cli = fresh.default_cli;
        self.default_count = fresh.default_count;
        self.job_delay = fresh.job_delay;
        self.font_size = fresh.font_size;
        self.restore = fresh.restore;
        self.worktree = fresh.worktree;
        self.director_first = fresh.director_first;
        self.hide_to_tray = fresh.hide_to_tray;
        self.auto_handover = fresh.auto_handover;
        self.auto_resume = fresh.auto_resume;
        self.notify_needs = fresh.notify_needs;
        self.notify_done = fresh.notify_done;
        self.jump_to_needs = fresh.jump_to_needs;
        self.review_on_done = fresh.review_on_done;
        self.split_max = fresh.split_max;
        self.limit_notify = fresh.limit_notify;
        self.skip_perms = false;
        self.save();
    }

    /// Make `dir` the current project and the newest recent one.
    pub fn open_project(&mut self, dir: &str) {
        if dir.is_empty() {
            return;
        }
        self.recents.retain(|d| !same_folder(d, dir));
        self.recents.insert(0, dir.to_string());
        self.recents.truncate(RECENTS);
        self.project = Some(dir.to_string());
        self.save();
    }

    pub fn save(&self) {
        let Some(path) = store() else { return };
        if let (Some(dir), Ok(json)) = (path.parent(), serde_json::to_vec_pretty(self)) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(path, json);
        }
    }
}
