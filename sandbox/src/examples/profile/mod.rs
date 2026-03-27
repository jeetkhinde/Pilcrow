pub mod handlers;
pub mod models;
pub mod templates;

use axum::Router;
use axum::routing::get;

pub fn router() -> Router {
    Router::new().route(
        "/examples/profile",
        get(handlers::view_profile).post(handlers::update_profile),
    )
}
