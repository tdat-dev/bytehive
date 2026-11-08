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

/// A setup kept with Save as preset.
#[derive(Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub cli: String,
    pub count: usize,
    pub dir: String,
    pub skip_perms: bool,
}

#[derive(Default, Serialize, Deserialize)]
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
}

fn store() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").ok()?;
    Some(PathBuf::from(home).join(".bytehive").join("native").join("prefs.json"))
}

impl Prefs {
    pub fn load() -> Self {
        store().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = store() else { return };
        if let (Some(dir), Ok(json)) = (path.parent(), serde_json::to_vec_pretty(self)) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(path, json);
        }
    }
}
