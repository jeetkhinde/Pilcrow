use pilcrow_web::axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub fn router() -> Router {
    Router::new().route("/", get(check))
}
