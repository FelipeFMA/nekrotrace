use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use nekrotrace_core::{start_trace, stop_trace, TraceEmitter, HopInfo, PingData};
use serde::Deserialize;
use std::{sync::Arc, net::SocketAddr};
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};
use async_trait::async_trait;

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
}

struct ServerEmitter {
    tx: broadcast::Sender<String>,
}

#[async_trait]
impl TraceEmitter for ServerEmitter {
    async fn emit_hop_list(&self, payload: &Vec<HopInfo>) {
        let msg = serde_json::json!({
            "event": "hop_list_updated",
            "payload": payload
        }).to_string();
        let _ = self.tx.send(msg);
    }
    async fn emit_ping_data(&self, payload: &PingData) {
        let msg = serde_json::json!({
            "event": "new_ping_data",
            "payload": payload
        }).to_string();
        let _ = self.tx.send(msg);
    }
}

#[tokio::main]
async fn main() {
    let (tx, _rx) = broadcast::channel(100);
    let state = AppState { tx };

    // Serve static files from "build" directory (relative to CWD)
    let app = Router::new()
        .route("/api/start", post(start_handler))
        .route("/api/stop", post(stop_handler))
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new("build").fallback(ServeFile::new("build/index.html")))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct StartPayload {
    host: String,
}

async fn start_handler(State(state): State<AppState>, Json(payload): Json<StartPayload>) {
    let emitter = Arc::new(ServerEmitter { tx: state.tx.clone() });
    tokio::spawn(async move {
        start_trace(payload.host, emitter).await;
    });
}

async fn stop_handler() {
    stop_trace();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}
