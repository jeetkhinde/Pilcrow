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

    let app = Router::new()
        .route(
            &pilcrow::assets::silcrow_js_path(),
            get(pilcrow::assets::serve_silcrow_js),
        )
        .route(
            "/profile",
            get(handlers::view_profile).post(handlers::update_profile),
        )
        .layer(axum::Extension(mock_user));

    println!("Listening on http://127.0.0.1:3000/profile");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
