// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Chrome starts this same exe as the "ByteHive for Chrome" native host.
    if bytehive_lib::browser::host::wanted() {
        bytehive_lib::browser::host::run();
    }
    bytehive_lib::run();
}
