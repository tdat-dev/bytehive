fn main() {
  // tauri-build only watches the files it saw: the bundle may come after the first build.
  println!("cargo:rerun-if-changed=resources/mcp");
  tauri_build::build()
}
