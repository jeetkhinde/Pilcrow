use pilcrow_web::axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AboutContent {
    pub title: String,
    pub description: String,
}

pub fn fetch() -> AboutContent {
    // Fake "DB row" — in a real app this would query a backend/DB.
    AboutContent {
        title: "About".to_string(),
        description: "Pilcrow compiles .html templates with Rust frontmatter into type-safe Askama render functions at build time.".to_string(),
    }
}

async fn get_about() -> Json<AboutContent> {
    Json(fetch())
}

pub fn router() -> Router {
    Router::new().route("/", get(get_about))
}
