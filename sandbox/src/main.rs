mod examples;
mod templates;
use crate::examples::profile::models::User;
use crate::examples::task_manager::models::*;
use axum::{routing::get, Extension, Router};

#[tokio::main]
async fn main() {
    let mock_user = User {
        id: 1,
        role: "admin".into(),
    };

    let app_state = AppState::new();

    let app = Router::new()
        .route(
            &pilcrow::assets::silcrow_js_path(),
            get(pilcrow::assets::serve_silcrow_js),
        )
        // Profile Example Routes
        .merge(examples::profile::router())
        // Task Manager Example Routes
        .merge(examples::task_manager::router())
        // Global Extension Configurations
        .layer(Extension(mock_user))
        .layer(Extension(app_state));

    println!("Listening on http://127.0.0.1:3000/examples/profile");
    println!("Tasks Dashboard on http://127.0.0.1:3000/examples/tasks");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
