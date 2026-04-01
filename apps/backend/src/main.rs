mod api;
mod auth;
mod middleware;
mod models;
mod repository;
mod service;

use axum::Router;
use repository::TodoRepository;
use service::TodoService;
use std::net::SocketAddr;

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
    let grpc_addr: SocketAddr = "127.0.0.1:50051".parse().expect("valid grpc address");

    println!("backend listening on http://127.0.0.1:4000");
    println!("backend gRPC listening on grpc://{grpc_addr}");

    tokio::spawn(async move {
        if let Err(err) = api::grpc::serve(grpc_addr).await {
            eprintln!("gRPC server failed: {err}");
        }
    });

    axum::serve(listener, app).await.expect("serve backend");
}
