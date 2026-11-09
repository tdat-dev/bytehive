//! The app icon in the exe (resource 1, which GPUI loads for its windows and
//! Explorer shows for the file and the taskbar).

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    embed_resource::compile("app.rc", embed_resource::NONE).manifest_optional().expect("app icon");
}
