//! ByteHive drawn by itself: Rust + GPUI, no WebView. For now one window with
//! a real terminal; the Inbox lands around it next.

mod boxdraw;
mod keys;
mod terminal;

use std::borrow::Cow;

use agent_core::CommandSpec;
use gpui_kit::*;

use terminal::Terminal;

const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Medium.ttf"),
];

struct Shell {
    terminal: Entity<Terminal>,
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.terminal.clone())
    }
}

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
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        keys::install();
        cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()).expect("bundled fonts");
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("ByteHive".into()), ..Default::default() }),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            let terminal = cx.new(|cx| Terminal::new(command(), cx).expect("terminal"));
            let focus = terminal.read(cx).focus_handle();
            focus.focus(window, cx);
            cx.new(|_| Shell { terminal })
        })
        .expect("window");
    });
}
