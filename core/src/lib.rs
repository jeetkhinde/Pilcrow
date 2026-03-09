// ./src/lib.rs

pub mod assets;
pub mod extract;
pub mod headers;
pub mod macros;
pub mod response;
pub mod sse;
pub mod ws;
// Re-export the core API so developers can just `use pilcrow::*`
pub use extract::{RequestMode, SilcrowRequest};
pub use response::{ErrorResponse, ResponseExt, html, json, navigate, status};
pub use sse::watch;
pub use sse::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute, interval, serialize_or_null,
    sse_raw, sse_stream,
};
pub use ws::{WsEvent, WsRoute, WsStream};
// Re-export Axum primitives they might need for convenience
pub use axum;
pub use axum::http::StatusCode;
pub use axum::response::Response;
pub use pilcrow_macros::sse;
