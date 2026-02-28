// ./src/lib.rs

/// Macro to define a typed route constant for SSE/WS endpoints.
/// Generates a newtype struct with `new`, `path`, `Deref`, and `AsRef<str>`.
#[macro_export]
macro_rules! define_route {
    ($name:ident, $protocol:expr, $example_path:expr, $example_const:expr) => {
        #[doc = concat!("A compile-time ", $protocol, " route path. Use as both a route string and header value.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name(&'static str);

        impl $name {
            pub const fn new(path: &'static str) -> Self {
                Self(path)
            }

            pub const fn path(&self) -> &'static str {
                self.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.0
            }
        }
    };
}

/// Serialize data to a JSON `Value`, falling back to `Null` with a warning on error.
pub fn serialize_or_null(data: impl serde::Serialize, context: &str) -> serde_json::Value {
    serde_json::to_value(data).unwrap_or_else(|e| {
        tracing::warn!("{context} serialization failed: {e}");
        serde_json::Value::Null
    })
}

pub mod assets;
pub mod extract;
pub mod headers;
pub mod macros;
pub mod response;
pub mod sse;
pub mod ws;

// Re-export the core API so developers can just `use pilcrow::*`
pub use extract::{RequestMode, SilcrowRequest};
pub use response::{html, json, navigate, ResponseExt};
pub use sse::{sse, SilcrowEvent, SseRoute};
pub use ws::{WsEvent, WsRoute, WsStream};
// Re-export Axum primitives they might need for convenience
pub use axum;
pub use axum::http::StatusCode;
pub use axum::response::Response;
