mod handlers;
mod models;
mod templates;

use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let mock_user = models::User {
        id: 1,
        role: "admin".into(),
    };

    let app_state = models::AppState::new();

    let app = Router::new()
        .route(
            &pilcrow::assets::silcrow_js_path(),
            get(pilcrow::assets::serve_silcrow_js),
        )
        .route(
            "/profile",
            get(handlers::view_profile).post(handlers::update_profile),
        )
        // Task Manager Routes
        .route(
            "/tasks",
            get(handlers::list_tasks).post(handlers::create_task),
        )
        .route(
            "/tasks/:id/toggle",
            axum::routing::post(handlers::toggle_task),
        )
        .route(
            "/tasks/:id/delete",
            axum::routing::delete(handlers::delete_task).post(handlers::delete_task),
        )
        .layer(axum::Extension(mock_user))
        .layer(axum::Extension(app_state));

    println!("Listening on http://127.0.0.1:3000/profile");
    println!("Tasks Dashboard on http://127.0.0.1:3000/tasks");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
