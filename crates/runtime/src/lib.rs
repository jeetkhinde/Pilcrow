// ./src/lib.rs

pub mod adapter;
pub mod adapters;
pub mod assets;
pub(crate) mod dev;
pub(crate) mod sw;
pub mod deferred;
pub mod context;
pub mod csrf;
pub mod generated_routes;
pub mod i18n;
pub mod isr;
pub mod middleware;
pub mod response;
pub mod sse;
pub mod start;
pub mod validator;
pub mod ws;
pub use adapter::{AdapterFuture, PilcrowAdapter, TokioAdapter};
pub use start::{export, start, start_with_adapter, start_with_prerender};
// ── Core API re-exports ──────────────────────────────────────
pub use axum::http::StatusCode;
pub use axum::response::Response;
pub use context::{FormMap, Locals, Req, Res};
pub use csrf::csrf_middleware;
pub use middleware::Next;
pub use generated_routes::{
    GeneratedApiRoute, GeneratedPageRoute, generated_api_routes, generated_routes, pilcrow_router,
    register_generated_api_routes, register_generated_routes,
};
pub use pilcrow_macros::sse;
pub use response::response::ToastLevel;
pub use response::response::{
    ActionResult, ErrorResponse, FormErrorItem, FormErrors, JsonResponse, NavigateResponse,
    ResponseExt, form_errors, json, navigate, redirect, status,
};
pub use sse::watch;
pub use sse::{
    EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter, SseRoute, interval, sse_raw, sse_stream,
};
pub use ws::ws::{WsEvent, WsRoute, WsStream};

// ── Available but not primary API ────────────────────────────
#[doc(hidden)]
pub use axum;
#[doc(hidden)]
pub use response::response::html;

pub use deferred::{
    Deferred, DeferredHtml, DeferredHtmlPatch, DeferredPatch,
    __deferred_html_patch_stream, __deferred_patch_stream, __serialize_deferred,
    __serialize_page_props, __streaming_props_response,
    deferred_response, deferred_response_combined,
};
// ── ISR ──────────────────────────────────────────────────────
pub use isr::{CacheEntrySnapshot, IsrCache, IsrCacheState, IsrHandle, __isr_cache_key};
// ── Validation ───────────────────────────────────────────────
pub use validator::Validator;
pub use context::ReqBuilder;
// ── i18n ─────────────────────────────────────────────────────
pub use i18n::{FmtHelper, I18nBundles};

// ── Internal helpers (used by ws.rs, macros, generated code) ─
pub(crate) use sse::serialize_or_null;
#[doc(hidden)]
pub use tokio;
