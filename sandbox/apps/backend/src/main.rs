mod api;
mod middleware;
mod models;
mod repository;
mod service;

use axum::Router;
use pilcrow_core::PilcrowConfig;
use repository::TodoRepository;
use service::TodoService;

#[derive(Clone)]
pub struct AppState {
    service: TodoService,
}

#[tokio::main]
async fn main() {
    let config = PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml");
    let bind_addr = config.backend_bind_addr();

    let repo = TodoRepository::new();
    let service = TodoService::new(repo);

    let state = AppState { service };
    let app = Router::new().merge(api::rest::router()).with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind backend server");

    println!("backend listening on http://{bind_addr}");
    axum::serve(listener, app).await.expect("serve backend");
}
