#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod network_engine;

use network_engine::start_trace;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_trace])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
