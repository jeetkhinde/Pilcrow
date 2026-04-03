mod api;
mod middleware;
mod models;
mod repository;
mod service;

use axum::Router;
use repository::TodoRepository;
use service::TodoService;

#[derive(Clone)]
pub struct AppState {
    service: TodoService,
}

#[tokio::main]
async fn main() {
    let repo = TodoRepository::new();
    let service = TodoService::new(repo);

    let state = AppState { service };
    let app = Router::new().merge(api::rest::router()).with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .expect("bind backend server");

    println!("backend listening on http://127.0.0.1:4000");
    axum::serve(listener, app).await.expect("serve backend");
}
