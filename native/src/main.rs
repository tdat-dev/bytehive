//! ByteHive drawn by itself: Rust + GPUI, no WebView. One window: the Inbox,
//! with the live fleet in its queue and the focused agent on its stage.

// A windowed app: Windows opens no console beside it (it did, and the tray's
// "Error removing system tray icon" showed in it). Debug builds keep the
// console for their logs. Everything the app starts in the background runs
// with CREATE_NO_WINDOW, so nothing flashes a console either.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod board;
mod boxdraw;
mod brand;
mod bridge;
mod cleanenv;
mod cloud;
mod company;
mod conductor;
mod finishnote;
mod fleet;
mod follow;
mod gitexclude;
mod gridcards;
mod gridfit;
mod gridpins;
mod guard;
mod harness;
mod icons;
mod import;
mod inbox;
mod keylog;
mod keymode;
mod keys;
mod laws;
mod limits;
mod memory;
mod notify;
mod office3d;
mod pipeline;
mod plan;
mod planparse;
mod presets;
mod project_usage;
mod review;
mod schedule;
mod single;
mod stall;
mod terminal;
mod transcript;
mod tray;
mod updater;
mod usage;
mod worktree;

use std::borrow::Cow;

use gpui_kit::*;

use inbox::Inbox;

const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Medium.ttf"),
];

fn main() {
    // Opened from inside an agent: every terminal still gets Windows' own
    // environment, not that session's (NO_COLOR and the rest). Before any thread.
    cleanenv::restore();
    // Started by Chrome as the extension's native messaging host: relay, no window.
    if browser_hub::host::wanted() {
        browser_hub::host::run();
    }
    if !single::claim() {
        return;
    }
    // The CLIs that read the bytehive MCP server only from their own config file
    // point at this app's server (a few file reads, off the UI thread).
    std::thread::spawn(harness::sync_globals);
    gpui_kit::application().with_assets(icons::Icons).run(|cx| {
        gpui_kit::init(cx);
        // The kit's own widgets (the New agent folder field) follow its theme:
        // dark, like the rest, never the OS's light mode.
        gpui_kit::component::theme::Theme::change(gpui_kit::component::theme::ThemeMode::Dark, None, cx);
        keys::install();
        stall::start(cx);
        cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()).expect("bundled fonts");
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("ByteHive".into()), appears_transparent: true, ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1536.0), px(864.0)), cx))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| cx.new(Inbox::new))
        .expect("window");
    });
}
