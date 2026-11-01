//! The Files drawer (top bar "Files", or Ctrl+K "Files on this branch"),
//! after the approved design: a floating panel on the right with the
//! project and branch on top, a search box, the branch's tree with changed
//! files marked, and a note that files open in your editor. The listing is
//! read off the UI thread and refreshed every few seconds while open, so a
//! file an agent adds shows up on its own.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use agent_core::git::current_branch;
use agent_core::gitfiles::{file_statuses, listed_files};
use eframe::egui::{self, Color32, RichText, Stroke};

use crate::filetree::{visible_rows, Row};
use crate::theme::{ADD, AMBER, DEL, INK3, R_XL, SHEET, TEXT};

/// How often an open drawer re-reads the branch.
const REFRESH: Duration = Duration::from_secs(3);
const ROW_H: f32 = 26.0;

/// A folder the drawer can show: a project, or an agent's worktree.
#[derive(Clone, PartialEq)]
pub struct FileSource {
    pub label: String,
    pub dir: String,
}

struct Listing {
    dir: String,
    branch: Option<String>,
    files: Vec<String>,
    marks: HashMap<String, char>,
}

fn read_listing(dir: String) -> Listing {
    let p = Path::new(&dir);
    let marks = file_statuses(p).into_iter().map(|f| (f.path, f.mark)).collect();
    Listing { branch: current_branch(p), files: listed_files(p), marks, dir }
}

pub struct FilesDrawer {
    pub open: bool,
    pub source: Option<String>,
    listing: Option<Listing>,
    open_dirs: HashSet<String>,
    query: String,
    pending: bool,
    last: Instant,
    tx: Sender<Listing>,
    rx: Receiver<Listing>,
    pub editor: Option<&'static str>,
}

impl Default for FilesDrawer {
    fn default() -> Self {
        let (tx, rx) = channel();
        FilesDrawer { open: false, source: None, listing: None, open_dirs: HashSet::new(), query: String::new(), pending: false, last: Instant::now(), tx, rx, editor: None }
    }
}

fn mark_color(m: char) -> Color32 {
    match m {
        'A' => ADD,
        'D' => DEL,
        _ => AMBER,
    }
}

impl FilesDrawer {
    pub fn toggle(&mut self, default_source: Option<String>) {
        self.open = !self.open;
        if self.open {
            if self.source.is_none() {
                self.source = default_source;
            }
            self.load();
            if self.editor.is_none() {
                self.editor = Some(crate::editor::editor_name());
            }
        }
    }

    fn load(&mut self) {
        let Some(dir) = self.source.clone() else { return };
        if self.pending {
            return;
        }
        self.pending = true;
        self.last = Instant::now();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(read_listing(dir));
        });
    }

    /// Paint the drawer. A file click hands back its full path to open.
    pub fn show(&mut self, ctx: &egui::Context, sources: &[FileSource]) -> Option<PathBuf> {
        if !self.open {
            return None;
        }
        while let Ok(l) = self.rx.try_recv() {
            self.pending = false;
            if Some(&l.dir) == self.source.as_ref() {
                self.listing = Some(l);
            }
        }
        if self.last.elapsed() >= REFRESH {
            self.load();
        }
        ctx.request_repaint_after(REFRESH);
        let mut picked = None;
        let screen = ctx.content_rect();
        let size = egui::vec2(360.0_f32.min(screen.width() - 32.0), screen.height() - 32.0);
        let frame = egui::Frame::new()
            .fill(SHEET)
            .corner_radius(R_XL)
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(20)))
            .shadow(egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) });
        egui::Area::new(egui::Id::unique("files-drawer"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.set_min_size(size);
                    ui.set_max_size(size);
                    picked = self.body(ui, sources);
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
        }
        picked
    }

    fn body(&mut self, ui: &mut egui::Ui, sources: &[FileSource]) -> Option<PathBuf> {
        let mut picked = None;
        egui::Frame::NONE.inner_margin(egui::Margin { left: 18, right: 14, top: 16, bottom: 8 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Files").size(15.0).color(TEXT));
                    let label = sources.iter().find(|s| Some(&s.dir) == self.source.as_ref()).map_or("pick a folder".to_string(), |s| s.label.clone());
                    let branch = self.listing.as_ref().and_then(|l| l.branch.clone()).unwrap_or_default();
                    egui::ComboBox::from_id_salt("files-source")
                        .selected_text(RichText::new(if branch.is_empty() { label } else { format!("{label} / {branch}") }).size(12.0).monospace().color(INK3))
                        .show_ui(ui, |ui| {
                            for s in sources {
                                if ui.selectable_label(Some(&s.dir) == self.source.as_ref(), &s.label).on_hover_text(&s.dir).clicked() {
                                    self.source = Some(s.dir.clone());
                                    self.listing = None;
                                    self.open_dirs.clear();
                                    self.pending = false;
                                    self.load();
                                }
                            }
                        });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Close").size(12.5).color(INK3)).frame(false)).clicked() {
                        self.open = false;
                    }
                });
            });
        });
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(14, 4)).show(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("Find a file\u{2026}").desired_width(f32::INFINITY).margin(egui::vec2(10.0, 8.0)));
        });
        let note_h = 44.0;
        let list_h = (ui.available_height() - note_h).max(60.0);
        let rows: Vec<Row> = match &self.listing {
            Some(l) => visible_rows(&l.files, &l.marks, &self.open_dirs, &self.query),
            None => Vec::new(),
        };
        ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
            if self.listing.is_none() {
                ui.add_space(12.0);
                ui.label(RichText::new(if self.source.is_some() { "Reading the branch\u{2026}" } else { "Pick a folder above." }).size(13.0).color(INK3));
                return;
            }
            if rows.is_empty() {
                ui.add_space(12.0);
                let q = self.query.trim();
                ui.label(RichText::new(if q.is_empty() { "No files here (is it a git repo?).".to_string() } else { format!("No file matches \u{201C}{q}\u{201D}.") }).size(13.0).color(INK3));
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for r in &rows[range] {
                    if let Some(p) = self.row(ui, r) {
                        picked = Some(p);
                    }
                }
            });
        });
        let line_y = ui.cursor().min.y;
        ui.painter().hline(ui.max_rect().x_range(), line_y, Stroke::new(1.0, Color32::from_white_alpha(15)));
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
            let ed = self.editor.unwrap_or("your editor");
            ui.label(RichText::new(format!("Files open in {ed}. Maestro shows the branch; it doesn't try to be a second IDE.")).size(12.0).color(INK3));
        });
        picked.and_then(|rel| self.source.as_ref().map(|d| Path::new(d).join(rel)))
    }

    /// One tree row; a folder toggles open, a file hands back its path.
    fn row(&mut self, ui: &mut egui::Ui, r: &Row) -> Option<String> {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), egui::Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect.shrink2(egui::vec2(8.0, 1.0)), 8.0, Color32::from_white_alpha(12));
        }
        let x = rect.left() + 16.0 + r.depth as f32 * 14.0;
        let font = egui::FontId::monospace(12.5);
        let (glyph, color) = if r.dir { (if r.open { "\u{25BE} " } else { "\u{25B8} " }, INK3) } else { ("  ", if resp.hovered() { TEXT } else { crate::theme::DIM }) };
        let name = format!("{glyph}{}{}", r.name, if r.dir { "/" } else { "" });
        ui.painter().with_clip_rect(rect.shrink2(egui::vec2(28.0, 0.0))).text(egui::pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, name, font.clone(), color);
        if let Some(m) = r.mark {
            let shown = if m == '*' { "\u{2022}".to_string() } else { m.to_string() };
            ui.painter().text(egui::pos2(rect.right() - 20.0, rect.center().y), egui::Align2::CENTER_CENTER, shown, font, mark_color(m));
        }
        if !resp.clicked() {
            return None;
        }
        if r.dir {
            if !self.open_dirs.remove(&r.path) {
                self.open_dirs.insert(r.path.clone());
            }
            None
        } else {
            Some(r.path.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawer_reads_this_repo_and_paints() {
        let mut d = FilesDrawer::default();
        d.source = Some(env!("CARGO_MANIFEST_DIR").to_string());
        d.open = true;
        d.load();
        let l = d.rx.recv_timeout(Duration::from_secs(20)).expect("listing arrives");
        assert!(l.files.iter().any(|f| f.ends_with("files_ui.rs")), "lists files of the repo");
        d.listing = Some(l);
        d.pending = false;
        let ctx = egui::Context::default();
        let src = vec![FileSource { label: "native-shell".into(), dir: d.source.clone().unwrap() }];
        ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(d.show(ui.ctx(), &src).is_none(), "nothing opens without a click");
        })
        .drop_without_applying_deltas();
    }
}
