//! Pilcrow web framework facade for SSR/UI apps.
//! This crate is the required entrypoint for convention-based `web` apps.

// ── Response builders ────────────────────────────────────────
pub use runtime::response::{
    ErrorResponse, JsonResponse, NavigateResponse, ResponseExt, ToastLevel,
};
pub use runtime::{json, navigate, status};

// ── Request handling ─────────────────────────────────────────
pub use runtime::{RequestMode, SilcrowRequest};

// ── Status & response primitives ─────────────────────────────
pub use runtime::Response;
pub use runtime::StatusCode;

// ── SSE ──────────────────────────────────────────────────────
pub use runtime::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute, interval, sse_raw, sse_stream,
    watch,
};

// ── WebSocket ────────────────────────────────────────────────
pub use runtime::{WsEvent, WsRoute, WsStream};

// ── Generated routes ─────────────────────────────────────────
pub use runtime::{
    GeneratedApiRoute, GeneratedPageRoute, generated_api_routes, generated_routes, pilcrow_router,
    register_generated_api_routes, register_generated_routes,
};

// ── Assets ───────────────────────────────────────────────────
pub use runtime::assets;

// ── Domain primitives (from pilcrow-core) ────────────────────
pub use pilcrow_core::{ApiEnvelope, AppError, AppResult, Meta};
