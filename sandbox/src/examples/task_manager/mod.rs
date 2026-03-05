pub mod handlers;
pub mod models;
pub mod templates;

use axum::Router;
use axum::routing::{delete, get, patch};

pub fn router() -> Router {
    Router::new()
        .route(
            "/examples/tasks",
            get(handlers::list_tasks).post(handlers::create_task),
        )
        .route("/examples/tasks/:id/toggle", patch(handlers::toggle_task))
        .route(
            "/examples/tasks/:id/delete",
            delete(handlers::delete_task).post(handlers::delete_task),
        )
}
