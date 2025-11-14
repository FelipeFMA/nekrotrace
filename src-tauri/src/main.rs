#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod network_engine;

use network_engine::start_trace;

fn main() {
    // Force X11 on Linux to match development environment
    // This mirrors the env vars used in the `tauri:dev:x11` script.
    // If the user already set these variables, we respect their values.
    #[cfg(target_os = "linux")]
    {
        use std::env;
        if env::var_os("GDK_BACKEND").is_none() {
            env::set_var("GDK_BACKEND", "x11");
        }
        if env::var_os("WINIT_UNIX_BACKEND").is_none() {
            env::set_var("WINIT_UNIX_BACKEND", "x11");
        }
        if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_trace])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
