//! Files dragged in from outside onto a terminal (bridges.ts
//! dropPathsIntoPane): their paths are typed in, a path with a space in
//! quotes, a space after the last so the next word does not run on, no Enter.
//! The terminal then takes the keyboard, as the Tauri app does.

use super::*;

/// The text a drop types: `C:\a.txt "C:\my file.png" `.
pub(super) fn dropped_text(paths: &[std::path::PathBuf]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let words: Vec<String> = paths
        .iter()
        .map(|p| {
            let s = p.to_string_lossy();
            if s.chars().any(char::is_whitespace) { format!("\"{s}\"") } else { s.into_owned() }
        })
        .collect();
    Some(words.join(" ") + " ")
}

impl Terminal {
    pub(super) fn on_files(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = dropped_text(paths.paths()) else { return };
        self.note_input();
        self.write(keys::text_records(&text, self.win32.load(Ordering::Relaxed)));
        window.focus(&self.focus, cx);
        self.claim_keys();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn types_paths_quoting_the_ones_with_spaces() {
        let text = super::dropped_text(&[PathBuf::from(r"C:\a.txt"), PathBuf::from(r"C:\my file.png")]);
        assert_eq!(text.as_deref(), Some("C:\\a.txt \"C:\\my file.png\" "));
        assert_eq!(super::dropped_text(&[]), None);
    }
}
