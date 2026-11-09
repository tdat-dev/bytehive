//! The Files panel drawn: Explorer head, the filter, the tree as one flat
//! list (filetree.css: 24px rows, 12px a level).

use gpui_kit::component::Sizable as _;

use super::*;

/// One visible line: its path under the root, depth and entry.
struct Row {
    rel: String,
    depth: usize,
    entry: Entry,
}

/// flattenTree: the open folders' entries in order, hidden ones left out.
fn flatten(f: &Files) -> Vec<Row> {
    fn walk(f: &Files, rel: &str, depth: usize, out: &mut Vec<Row>) {
        let Some(list) = f.children.get(rel) else { return };
        for e in list.iter().filter(|e| !hidden(e, f.show_hidden)) {
            let path = if rel.is_empty() { e.name.clone() } else { format!("{rel}/{}", e.name) };
            let open = e.is_dir && f.open.contains(&path);
            out.push(Row { rel: path.clone(), depth, entry: e.clone() });
            if open {
                walk(f, &path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(f, "", 0, &mut out);
    out
}

/// `.cp-btn`: a round tool button in the head.
fn tool(id: &'static str, name: &'static str, title: &'static str, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .tooltip(tip(title))
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .when(on, |b| b.bg(white(0.08)))
        .hover(|b| b.bg(white(0.08)))
        .child(icon(name, 15.0, color(if on { INK } else { INK_3 })))
}

impl Inbox {
    pub(in crate::inbox) fn files_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.files.as_ref()?;
        let mut rows: Vec<AnyElement> = Vec::new();
        let msg = |text: String| div().py(px(22.0)).px(px(16.0)).text_center().text_size(px(11.5)).line_height(relative(1.5)).text_color(color(INK_3)).child(text).into_any_element();
        if let Some(e) = &f.error {
            rows.push(msg(e.clone()));
        } else if let Some(hits) = &f.hits {
            if hits.is_empty() {
                rows.push(msg("No file has that in its name.".to_string()));
            }
            for (i, (rel, is_dir)) in hits.iter().enumerate() {
                let (name, dir) = rel.rsplit_once('/').map_or((rel.as_str(), ""), |(d, n)| (n, d));
                let entry = Entry { name: name.to_string(), is_dir: *is_dir };
                rows.push(self.file_row(i, rel.clone(), 0, &entry, Some(dir.to_string()), f, cx));
            }
        } else {
            let flat = flatten(f);
            if flat.is_empty() && f.children.contains_key("") {
                rows.push(msg("This folder is empty.".to_string()));
            }
            for (i, row) in flat.into_iter().enumerate() {
                rows.push(self.file_row(i, row.rel, row.depth, &row.entry, None, f, cx));
            }
        }
        let name = crate::fleet::project_of(&f.root);
        Some(
            div()
                .w(px(380.0))
                .flex_none()
                .mr(px(16.0))
                .mb(px(DOCK_H))
                .flex()
                .flex_col()
                .min_h_0()
                .overflow_hidden()
                .rounded(px(18.0))
                .bg(color(PANEL))
                .shadow(vec![ring(hairline())])
                .child(
                    div()
                        .h(px(48.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .pl(px(18.0))
                        .pr(px(10.0))
                        .child(div().flex_1().min_w_0().flex().items_baseline().gap(px(8.0)).child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Explorer")).child(div().truncate().text_size(px(12.0)).text_color(color(INK_3)).child(name)))
                        .child(tool("cp-refresh", "refresh", "Refresh", false).on_click(cx.listener(|this, _, _, cx| this.reload_files(cx))))
                        .child(tool("cp-collapse", "collapse", "Collapse all folders", false).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(f) = &mut this.files {
                                f.open.clear();
                            }
                            cx.notify();
                        })))
                        .child(tool("cp-hidden", "eye", "Show hidden & build folders (.git, node_modules, target)", f.show_hidden).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(f) = &mut this.files {
                                f.show_hidden = !f.show_hidden;
                            }
                            this.search_files(cx);
                            cx.notify();
                        })))
                        .child(tool("cp-close", "hide-panel", "Hide Files (Ctrl+Shift+E)", false).on_click(cx.listener(|this, _, window, cx| this.toggle_files(window, cx)))),
                )
                .child(
                    div()
                        .flex_none()
                        .mx(px(10.0))
                        .mb(px(6.0))
                        .h(px(30.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .rounded(px(8.0))
                        .bg(hsla(0.0, 0.0, 0.0, 0.3))
                        .shadow(vec![ring(white(0.07))])
                        .child(icon("search", 13.0, color(INK_3)))
                        .child(div().flex_1().min_w_0().child(Input::new(&f.filter).appearance(false).with_size(px(12.5 / 0.875)))),
                )
                .child(div().id("file-tree").flex_1().min_h_0().overflow_y_scroll().pt(px(4.0)).px(px(6.0)).pb(px(8.0)).text_size(px(12.5)).text_color(color(INK_2)).children(rows))
                .into_any_element(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn file_row(&self, i: usize, rel: String, depth: usize, e: &Entry, place: Option<String>, f: &Files, cx: &mut Context<Self>) -> AnyElement {
        let found = place.is_some();
        let open = e.is_dir && !found && f.open.contains(&rel);
        let sel = f.selected.as_deref() == Some(rel.as_str());
        let (glyph, tone) = if e.is_dir { (if open { "ft-dir-open" } else { "ft-dir" }, 0x79a6f6) } else { (icon_name(icon_for(&e.name).0), icon_for(&e.name).1) };
        let is_dir = e.is_dir;
        let pick = rel.clone();
        div()
            .id(("tw-row", i))
            .relative()
            .h(px(24.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .pl(px(6.0 + depth as f32 * 12.0))
            .pr(px(6.0))
            .rounded(px(6.0))
            .whitespace_nowrap()
            .cursor_pointer()
            .when(sel, |r| r.bg(tint(REVIEW, 0.12)).text_color(color(INK)).child(div().absolute().left_0().top(px(3.0)).bottom(px(3.0)).w(px(2.0)).rounded(px(2.0)).bg(color(REVIEW))))
            .when(!sel, |r| r.hover(|r| r.bg(white(0.04))))
            .tooltip(tip(if is_dir { rel.clone() } else { format!("{rel} (opens in VS Code)") }))
            .on_click(cx.listener(move |this, _, window, cx| {
                if found && is_dir {
                    this.reveal_dir(pick.clone(), window, cx);
                } else {
                    this.pick_file(pick.clone(), is_dir, cx);
                }
            }))
            .child(div().w(px(12.0)).flex_none().when(is_dir, |c| c.child(icon(if open { "chev-open" } else { "chev" }, 12.0, color(INK_3)))))
            .child(icon(glyph, 14.0, color(tone)))
            .child(div().min_w_0().truncate().when(is_dir, |n| n.font_weight(FontWeight::MEDIUM).text_color(color(INK_2))).child(e.name.clone()))
            .children(place.filter(|p| !p.is_empty()).map(|p| div().ml(px(7.0)).min_w_0().truncate().text_size(px(10.5)).text_color(color(INK_3)).child(p)))
            .into_any_element()
    }
}

fn icon_name(glyph: &str) -> &'static str {
    match glyph {
        "doc" => "ft-doc",
        "code" => "ft-code",
        "markup" => "ft-markup",
        "braces" => "ft-braces",
        "hash" => "ft-hash",
        "gear" => "ft-gear",
        "term" => "ft-term",
        "image" => "ft-image",
        "lock" => "ft-lock",
        "git" => "ft-git",
        "db" => "ft-db",
        _ => "ft-file",
    }
}
