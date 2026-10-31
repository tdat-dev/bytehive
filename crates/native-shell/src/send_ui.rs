//! Send box: one line to type into any running agent, mirroring the web
//! app's command bar targeted mode. Pick the pane, type, Enter to send.
//! Pure bytes shaping is unit-tested; painting is covered by the grid
//! headless test pattern (run the app to see it).

use eframe::egui;

use crate::term_pane::Pane;

pub struct SendPanel {
    target: Option<String>,
    text: String,
}

impl Default for SendPanel {
    fn default() -> Self {
        SendPanel { target: None, text: String::new() }
    }
}

/// Bytes for the wire: the text plus Enter, or nothing for blank input.
pub fn send_bytes(text: &str) -> Option<Vec<u8>> {
    let t = text.trim();
    if t.is_empty() {
        None
    } else {
        let mut out = t.as_bytes().to_vec();
        out.push(b'\r');
        Some(out)
    }
}

impl SendPanel {
    pub fn show(&mut self, ui: &mut egui::Ui, panes: &[Pane], registry: &mut agent_core::Registry) {
        if panes.is_empty() {
            return;
        }
        if self.target.as_ref().is_none_or(|id| !panes.iter().any(|p| &p.id == id)) {
            self.target = panes.first().map(|p| p.id.clone());
        }
        let current = self
            .target
            .as_ref()
            .and_then(|id| panes.iter().find(|p| &p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_default();
        ui.separator();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("send-to")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for p in panes {
                        let sel = self.target.as_deref() == Some(p.id.as_str());
                        if ui.selectable_label(sel, &p.name).clicked() {
                            self.target = Some(p.id.clone());
                        }
                    }
                });
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.text)
                    .hint_text("Type a message…")
                    .desired_width(f32::INFINITY),
            );
            let send = ui.small_button("Send").clicked()
                || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if send {
                if let (Some(bytes), Some(id)) = (send_bytes(&self.text), self.target.clone()) {
                    let _ = registry.write_input(&id, &bytes);
                    self.text.clear();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_never_sends() {
        assert_eq!(send_bytes(""), None);
        assert_eq!(send_bytes("   \n "), None);
    }

    #[test]
    fn text_goes_with_enter() {
        assert_eq!(send_bytes("hi"), Some(b"hi\r".to_vec()));
        assert_eq!(send_bytes("  hi  "), Some(b"hi\r".to_vec()));
    }

    #[test]
    fn panel_paints_without_panic() {
        use agent_core::launch::launch_spec;
        use crate::term_pane::Pane;
        let spec = || launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        let panes = vec![
            Pane::new("a".to_string(), "alpha".to_string(), spec(), 80, 24),
            Pane::new("b".to_string(), "bea".to_string(), spec(), 80, 24),
        ];
        let mut registry = agent_core::Registry::new();
        let mut send = SendPanel::default();
        egui::__run_test_ui(|ui| {
            send.show(ui, &panes, &mut registry);
        });
        // Defaults to the first pane, keeps the draft until sent.
        assert_eq!(send.target.as_deref(), Some("a"));
        assert!(send.text.is_empty());
    }
}
