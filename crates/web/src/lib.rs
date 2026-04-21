//! Pilcrow web framework facade for SSR/UI apps.
//! This crate is the required entrypoint for convention-based `web` apps.

// ── Response builders ────────────────────────────────────────
pub use runtime::response::response::{
    ActionResult, ErrorResponse, FormErrorItem, FormErrors, JsonResponse, NavigateResponse,
    ResponseExt, ToastLevel,
};
pub use runtime::response::response::{form_errors, json, navigate, redirect, status};

// ── Request handling ─────────────────────────────────────────
pub use runtime::{FormMap, Locals, Next, Req, Res};

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
pub use pilcrow_core::{
    ApiEnvelope, AppError, AppResult, BackendConfig, Meta, PilcrowConfig, WebConfig,
};

pub use pilcrow_client::PilcrowClient;
pub use pilcrow_macros::handler;
pub use runtime::start;

// ── Deferred streaming ───────────────────────────────────────
pub use runtime::{Deferred, DeferredHtml, DeferredHtmlPatch, DeferredPatch, deferred_response, deferred_response_combined};

// ── Doc-hidden re-exports for generated code ────────────────
#[doc(hidden)]
pub use axum;
#[doc(hidden)]
pub use tracing;
#[doc(hidden)]
pub use pilcrow_client;
#[doc(hidden)]
pub use runtime::csrf_middleware as __csrf_middleware;
#[doc(hidden)]
pub use runtime::{__deferred_html_patch_stream, __deferred_patch_stream, __serialize_deferred};

/// Include the auto-generated Pilcrow app module and expose `pilcrow_router()`.
///
/// This macro eliminates all manual route wiring. Place it at the top of your
/// `main.rs` and use `pilcrow_router()` to get a fully-wired `axum::Router`:
///
/// ```ignore
/// pilcrow_web::pilcrow_app!();
///
/// #[tokio::main]
/// async fn main() {
///     let app = pilcrow_router();
///     pilcrow_web::start(app).await
/// }
/// ```
#[macro_export]
macro_rules! pilcrow_app {
    () => {
        // API mod tree at crate root so `mod api { pub mod health; }` resolves to src/api/health.rs
        include!(concat!(env!("OUT_DIR"), "/generated_api_mods.rs"));

        mod __pilcrow_app {
            include!(concat!(env!("OUT_DIR"), "/generated_app.rs"));
        }

        // Typed route helpers: `routes::products_id("42")` → `"/products/42"`
        include!(concat!(env!("OUT_DIR"), "/generated_typed_routes.rs"));

        // Typed env structs: `env::Private::load()?` → `env::Private { database_url: .. }`
        include!(concat!(env!("OUT_DIR"), "/generated_env.rs"));

        fn pilcrow_router() -> ::pilcrow_web::axum::Router {
            __pilcrow_app::build_router()
        }
    };
}
