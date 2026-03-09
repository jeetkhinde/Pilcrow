mod examples;
mod templates;
use crate::examples::profile::models::User;
use crate::examples::sse::task_manager::model::AppState as SseAppState;
use crate::examples::standard::task_manager::model::*;
use axum::{Extension, Router, routing::get};

#[tokio::main]
async fn main() {
    let mock_user = User {
        id: 1,
        role: "admin".into(),
    };

    let app_state = AppState::new();
    let sse_state = SseAppState::new();

    let app = Router::new()
        .route(
            &pilcrow::assets::silcrow_js_path(),
            get(pilcrow::assets::serve_silcrow_js),
        )
        // Profile Example Routes
        .merge(examples::profile::router())
        // Standard Task Manager Example Routes
        .merge(examples::standard::task_manager::router().layer(Extension(app_state)))
        // SSE Task Manager Example Routes
        .merge(examples::sse::task_manager::router().layer(Extension(sse_state)))
        // Global Extension Configurations
        .layer(Extension(mock_user));

    println!("Listening on http://127.0.0.1:3000/examples/profile");
    println!("Tasks Dashboard on http://127.0.0.1:3000/examples/tasks");
    println!("SSE Tasks Dashboard on http://127.0.0.1:3000/examples/sse/tasks");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
