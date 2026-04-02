// ./src/lib.rs

pub mod assets;
pub mod extract;
pub mod generated_routes;
pub(crate) mod headers;
mod macros;
pub mod response;
pub mod sse;
pub mod ws;

// ── Core API re-exports ──────────────────────────────────────
pub use extract::{RequestMode, SilcrowRequest};
pub use response::{ErrorResponse, ResponseExt, json, navigate, status};
pub use response::ToastLevel;
pub use sse::watch;
pub use sse::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute, interval, sse_raw, sse_stream,
};
pub use ws::{WsEvent, WsRoute, WsStream};
pub use axum::http::StatusCode;
pub use axum::response::Response;
pub use generated_routes::{
    GeneratedPageRoute, generated_routes, pilcrow_router, register_generated_routes,
};
pub use pilcrow_macros::sse;

// ── Available but not primary API ────────────────────────────
#[doc(hidden)]
pub use response::html;
#[doc(hidden)]
pub use axum;

// ── Internal helpers (used by ws.rs, macros, generated code) ─
pub(crate) use sse::serialize_or_null;
