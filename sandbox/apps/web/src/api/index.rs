use pilcrow_web::axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct IndexContent {
    pub title: String,
    pub message: String,
}

pub fn fetch() -> IndexContent {
    // Fake "DB row" — in a real app this would query a backend/DB.
    IndexContent {
        title: "Pilcrow".to_string(),
        message: "Welcome to Pilcrow — an Astro-like web framework for Rust.".to_string(),
    }
}

async fn get_index() -> Json<IndexContent> {
    Json(fetch())
}

pub fn router() -> Router {
    Router::new().route("/", get(get_index))
}
