//! ByteHive drawn by itself: Rust + GPUI, no WebView. For now one window with
//! a real terminal; the Inbox lands around it next.

mod boxdraw;
mod icons;
mod inbox;
mod keys;
mod terminal;

use std::borrow::Cow;

use agent_core::CommandSpec;
use gpui_kit::*;

use inbox::Inbox;
use terminal::Terminal;

const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Medium.ttf"),
];

/// `bytehive-native claude --resume` runs that; bare, a PowerShell.
fn command() -> CommandSpec {
    let mut args = std::env::args().skip(1);
    let mut spec = CommandSpec::new(args.next().unwrap_or_else(|| "powershell.exe".to_string()));
    for arg in args {
        spec = spec.arg(arg);
    }
    spec
}

fn main() {
    gpui_kit::application().with_assets(icons::Icons).run(|cx| {
        gpui_kit::init(cx);
        keys::install();
        cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()).expect("bundled fonts");
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("ByteHive".into()), appears_transparent: true, ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1536.0), px(864.0)), cx))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            let terminal = cx.new(|cx| Terminal::new(command(), cx).expect("terminal"));
            // BYTEHIVE_TERMINALS=5 keeps four more alive out of sight, as a
            // fleet does, to measure the app at the load it really carries.
            let extra = std::env::var("BYTEHIVE_TERMINALS").ok().and_then(|n| n.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1);
            let fleet: Vec<_> = (0..extra).map(|_| cx.new(|cx| Terminal::new(command(), cx).expect("terminal"))).collect();
            for hidden in &fleet {
                hidden.read(cx).keep_lines(terminal::SCROLLBACK_HIDDEN);
            }
            terminal.read(cx).claim_keys();
            let focus = terminal.read(cx).focus_handle();
            focus.focus(window, cx);
            cx.new(|_| Inbox::new(terminal, fleet))
        })
        .expect("window");
    });
}
