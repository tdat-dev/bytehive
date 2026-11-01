//! The approved design's look (design/redesign/maestro-inbox.html, D.Cinema)
//! applied to egui as a whole: soft floating panels on a black ground, quiet
//! pills for controls, white for the one primary action, color only for
//! state. `install_style` sets egui's own widgets (buttons, inputs, combos,
//! selection, scrollbars) so every sheet carries the design, not egui's
//! defaults; the constants are the design's tokens the shell paints with.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

use crate::theme::{BG, DIM, INK3, SURFACE, TEXT};

/// --panel: the floating panels (queue, stage, columns).
pub const PANEL: Color32 = Color32::from_rgb(14, 15, 17);
/// --raise: a selected row, cards inside a panel.
pub const RAISE: Color32 = SURFACE;
/// --line-soft / --line: hairlines.
pub const LINE_SOFT: Color32 = Color32::from_rgba_premultiplied(15, 15, 15, 15);
pub const LINE: Color32 = Color32::from_rgba_premultiplied(23, 23, 23, 23);
/// The space around the panels and above the dock.
pub const GUTTER: f32 = 16.0;
pub const DOCK_H: f32 = 84.0;

/// A round icon button (.ibtn): `size` square, the icon in ink-3, a white
/// 6% wash on hover; `danger` turns the hover red (remove).
pub fn icon_btn(ui: &mut egui::Ui, icon: crate::icons::Icon, size: f32, on: bool, danger: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let hot = r.hovered() || on;
    let (fill, ink) = match (hot, danger && r.hovered()) {
        (_, true) => (Color32::from_rgba_unmultiplied(242, 118, 107, 41), crate::theme::DEL),
        (true, _) => (Color32::from_white_alpha(if on { 26 } else { 15 }), TEXT),
        _ => (Color32::TRANSPARENT, INK3),
    };
    ui.painter().rect_filled(rect, size / 2.0, fill);
    crate::icons::paint_in(ui.painter(), rect, 15.0, icon, ink, 1.6);
    r
}

/// A state's tint for pills and rows: the color at 12%.
pub fn tint(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 31)
}

/// Geist at 500, the web app's weight for names, pills and tabs.
pub fn w500(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

/// Geist at 600: titles, the sentence's lead, primary buttons.
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

/// egui's widgets in the design's dress. Call once at startup.
pub fn install_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(TextStyle::Body, FontId::proportional(13.5));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(13.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
        s.text_styles.insert(TextStyle::Heading, medium(22.0));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(12.5));
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.combo_height = 320.0;
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;
        let v = &mut s.visuals;
        v.dark_mode = true;
        // egui thickens light text on dark (2c - c^2), which reads bold next
        // to the browser's text; a gentler curve reads like the browser.
        v.text_options.color_transfer_function = egui::epaint::FontColorTransferFunction::Gamma(0.7);
        v.panel_fill = BG;
        v.window_fill = Color32::from_rgb(20, 21, 24);
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = CornerRadius::same(18);
        v.menu_corner_radius = CornerRadius::same(12);
        v.extreme_bg_color = Color32::from_rgb(10, 10, 12);
        v.faint_bg_color = Color32::from_white_alpha(6);
        v.override_text_color = None;
        v.hyperlink_color = TEXT;
        v.selection.bg_fill = Color32::from_white_alpha(46);
        v.selection.stroke = Stroke::new(1.0, TEXT);
        v.text_cursor.stroke = Stroke::new(2.0, TEXT);
        v.popup_shadow = egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) };
        v.window_shadow = v.popup_shadow;
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = SURFACE;
        w.noninteractive.weak_bg_fill = SURFACE;
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE_SOFT);
        w.noninteractive.fg_stroke = Stroke::new(1.0, DIM);
        for (st, fill, fg) in [(&mut w.inactive, 13u8, DIM), (&mut w.hovered, 26, TEXT), (&mut w.active, 36, TEXT), (&mut w.open, 26, TEXT)] {
            st.bg_fill = Color32::from_white_alpha(fill);
            st.weak_bg_fill = Color32::from_white_alpha(fill);
            st.bg_stroke = Stroke::NONE;
            st.fg_stroke = Stroke::new(1.0, fg);
            st.corner_radius = CornerRadius::same(10);
            st.expansion = 0.0;
        }
        w.inactive.fg_stroke = Stroke::new(1.0, INK3.lerp_to_gamma(TEXT, 0.5));
    });
}

/// A quiet round button (.hbtn / .tbtn): label, white 5% fill.
pub fn hbtn(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let color = if on { TEXT } else { DIM };
    let fill = if on { Color32::from_white_alpha(31) } else { Color32::from_white_alpha(13) };
    ui.add(egui::Button::new(egui::RichText::new(label).size(12.5).color(color)).fill(fill).corner_radius(999.0).min_size(egui::vec2(0.0, 30.0)))
}

/// The white primary pill (.newbtn / .btn.primary), with an optional key.
pub fn primary(ui: &mut egui::Ui, label: &str, key: Option<&str>, h: f32) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    job.append(label, 0.0, egui::TextFormat::simple(medium(13.0), Color32::BLACK));
    if let Some(k) = key {
        job.append(&format!("  {k}"), 0.0, egui::TextFormat::simple(FontId::monospace(11.0), Color32::from_gray(110)));
    }
    ui.add(egui::Button::new(job).fill(Color32::WHITE).corner_radius(999.0).min_size(egui::vec2(0.0, h)))
}
