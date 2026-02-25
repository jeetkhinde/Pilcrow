// ./crates/pilcrow/src/lib.rs

pub mod assets;
pub mod extract;
pub mod macros;
pub mod response;
pub mod select;
pub mod sse;

// Re-export the core API so developers can just `use pilcrow::*`
pub use extract::SilcrowRequest;
pub use response::{html, json, navigate, ResponseExt};
pub use select::Responses;
pub use sse::{sse, SilcrowEvent, SseRoute};
// Re-export Axum primitives they might need for convenience
pub use axum;
pub use axum::http::StatusCode;
pub use axum::response::Response;
