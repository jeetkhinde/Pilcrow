pub mod handlers;
pub mod models;
pub mod templates;

use axum::routing::get;
use axum::Router;

pub fn router() -> Router {
    Router::new()
        .route(
            "/examples/tasks",
            get(handlers::list_tasks).post(handlers::create_task),
        )
        .route(
            "/examples/tasks/:id/toggle",
            axum::routing::post(handlers::toggle_task),
        )
        .route(
            "/examples/tasks/:id/delete",
            axum::routing::delete(handlers::delete_task).post(handlers::delete_task),
        )
}
