//! A custom agent: any command on PATH (`gemini`, `ollama run llama3.1`)
//! started like the built-in CLIs, in its folder, worktree and all. It has
//! no session to resume, so a restart runs the command again.

use crate::sessions_ui::Cli;
use agent_core::{launch::launch_spec, CommandSpec};
use eframe::egui;

/// The command as words; double quotes keep spaces together.
pub fn split(cmd: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in cmd.trim().chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// How to start `cmd` in `dir`; nothing for an empty command.
pub fn spec(cmd: &str, dir: &str) -> Option<CommandSpec> {
    let words = split(cmd);
    let (program, args) = words.split_first()?;
    let mut spec = launch_spec(program, args);
    spec.cwd = Some(dir.to_string());
    Some(spec)
}

/// A short name for the agent's command: its program.
pub fn program_name(cmd: &str) -> String {
    let words = split(cmd);
    let first = words.first().map(String::as_str).unwrap_or("custom");
    first.rsplit(['/', '\\']).next().unwrap_or(first).to_string()
}

/// The Home composer's agent menu: the CLIs with their logos, then Custom
/// with a box for the command and the ones used lately.
pub fn home_menu(ui: &mut egui::Ui, installed: &[Cli], cli: &mut Cli, custom: &mut Option<String>, recent: &[String]) {
    ui.set_min_width(280.0);
    let logo = |ui: &egui::Ui, b: Option<&crate::brand::Brand>| b.map(|b| egui::Image::new((crate::brand::texture(ui.ctx(), b), egui::Vec2::splat(16.0))));
    for c in Cli::all() {
        let ok = installed.is_empty() || installed.contains(&c);
        let label = if ok { c.name().to_string() } else { format!("{}  (not installed)", c.name()) };
        let b = match logo(ui, Some(crate::brand::of_cli(c))) {
            Some(img) => egui::Button::image_and_text(img, label),
            None => egui::Button::new(label),
        };
        if ui.add_enabled(ok, b.selected(custom.is_none() && *cli == c)).clicked() {
            *cli = c;
            *custom = None;
            ui.close();
        }
    }
    ui.separator();
    ui.label(egui::RichText::new("Custom command").size(12.0).weak());
    let mut text = custom.clone().unwrap_or_default();
    let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("gemini, grok, ollama run llama3.1").font(egui::FontId::monospace(13.0)).desired_width(270.0));
    if r.changed() {
        *custom = Some(text).filter(|t| !t.is_empty());
    }
    for c in recent {
        let b = match logo(ui, crate::brand::of_command(c)) {
            Some(img) => egui::Button::image_and_text(img, c.as_str()),
            None => egui::Button::new(c.as_str()),
        };
        if ui.add(b.selected(custom.as_deref() == Some(c.as_str()))).clicked() {
            *custom = Some(c.clone());
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_split_like_a_shell() {
        assert_eq!(split("  ollama run  llama3.1 "), ["ollama", "run", "llama3.1"]);
        assert_eq!(split(r#""C:\My Tools\x.exe" --p "a b""#), [r"C:\My Tools\x.exe", "--p", "a b"]);
        assert!(split("   ").is_empty());
        assert!(spec("", "D:/x").is_none());
        assert_eq!(spec("gemini", "D:/x").unwrap().cwd.as_deref(), Some("D:/x"));
        assert_eq!(program_name(r"C:\t\grok.exe -x"), "grok.exe");
    }
}
