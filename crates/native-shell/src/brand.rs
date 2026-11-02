//! The logos agents wear: each CLI's own mark (from lobe-icons, MIT), for
//! the four built-in CLIs and the AI CLIs people run as a custom command.

use crate::sessions_ui::Cli;
use eframe::egui::{self, Color32};

pub struct Brand {
    pub key: &'static str,
    /// Program names that mean this CLI.
    names: &'static [&'static str],
    png: &'static [u8],
    pub color: Color32,
}

macro_rules! brand {
    ($key:literal, [$($n:literal),*], $r:literal, $g:literal, $b:literal) => {
        Brand { key: $key, names: &[$($n),*], png: include_bytes!(concat!("../assets/clis/", $key, ".png")), color: Color32::from_rgb($r, $g, $b) }
    };
}

pub static BRANDS: &[Brand] = &[
    brand!("claude", ["claude"], 0xd9, 0x77, 0x57),
    brand!("codex", ["codex"], 0x10, 0xa3, 0x7f),
    brand!("opencode", ["opencode"], 0xf0, 0x88, 0x3e),
    brand!("muse", ["muse"], 0x1d, 0x7c, 0xf2),
    brand!("gemini", ["gemini"], 0x4e, 0x8c, 0xf7),
    brand!("grok", ["grok"], 0xd8, 0xd8, 0xdc),
    brand!("cursor", ["cursor-agent", "cursor"], 0xd8, 0xd8, 0xdc),
    brand!("ollama", ["ollama"], 0xd8, 0xd8, 0xdc),
    brand!("qwen", ["qwen"], 0x61, 0x5c, 0xed),
    brand!("copilot", ["copilot", "gh"], 0xd8, 0xd8, 0xdc),
    brand!("kimi", ["kimi"], 0x1e, 0x8b, 0xff),
    brand!("deepseek", ["deepseek"], 0x4d, 0x6b, 0xfe),
    brand!("hermes", ["hermes"], 0xd8, 0xd8, 0xdc),
    brand!("antigravity", ["agy", "antigravity"], 0x42, 0x85, 0xf4),
    brand!("amp", ["amp"], 0xd8, 0xd8, 0xdc),
    brand!("cline", ["cline"], 0xd8, 0xd8, 0xdc),
    brand!("kilo", ["kilo", "kilocode"], 0xd8, 0xd8, 0xdc),
    brand!("goose", ["goose"], 0xd8, 0xd8, 0xdc),
    brand!("mistral", ["vibe", "mistral"], 0xfa, 0x50, 0x0f),
    brand!("trae", ["trae", "trae-cli", "trae-agent"], 0x32, 0xf0, 0x8c),
    brand!("junie", ["junie"], 0x47, 0xe0, 0x6a),
    brand!("roo", ["roo", "roocode"], 0xd8, 0xd8, 0xdc),
    brand!("kiro", ["kiro", "kiro-cli"], 0x79, 0x5a, 0xf8),
    brand!("qoder", ["qoder", "qodercli"], 0x2a, 0xdb, 0x5c),
    brand!("codebuddy", ["codebuddy"], 0x6c, 0x5c, 0xf5),
];

pub fn of_cli(cli: Cli) -> &'static Brand {
    let key = match cli {
        Cli::Claude => "claude",
        Cli::Codex => "codex",
        Cli::Opencode => "opencode",
        Cli::Muse => "muse",
    };
    BRANDS.iter().find(|b| b.key == key).unwrap_or(&BRANDS[0])
}

/// The brand of a custom command, from its program: `gemini -m x`,
/// `C:\tools\grok.exe`, `npx @google/gemini-cli` all count.
pub fn of_command(cmd: &str) -> Option<&'static Brand> {
    let words = crate::custom::split(cmd);
    let mut it = words.iter();
    let mut word = it.next()?;
    if matches!(stem(word).as_str(), "npx" | "bunx" | "pnpx" | "uvx") {
        word = it.find(|w| !w.starts_with('-'))?;
    }
    let s = stem(word);
    let s = s.strip_suffix("-cli").unwrap_or(&s);
    BRANDS.iter().find(|b| b.names.contains(&s))
}

/// `C:\x\Gemini.CMD` or `@google/gemini-cli@1` as `gemini-cli`.
fn stem(word: &str) -> String {
    let base = word.rsplit(['/', '\\']).next().unwrap_or(word).to_lowercase();
    let base = base.split('@').find(|s| !s.is_empty()).unwrap_or("").to_string();
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(b) = base.strip_suffix(ext) {
            return b.to_string();
        }
    }
    base
}

/// The brand's logo, loaded once.
pub fn texture(ctx: &egui::Context, b: &Brand) -> egui::TextureId {
    let id = egui::Id::unique(("cli-logo", b.key));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t.id();
    }
    let img = image::load_from_memory(b.png).map(|i| i.into_rgba8()).unwrap_or_default();
    let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
    let t = ctx.load_texture(format!("cli-logo-{}", b.key), color, egui::TextureOptions::LINEAR);
    let tid = t.id();
    ctx.data_mut(|d| d.insert_temp(id, t));
    tid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_commands_find_their_logo() {
        let key = |c: &str| of_command(c).map(|b| b.key);
        assert_eq!(key("gemini -m pro"), Some("gemini"));
        assert_eq!(key(r"C:\tools\Grok.EXE"), Some("grok"));
        assert_eq!(key("npx -y @google/gemini-cli"), Some("gemini"));
        assert_eq!(key("ollama run llama3.1"), Some("ollama"));
        assert_eq!(key("cursor-agent"), Some("cursor"));
        assert_eq!(key("my-tool --x"), None);
        assert_eq!(key(""), None);
    }

    #[test]
    fn every_logo_decodes() {
        for b in BRANDS {
            assert!(image::load_from_memory(b.png).is_ok(), "{}", b.key);
        }
    }
}
