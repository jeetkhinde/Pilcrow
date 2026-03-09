pub mod handlers;
pub mod model;
pub mod templates;

use axum::Router;
use axum::routing::{delete, get, patch};
use pilcrow::SseRoute;

/// Compile-time SSE route constant.
///
/// Used in three places — all referencing the same value:
///   1. Route registration: `.route(EVENTS.path(), get(handlers::events))`
///   2. Response modifier: `.sse(EVENTS)` sets the `silcrow-sse` header
///   3. Template:          `s-live=(events_path)` on DOM elements
///
/// One constant, zero drift.
pub const EVENTS: SseRoute = SseRoute::new("/examples/sse/tasks/events");

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
        .route(EVENTS.path(), get(handlers::events))
}
