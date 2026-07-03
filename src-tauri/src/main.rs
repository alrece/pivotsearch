// Prevent a console window from popping up on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pivotsearch_tauri_lib::run()
}
