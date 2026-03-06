pub mod sse;
pub mod standard;
pub mod templates;

use axum::Router;
use axum::routing::{delete, get, patch};

pub fn router() -> Router {
    Router::new()
        .route(
            "/examples/tasks",
            get(standard::handlers::list_tasks).post(standard::handlers::create_task),
        )
        .route(
            "/examples/tasks/:id/toggle",
            patch(standard::handlers::toggle_task),
        )
        .route(
            "/examples/tasks/:id/delete",
            delete(standard::handlers::delete_task).post(standard::handlers::delete_task),
        )
        // SSE version events
        .route(
            "/examples/tasks-sse/events",
            get(sse::handlers::task_events),
        )
}
