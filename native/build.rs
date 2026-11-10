//! The app icon in the exe (resource 1, which GPUI loads for its windows and
//! Explorer shows for the file and the taskbar), and the Company's shipped roles
//! (roles_gen.rs: every file under assets/company/roles, built in).

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    embed_resource::compile("app.rc", embed_resource::NONE).manifest_optional().expect("app icon");
    roles();
}

/// `SHIPPED`: (path under roles/ with forward slashes, the file's text), sorted by path.
fn roles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/company/roles");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files = Vec::new();
    walk(&dir, "", &mut files);
    files.sort();
    let mut out = String::from("pub const SHIPPED: &[(&str, &str)] = &[\n");
    for (rel, full) in files {
        out.push_str(&format!("    ({rel:?}, include_str!({full:?})),\n"));
    }
    out.push_str("];\n");
    let file = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("roles_gen.rs");
    std::fs::write(file, out).expect("roles_gen.rs");
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, String)>) {
    for e in std::fs::read_dir(dir).expect("assets/company/roles") {
        let e = e.expect("a role file");
        let name = e.file_name().to_string_lossy().into_owned();
        let sub = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        if e.file_type().expect("a role file").is_dir() {
            walk(&e.path(), &sub, out);
        } else {
            out.push((sub, e.path().to_string_lossy().into_owned()));
        }
    }
}
