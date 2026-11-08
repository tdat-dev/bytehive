//! Each agent wears the logo of the CLI it runs, worked out from what it runs,
//! as today's src/clibrand.ts does: its program, past anything that only
//! launches it (npx, cmd /c). A CLI with no logo gets its first letter; a plain
//! shell gets a prompt.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use gpui_kit::{Image, ImageFormat};

pub struct Brand {
    pub key: &'static str,
    names: &'static [&'static str],
    pub color: u32,
    /// The logo brings its own square, so it fills the tile.
    pub tile: bool,
}

const MONO: u32 = 0xd8d8dc;

pub const BRANDS: &[Brand] = &[
    Brand { key: "claude", names: &["claude", "claude-code"], color: 0xd97757, tile: false },
    Brand { key: "codex", names: &["codex"], color: 0x10a37f, tile: true },
    Brand { key: "opencode", names: &["opencode", "opencode-ai"], color: 0xf0883e, tile: false },
    Brand { key: "muse", names: &["muse"], color: 0x1d7cf2, tile: false },
    Brand { key: "gemini", names: &["gemini"], color: 0x4e8cf7, tile: false },
    Brand { key: "aider", names: &["aider", "aider-chat"], color: 0x0f9d58, tile: true },
    Brand { key: "grok", names: &["grok"], color: MONO, tile: false },
    Brand { key: "cursor", names: &["cursor-agent", "cursor"], color: MONO, tile: false },
    Brand { key: "ollama", names: &["ollama"], color: MONO, tile: false },
    Brand { key: "qwen", names: &["qwen", "qwen-code"], color: 0x615ced, tile: false },
    Brand { key: "copilot", names: &["copilot"], color: MONO, tile: false },
    Brand { key: "kimi", names: &["kimi"], color: 0x1e8bff, tile: false },
    Brand { key: "deepseek", names: &["deepseek"], color: 0x4d6bfe, tile: false },
    Brand { key: "hermes", names: &["hermes"], color: MONO, tile: false },
    Brand { key: "antigravity", names: &["agy", "antigravity"], color: 0x4285f4, tile: false },
    Brand { key: "amp", names: &["amp"], color: MONO, tile: false },
    Brand { key: "cline", names: &["cline"], color: MONO, tile: false },
    Brand { key: "kilo", names: &["kilo", "kilocode"], color: MONO, tile: false },
    Brand { key: "goose", names: &["goose"], color: MONO, tile: false },
    Brand { key: "mistral", names: &["vibe", "mistral"], color: 0xfa500f, tile: false },
    Brand { key: "trae", names: &["trae", "trae-agent"], color: 0x32f08c, tile: false },
    Brand { key: "junie", names: &["junie"], color: 0x47e06a, tile: false },
    Brand { key: "roo", names: &["roo", "roocode"], color: MONO, tile: false },
    Brand { key: "kiro", names: &["kiro"], color: 0x795af8, tile: true },
    Brand { key: "qoder", names: &["qoder", "qodercli"], color: 0x2adb5c, tile: false },
    Brand { key: "codebuddy", names: &["codebuddy"], color: 0x6c5cf5, tile: true },
];

macro_rules! logos {
    ($($key:literal),* $(,)?) => {
        const LOGOS: &[(&str, &[u8])] = &[$(($key, include_bytes!(concat!("../assets/clis/", $key, ".png")))),*];
    };
}

logos!(
    "amp", "antigravity", "claude", "cline", "codebuddy", "codex", "copilot", "cursor", "deepseek", "gemini", "goose", "grok", "hermes", "junie", "kilo", "kimi", "kiro", "mistral",
    "muse", "ollama", "opencode", "qoder", "qwen", "roo", "trae",
);

/// Programs that only launch the CLI, and the words they take before naming it.
const LAUNCHERS: &[&str] = &["npx", "bunx", "pnpx", "uvx", "pipx", "pnpm", "yarn", "bun", "npm", "uv", "wsl", "cmd", "gh"];
const LAUNCH_WORDS: &[&str] = &["run", "dlx", "x", "exec", "tool", "/c"];
const SHELLS: &[&str] = &["powershell", "pwsh", "cmd"];

/// `C:\x\Gemini.CMD` or `@google/gemini-cli@1` as `gemini-cli`.
fn stem(word: &str) -> String {
    let base = word.rsplit(['\\', '/']).next().unwrap_or("").to_ascii_lowercase();
    let name = base.split('@').find(|s| !s.is_empty()).unwrap_or("").to_string();
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(cut) = name.strip_suffix(ext) {
            return cut.to_string();
        }
    }
    name
}

/// The word of a command that names the CLI, past anything that only launches it.
fn program_word(words: &[&str]) -> Option<String> {
    words.iter().map(|w| stem(w)).zip(words).find(|(s, w)| !LAUNCHERS.contains(&s.as_str()) && !LAUNCH_WORDS.contains(&w.to_ascii_lowercase().as_str()) && !w.starts_with('-')).map(|(s, _)| s)
}

/// What an agent's tile shows.
pub enum Mark {
    Logo { image: Arc<Image>, fills: bool, color: u32 },
    Shell,
    Letter { letter: String, color: u32 },
}

fn images() -> &'static HashMap<&'static str, Arc<Image>> {
    static IMAGES: OnceLock<HashMap<&'static str, Arc<Image>>> = OnceLock::new();
    IMAGES.get_or_init(|| LOGOS.iter().map(|(key, bytes)| (*key, Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec())))).collect())
}

pub fn brand_of(command: &str) -> Option<&'static Brand> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let word = program_word(&words)?;
    let s = word.strip_suffix("-cli").unwrap_or(&word).to_string();
    BRANDS.iter().find(|b| b.names.contains(&s.as_str()))
}

/// The mark for an agent running `command` (program and args, as typed).
pub fn mark_of(command: &str) -> Mark {
    let words: Vec<&str> = command.split_whitespace().collect();
    let word = program_word(&words).unwrap_or_default();
    if SHELLS.contains(&word.as_str()) {
        return Mark::Shell;
    }
    match brand_of(command) {
        Some(b) => match images().get(b.key) {
            Some(image) => Mark::Logo { image: image.clone(), fills: b.tile, color: b.color },
            None => Mark::Letter { letter: b.key[..1].to_ascii_uppercase(), color: b.color },
        },
        None => Mark::Letter { letter: word.chars().next().filter(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into()), color: 0x8c8c96 },
    }
}

/// The tile's tint for an agent running `command`.
pub fn color_of(command: &str) -> u32 {
    match mark_of(command) {
        Mark::Logo { color, .. } | Mark::Letter { color, .. } => color,
        Mark::Shell => 0x8c8c96,
    }
}
