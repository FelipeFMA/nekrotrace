use nekrotrace_core::{start_trace as core_start_trace, stop_trace as core_stop_trace, TraceEmitter, HopInfo, PingData};
use tauri::{Emitter, Window};
use std::sync::Arc;
use async_trait::async_trait;

struct TauriEmitter(Window);

#[async_trait]
impl TraceEmitter for TauriEmitter {
    async fn emit_hop_list(&self, payload: &Vec<HopInfo>) {
        let _ = self.0.emit("hop_list_updated", payload);
    }
    async fn emit_ping_data(&self, payload: &PingData) {
        let _ = self.0.emit("new_ping_data", payload);
    }
}

#[tauri::command]
pub async fn start_trace(host: String, window: Window) {
    let emitter = Arc::new(TauriEmitter(window));
    core_start_trace(host, emitter).await;
}

#[tauri::command]
pub async fn stop_trace() {
    core_stop_trace();
}
