pub mod handlers;
pub mod model;
pub mod templates;

use axum::Router;
use axum::routing::{delete, get, patch};

pub fn router() -> Router {
    Router::new()
        .route(
            "/examples/sse/tasks",
            get(handlers::list_tasks).post(handlers::create_task),
        )
        .route(
            "/examples/sse/tasks/:id/toggle",
            patch(handlers::toggle_task),
        )
        .route(
            "/examples/sse/tasks/:id/delete",
            delete(handlers::delete_task),
        )
        // SSE version events
        .route("/examples/sse/tasks/events", get(handlers::task_events))
        .route(
            "/examples/sse/tasks/list-events",
            get(handlers::task_list_events),
        )
}
