//! Pilcrow web framework facade for SSR/UI apps.
//! This crate is the required entrypoint for convention-based `web` apps.

// ── Response builders ────────────────────────────────────────
pub use runtime::response::response::{form_errors, json, navigate, redirect, status};
pub use runtime::response::response::{
    ActionResult, ErrorResponse, FormErrorItem, FormErrors, JsonResponse, NavigateResponse,
    ResponseExt, ToastLevel,
};

// ── Request handling ─────────────────────────────────────────
pub use runtime::{FormMap, Locals, Next, Page, Req, Res};

// ── Status & response primitives ─────────────────────────────
pub use runtime::Response;
pub use runtime::StatusCode;

// ── SSE ──────────────────────────────────────────────────────
pub use runtime::{
    interval, sse_raw, sse_stream, watch, EmitError, PilcrowStreamExt, SilcrowEvent, SseEmitter,
    SseRoute,
};

// ── WebSocket ────────────────────────────────────────────────
pub use runtime::{WsEvent, WsRoute, WsStream};

// ── Generated routes ─────────────────────────────────────────
pub use runtime::{
    generated_api_routes, generated_routes, pilcrow_router, register_generated_api_routes,
    register_generated_routes, GeneratedApiRoute, GeneratedPageRoute,
};

// ── Assets ───────────────────────────────────────────────────
pub use runtime::assets;

// ── Domain primitives (from pilcrow-core) ────────────────────
pub use pilcrow_core::{
    ApiEnvelope, AppError, AppResult, BackendConfig, HookError, Meta, PilcrowConfig, WebConfig,
};

pub use pilcrow_client::PilcrowClient;
pub use pilcrow_macros::handler;
pub use runtime::island_ssr::IslandSsrWorker;
pub use runtime::{export, start, start_with_adapter, start_with_prerender};
pub use runtime::{AdapterFuture, PilcrowAdapter, TokioAdapter};

/// Platform deployment adapters.
///
/// Each adapter implements [`PilcrowAdapter`] and is passed to
/// [`start_with_adapter`] to target a specific hosting platform.
///
/// # Available adapters
///
/// | Adapter | Platform | Feature flag |
/// |---|---|---|
/// | [`TokioAdapter`] | Local / VPS / bare metal | *(default)* |
/// | [`adapters::PortEnvAdapter`] | Any `PORT`-env platform | *(default)* |
/// | [`adapters::FlyAdapter`] | Fly.io | *(default)* |
/// | [`adapters::RailwayAdapter`] | Railway | *(default)* |
/// | [`adapters::CloudRunAdapter`] | Google Cloud Run | *(default)* |
/// | [`adapters::RenderAdapter`] | Render | *(default)* |
/// | [`adapters::VercelAdapter`] | Vercel (long-running) | *(default)* |
/// | [`adapters::LambdaAdapter`] | AWS Lambda / Vercel Functions / Netlify | `lambda` |
pub mod adapters {
    pub use runtime::adapters::{
        CloudRunAdapter, FlyAdapter, PortEnvAdapter, RailwayAdapter, RenderAdapter, VercelAdapter,
    };

    #[cfg(feature = "lambda")]
    pub use runtime::adapters::LambdaAdapter;
}

// ── Deferred streaming ───────────────────────────────────────
pub use runtime::{
    deferred_response, deferred_response_combined, Deferred, DeferredHtml, DeferredHtmlPatch,
    DeferredPatch,
};

// ── ISR (Incremental Static Regeneration) ────────────────────
pub use runtime::{IsrCache, IsrCacheState, IsrHandle};
// ── i18n ─────────────────────────────────────────────────────
pub use runtime::{FmtHelper, I18nBundles};

// ── Doc-hidden re-exports for generated code ────────────────
#[doc(hidden)]
pub use axum;
#[doc(hidden)]
pub use pilcrow_client;
#[doc(hidden)]
pub use runtime::__isr_cache_key;
#[doc(hidden)]
pub use runtime::csrf_middleware as __csrf_middleware;
#[doc(hidden)]
pub use runtime::tokio;
#[doc(hidden)]
pub use runtime::{__deferred_html_patch_stream, __deferred_patch_stream, __serialize_deferred};
#[doc(hidden)]
pub use tracing;

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

        // Typed i18n helpers: `t::greeting(&req, &name)` → `String`
        include!(concat!(env!("OUT_DIR"), "/generated_i18n.rs"));

        fn pilcrow_router() -> ::pilcrow_web::axum::Router {
            __pilcrow_app::build_router()
        }

        /// Start the Pilcrow server, pre-rendering any SSG pages before accepting connections.
        ///
        /// Use `pilcrow_start(pilcrow_router()).await` in place of
        /// `pilcrow_web::start(pilcrow_router()).await` when your app has pages with
        /// `pub const PRERENDER: bool = true`.
        async fn pilcrow_start(router: ::pilcrow_web::axum::Router) {
            // Wire up the React SSR Node worker when [client.react] ssr = true.
            let config =
                ::pilcrow_web::PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml");
            let router = {
                let bundles = __pilcrow_app::__pilcrow_ssr_bundles();
                if config.client.react.ssr && !bundles.is_empty() {
                    match ::pilcrow_web::IslandSsrWorker::spawn_with_sources(
                        bundles,
                        &config.client.react.node_bin,
                    ) {
                        Ok(worker) => router.layer(::pilcrow_web::axum::Extension(
                            ::std::sync::Arc::new(::std::sync::Mutex::new(worker)),
                        )),
                        Err(e) => {
                            eprintln!("[pilcrow] failed to spawn React SSR worker: {e}");
                            router
                        }
                    }
                } else {
                    router
                }
            };
            __pilcrow_app::__pilcrow_init().await;
            ::pilcrow_web::start_with_prerender(router, |cache| async move {
                __pilcrow_app::__pilcrow_prerender_all(&cache).await
            })
            .await;
        }

        /// Export all pre-rendered pages as static HTML files to `dir`.
        ///
        /// Run with `cargo run -- export <dir>` (the scaffold `main.rs` handles this arg).
        /// Each page with `pub const PRERENDER: bool = true` is written to `<dir><key>/index.html`.
        async fn pilcrow_export(dir: &str) {
            ::pilcrow_web::export(dir, |cache| async move {
                __pilcrow_app::__pilcrow_prerender_all(&cache).await
            })
            .await;
        }
    };
}
