//! Files (filetree.ts, codepanel.ts): the project's folders as a tree in a
//! panel beside the stage. The in-app editor is gone on purpose: a file opens
//! in VS Code. Folders load as they open and are read again every few
//! seconds, so what an agent writes shows up by itself.

mod view;

use std::os::windows::process::CommandExt as _;
use std::path::Path;

use super::*;

/// How often the open folders are read again.
const POLL: Duration = Duration::from_secs(3);
/// Files looked at, and found, at most when the filter searches.
const SEARCH_VISIT_CAP: usize = 12_000;
const SEARCH_HIT_CAP: usize = 300;
/// Heavy build and VCS folders, left out unless asked for. Dotfiles stay.
const HIDDEN_DIRS: [&str; 3] = ["node_modules", ".git", "target"];

#[derive(Clone, PartialEq)]
pub(super) struct Entry {
    pub name: String,
    pub is_dir: bool,
}

pub(super) struct Files {
    root: String,
    /// Each folder read so far, by its path under the root ("" is the root).
    children: HashMap<String, Vec<Entry>>,
    open: HashSet<String>,
    show_hidden: bool,
    selected: Option<String>,
    filter: Entity<InputState>,
    /// What the filter found, as paths under the root; none while it is empty.
    hits: Option<Vec<(String, bool)>>,
    search_no: u64,
    error: Option<String>,
    _sub: Subscription,
    _poll: Task<()>,
}

/// Directories first, then by name, case aside.
pub(super) fn sort_entries(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    entries
}

fn hidden(e: &Entry, show_hidden: bool) -> bool {
    !show_hidden && e.is_dir && HIDDEN_DIRS.contains(&e.name.as_str())
}

/// iconForFile: a glyph and a tint, so the tree is scanned by colour first.
pub(super) fn icon_for(name: &str) -> (&'static str, u32) {
    const GRAY: u32 = 0x6e6e78;
    let lower = name.to_lowercase();
    let by_name = match lower.as_str() {
        ".gitignore" | ".gitattributes" | ".gitmodules" => Some(("git", 0xe0813c)),
        "dockerfile" => Some(("gear", 0x519aba)),
        "makefile" => Some(("gear", GRAY)),
        "license" => Some(("doc", 0xc9c04a)),
        "readme.md" => Some(("doc", 0x519aba)),
        _ => None,
    };
    if let Some(hit) = by_name {
        return hit;
    }
    let ext = lower.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).map_or("", |(_, e)| e);
    let (blue, yellow, orange, green, purple, red, cyan, pink) = (0x519aba, 0xc9c04a, 0xe0813c, 0x7fbf4d, 0xa074c4, 0xcc5a5a, 0x4fb8b0, 0xe0709b);
    match ext {
        "ts" | "tsx" | "mts" | "cts" | "py" | "c" | "h" | "cpp" | "hpp" => ("code", blue),
        "js" | "jsx" | "mjs" | "cjs" => ("code", yellow),
        "vue" | "cs" => ("code", green),
        "svelte" | "swift" => ("code", orange),
        "rs" => ("gear", orange),
        "go" => ("code", cyan),
        "rb" | "java" => ("code", red),
        "kt" | "php" => ("code", purple),
        "sql" => ("db", pink),
        "json" | "jsonc" => ("braces", yellow),
        "toml" | "yaml" | "yml" | "ini" | "env" => ("gear", GRAY),
        "lock" => ("lock", GRAY),
        "css" | "less" => ("hash", blue),
        "scss" | "sass" => ("hash", pink),
        "html" | "htm" | "xml" => ("markup", orange),
        "md" | "markdown" => ("doc", blue),
        "txt" => ("doc", GRAY),
        "pdf" => ("doc", red),
        "sh" | "bash" | "zsh" | "bat" | "cmd" => ("term", green),
        "ps1" => ("term", blue),
        "svg" => ("image", yellow),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "avif" | "bmp" => ("image", purple),
        _ => ("file", GRAY),
    }
}

fn join(root: &str, rel: &str) -> std::path::PathBuf {
    if rel.is_empty() { Path::new(root).to_path_buf() } else { Path::new(root).join(rel.replace('/', "\\")) }
}

fn read_dir(root: &str, rel: &str) -> Result<Vec<Entry>, String> {
    let list = std::fs::read_dir(join(root, rel)).map_err(|e| e.to_string())?;
    Ok(sort_entries(list.flatten().map(|e| Entry { name: e.file_name().to_string_lossy().into_owned(), is_dir: e.file_type().is_ok_and(|t| t.is_dir()) }).collect()))
}

/// The files under the root whose name has the query, breadth first, capped.
fn search(root: &str, query: &str, show_hidden: bool) -> Vec<(String, bool)> {
    let q = query.to_lowercase();
    let mut hits = Vec::new();
    let mut queue = std::collections::VecDeque::from([String::new()]);
    let mut visited = 0;
    while let Some(rel) = queue.pop_front() {
        let Ok(entries) = read_dir(root, &rel) else { continue };
        for e in entries {
            visited += 1;
            if visited > SEARCH_VISIT_CAP || hits.len() >= SEARCH_HIT_CAP {
                return hits;
            }
            if hidden(&e, show_hidden) {
                continue;
            }
            let path = if rel.is_empty() { e.name.clone() } else { format!("{rel}/{}", e.name) };
            if e.name.to_lowercase().contains(&q) {
                hits.push((path.clone(), e.is_dir));
            }
            if e.is_dir {
                queue.push_back(path);
            }
        }
    }
    hits
}

/// Open a file in VS Code, else in whatever Windows opens it with.
fn open_in_editor(path: &Path) -> Result<(), String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let code = std::process::Command::new("cmd").args(["/c", "code", "-g"]).arg(path).creation_flags(CREATE_NO_WINDOW).status();
    if code.is_ok_and(|s| s.success()) {
        return Ok(());
    }
    std::process::Command::new("explorer").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

impl Inbox {
    pub(super) fn toggle_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.files.take().is_some() {
            cx.notify();
            return;
        }
        let Some(root) = self.project_dir() else {
            self.toast("Open a project folder first.", cx);
            return;
        };
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter files…"));
        let sub = cx.subscribe(&filter, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.search_files(cx);
            }
        });
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |this, cx| this.reload_files(cx)).is_err() {
                    break;
                }
            }
        });
        self.files = Some(Files { root, children: HashMap::new(), open: HashSet::new(), show_hidden: false, selected: None, filter, hits: None, search_no: 0, error: None, _sub: sub, _poll: poll });
        self.load_dir(String::new(), cx);
        cx.notify();
    }

    fn load_dir(&mut self, rel: String, cx: &mut Context<Self>) {
        let Some(f) = &self.files else { return };
        let root = f.root.clone();
        let read = {
            let rel = rel.clone();
            cx.background_spawn(async move { read_dir(&root, &rel) })
        };
        cx.spawn(async move |this, cx| {
            let got = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(f) = &mut this.files else { return };
                match got {
                    Ok(list) => {
                        if f.children.get(&rel) != Some(&list) {
                            f.children.insert(rel.clone(), list);
                            cx.notify();
                        }
                        if rel.is_empty() {
                            f.error = None;
                        }
                    }
                    Err(e) if rel.is_empty() => {
                        f.error = Some(format!("Couldn't read {}: {e}", f.root));
                        cx.notify();
                    }
                    // A folder that went away closes.
                    Err(_) => {
                        f.children.remove(&rel);
                        f.open.remove(&rel);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// Read the root and every open folder again.
    fn reload_files(&mut self, cx: &mut Context<Self>) {
        let Some(f) = &self.files else { return };
        let dirs: Vec<String> = std::iter::once(String::new()).chain(f.open.iter().cloned()).collect();
        for rel in dirs {
            self.load_dir(rel, cx);
        }
    }

    fn search_files(&mut self, cx: &mut Context<Self>) {
        let Some(f) = &mut self.files else { return };
        let query = f.filter.read(cx).value().trim().to_string();
        f.search_no += 1;
        if query.is_empty() {
            f.hits = None;
            cx.notify();
            return;
        }
        let (no, root, show) = (f.search_no, f.root.clone(), f.show_hidden);
        let found = cx.background_spawn(async move { search(&root, &query, show) });
        cx.spawn(async move |this, cx| {
            let hits = found.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(f) = this.files.as_mut().filter(|f| f.search_no == no) {
                    f.hits = Some(hits);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// A folder the filter found: the filter clears and the tree opens down to it.
    fn reveal_dir(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = &mut self.files else { return };
        f.hits = None;
        f.search_no += 1;
        f.filter.update(cx, |input, cx| input.set_value("", window, cx));
        let mut at = String::new();
        let mut opened = Vec::new();
        for part in rel.split('/') {
            at = if at.is_empty() { part.to_string() } else { format!("{at}/{part}") };
            f.open.insert(at.clone());
            opened.push(at.clone());
        }
        f.selected = Some(rel);
        for dir in opened {
            self.load_dir(dir, cx);
        }
        cx.notify();
    }

    /// A folder opens or closes; a file opens in VS Code.
    fn pick_file(&mut self, rel: String, is_dir: bool, cx: &mut Context<Self>) {
        let Some(f) = &mut self.files else { return };
        f.selected = Some(rel.clone());
        if is_dir {
            if !f.open.remove(&rel) {
                f.open.insert(rel.clone());
                self.load_dir(rel, cx);
            }
            cx.notify();
            return;
        }
        let path = join(&f.root, &rel);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        cx.notify();
        let opened = cx.background_spawn(async move { open_in_editor(&path) });
        cx.spawn(async move |this, cx| {
            if let Err(e) = opened.await {
                let _ = this.update(cx, |this, cx| this.toast(format!("Couldn't open {name}: {e}"), cx));
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, icon_for, sort_entries};

    #[test]
    fn folders_first_then_by_name() {
        let e = |name: &str, is_dir| Entry { name: name.to_string(), is_dir };
        let sorted: Vec<String> = sort_entries(vec![e("b.rs", false), e("Src", true), e("a.rs", false), e("assets", true)]).into_iter().map(|e| e.name).collect();
        assert_eq!(sorted, vec!["assets", "Src", "a.rs", "b.rs"]);
    }

    #[test]
    fn icons_by_name_then_extension() {
        assert_eq!(icon_for("main.rs").0, "gear");
        assert_eq!(icon_for(".gitignore").0, "git");
        assert_eq!(icon_for("README.md"), ("doc", 0x519aba));
        assert_eq!(icon_for(".env").0, "file", "a dotfile has no extension");
        assert_eq!(icon_for("x.unknown").0, "file");
    }
}
