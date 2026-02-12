#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::SocketAddr;
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

mod gemini;
// Note: Other modules are kept for desktop compatibility but ignored in API mode

#[tokio::main]
async fn main() {
    // Check if we are running in a server environment (like Railway)
    let is_server = std::env::var("PORT").is_ok() || std::env::var("RAILWAY_ENVIRONMENT").is_ok();

    if is_server {
        run_server().await;
    } else {
        run_tauri_app();
    }
}

async fn run_server() {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);

    let app = Router::new()
        .route("/", get(health_check))
        .route("/api/chat", post(chat_handler))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("🚀 OMAR AI Server starting on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "online",
        "agent": "OMAR AI",
        "solidarity": "Stand with Palestine"
    }))
}

async fn chat_handler(Json(payload): Json<Value>) -> Json<Value> {
    // Basic AI handler that can be expanded to use the gemini module
    println!("Received message: {:?}", payload);
    Json(json!({
        "response": "OMAR AI Backend is active. Message received.",
        "note": "Computer control features are limited in web mode."
    }))
}

fn run_tauri_app() {
    // Original Tauri startup logic for desktop
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
