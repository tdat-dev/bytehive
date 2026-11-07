//! ByteHive drawn by itself. For now, one window that proves GPUI builds and
//! runs here; the terminal and the Inbox land on top of it.

use gpui_kit::*;

struct Shell;

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x0f0f12)).text_color(rgb(0xe8e6e1)).child("ByteHive")
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Shell)).expect("window");
        })
        .detach();
    });
}
