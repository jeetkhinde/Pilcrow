pub mod handlers;
pub mod models;
pub mod templates;

use axum::routing::get;
use axum::Router;

pub fn router() -> Router {
    Router::new().route(
        "/examples/profile",
        get(handlers::view_profile).post(handlers::update_profile),
    )
}
