//! Pilcrow web framework facade for SSR/UI apps.
//! This crate is the required entrypoint for convention-based `web` apps.

// ── Response builders ────────────────────────────────────────
pub use pilcrow::{json, navigate, status};
pub use pilcrow::response::{
    ErrorResponse, JsonResponse, NavigateResponse, ResponseExt, ToastLevel,
};

// ── Request handling ─────────────────────────────────────────
pub use pilcrow::{RequestMode, SilcrowRequest};

// ── Status & response primitives ─────────────────────────────
pub use pilcrow::StatusCode;
pub use pilcrow::Response;

// ── SSE ──────────────────────────────────────────────────────
pub use pilcrow::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute,
    interval, sse_raw, sse_stream, watch,
};

// ── WebSocket ────────────────────────────────────────────────
pub use pilcrow::{WsEvent, WsRoute, WsStream};

// ── Generated routes ─────────────────────────────────────────
pub use pilcrow::{
    GeneratedPageRoute, generated_routes, pilcrow_router, register_generated_routes,
};

// ── Assets ───────────────────────────────────────────────────
pub use pilcrow::assets;

// ── Domain primitives (from pilcrow-core) ────────────────────
pub use pilcrow_core::{ApiEnvelope, AppError, AppResult, Meta};
