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

/// Experimental APIs that may change before stabilization.
#[cfg(feature = "experimental-baked-pages")]
pub mod experimental {
    /// Developer-facing helpers for opt-in baked page serving.
    pub mod baked_pages {
        pub use runtime::baked_pages::*;

        use axum::{
            http::header::{HeaderName, HeaderValue},
            response::{Html, IntoResponse, Response},
        };
        use std::io;

        const BAKED_HEADER: HeaderName = HeaderName::from_static("x-pilcrow-baked");
        const SSR_LOAD_HEADER: HeaderName = HeaderName::from_static("x-pilcrow-ssr-load");

        /// Tiny route-local wrapper for opting a handler into baked serving.
        ///
        /// The route still owns its source-of-truth render/load function. This helper
        /// only checks the baked store first, writes lazy artifacts on misses, and
        /// returns a normal Axum response.
        #[derive(Debug, Clone)]
        pub struct BakedRoute {
            store: BakedPageStore,
            declaration: BakedRouteDeclaration,
        }

        impl BakedRoute {
            pub fn new(store: BakedPageStore, declaration: BakedRouteDeclaration) -> Self {
                Self { store, declaration }
            }

            pub fn store(&self) -> &BakedPageStore {
                &self.store
            }

            pub fn declaration(&self) -> &BakedRouteDeclaration {
                &self.declaration
            }

            pub fn serve<F>(&self, render: F) -> io::Result<Response>
            where
                F: FnOnce(&BakedRouteDeclaration) -> io::Result<BakedRenderedPage>,
            {
                serve_baked_or_render(&self.store, &self.declaration, render)
            }
        }

        pub fn serve_baked_or_render<F>(
            store: &BakedPageStore,
            declaration: &BakedRouteDeclaration,
            render: F,
        ) -> io::Result<Response>
        where
            F: FnOnce(&BakedRouteDeclaration) -> io::Result<BakedRenderedPage>,
        {
            let outcome = store.get_or_render_declared(declaration, render)?;
            Ok(baked_html_response(outcome))
        }

        pub fn baked_html_response(outcome: BakedServeOutcome) -> Response {
            let baked_state = outcome.state.as_str();
            let render_state = outcome.state.render_state();
            let mut response = Html(outcome.html).into_response();
            response
                .headers_mut()
                .insert(BAKED_HEADER, HeaderValue::from_static(baked_state));
            response
                .headers_mut()
                .insert(SSR_LOAD_HEADER, HeaderValue::from_static(render_state));
            response
        }
    }
}

#[cfg(all(test, feature = "experimental-baked-pages"))]
mod baked_page_tests {
    use super::experimental::baked_pages::{
        BakedPageStore, BakedRenderedPage, BakedRoute, BakedRouteDeclaration, DependencyKey,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use http_body_util::BodyExt;
    use std::{
        fs, io,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn lazy_route_opt_in_writes_then_hits_without_rendering_again() {
        let temp = tempfile::tempdir().unwrap();
        let route = BakedRoute::new(
            BakedPageStore::new(temp.path()),
            BakedRouteDeclaration::lazy_on_first_hit("/lazy", "/lazy")
                .full_page()
                .text_slot("status", vec![DependencyKey::new("lazy")]),
        );
        let render_count = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/lazy",
            get({
                let route = route.clone();
                let render_count = render_count.clone();
                move || {
                    let route = route.clone();
                    let render_count = render_count.clone();
                    async move {
                        route
                            .serve(|_| {
                                render_count.fetch_add(1, Ordering::SeqCst);
                                Ok(BakedRenderedPage::new(
                                    "<main>fresh lazy</main>",
                                    "render-v1",
                                ))
                            })
                            .unwrap()
                    }
                }
            }),
        );

        let first = app
            .clone()
            .oneshot(Request::builder().uri("/lazy").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()["x-pilcrow-baked"], "miss-rendered");
        assert_eq!(first.headers()["x-pilcrow-ssr-load"], "ran");
        assert_eq!(response_text(first).await, "<main>fresh lazy</main>");
        assert_eq!(render_count.load(Ordering::SeqCst), 1);

        let second = app
            .oneshot(Request::builder().uri("/lazy").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(second.headers()["x-pilcrow-baked"], "hit");
        assert_eq!(second.headers()["x-pilcrow-ssr-load"], "skipped");
        assert_eq!(response_text(second).await, "<main>fresh lazy</main>");
        assert_eq!(render_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fragment_composed_route_opt_in_composes_and_hits_without_rendering_again() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let layout_path = store.layout_path("app");
        fs::create_dir_all(layout_path.parent().unwrap()).unwrap();
        fs::write(
            layout_path,
            "<html><body><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>",
        )
        .unwrap();
        let route = BakedRoute::new(
            store,
            BakedRouteDeclaration::lazy_on_first_hit("/composed", "/composed")
                .fragment_composed("app")
                .text_slot("status", vec![DependencyKey::new("composed")]),
        );
        let render_count = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/composed",
            get({
                let route = route.clone();
                let render_count = render_count.clone();
                move || {
                    let route = route.clone();
                    let render_count = render_count.clone();
                    async move {
                        route
                            .serve(|_| {
                                render_count.fetch_add(1, Ordering::SeqCst);
                                Ok(BakedRenderedPage::new(
                                    "<main>fresh body</main>",
                                    "render-v1",
                                ))
                            })
                            .unwrap()
                    }
                }
            }),
        );

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/composed")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.headers()["x-pilcrow-baked"], "miss-rendered");
        assert_eq!(
            response_text(first).await,
            "<html><body><main>fresh body</main></body></html>"
        );
        assert_eq!(render_count.load(Ordering::SeqCst), 1);

        let second = app
            .oneshot(
                Request::builder()
                    .uri("/composed")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.headers()["x-pilcrow-baked"], "hit");
        assert_eq!(second.headers()["x-pilcrow-ssr-load"], "skipped");
        assert_eq!(
            response_text(second).await,
            "<html><body><main>fresh body</main></body></html>"
        );
        assert_eq!(render_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn never_bake_route_opt_in_renders_without_writing_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let route = BakedRoute::new(
            BakedPageStore::new(temp.path()),
            BakedRouteDeclaration::never_bake("/live", "/live"),
        );
        let render_count = AtomicUsize::new(0);

        let first = route
            .serve(|_| {
                render_count.fetch_add(1, Ordering::SeqCst);
                Ok(BakedRenderedPage::new("<main>live</main>", "render-v1"))
            })
            .unwrap();
        assert_eq!(first.headers()["x-pilcrow-baked"], "never-bake-rendered");
        assert_eq!(first.headers()["x-pilcrow-ssr-load"], "ran");

        let second = route
            .serve(|_| {
                render_count.fetch_add(1, Ordering::SeqCst);
                Ok(BakedRenderedPage::new("<main>live</main>", "render-v1"))
            })
            .unwrap();
        assert_eq!(second.headers()["x-pilcrow-baked"], "never-bake-rendered");
        assert_eq!(second.headers()["x-pilcrow-ssr-load"], "ran");
        assert_eq!(render_count.load(Ordering::SeqCst), 2);
        assert!(route.store().serve_if_fresh("/live").unwrap().is_none());
    }

    #[test]
    fn build_time_opt_in_requires_existing_prebake() {
        let temp = tempfile::tempdir().unwrap();
        let route = BakedRoute::new(
            BakedPageStore::new(temp.path()),
            BakedRouteDeclaration::build_time("/built", "/built").full_page(),
        );
        let render_count = AtomicUsize::new(0);

        let error = route
            .serve(|_| {
                render_count.fetch_add(1, Ordering::SeqCst);
                Ok(BakedRenderedPage::new("<main>built</main>", "render-v1"))
            })
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(render_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn prebaked_build_time_route_serves_hit_and_skips_render() {
        let temp = tempfile::tempdir().unwrap();
        let store = BakedPageStore::new(temp.path());
        let declaration = BakedRouteDeclaration::build_time("/built", "/built")
            .full_page()
            .text_slot("status", vec![DependencyKey::new("built")]);
        store
            .prebake_declared(&declaration, |_| {
                Ok(BakedRenderedPage::new("<main>prebaked</main>", "render-v1"))
            })
            .unwrap();
        let route = BakedRoute::new(store, declaration);
        let render_count = AtomicUsize::new(0);

        let response = route
            .serve(|_| {
                render_count.fetch_add(1, Ordering::SeqCst);
                Ok(BakedRenderedPage::new("<main>miss</main>", "render-v2"))
            })
            .unwrap();

        assert_eq!(response.headers()["x-pilcrow-baked"], "hit");
        assert_eq!(response.headers()["x-pilcrow-ssr-load"], "skipped");
        assert_eq!(response_text(response).await, "<main>prebaked</main>");
        assert_eq!(render_count.load(Ordering::SeqCst), 0);
    }

    async fn response_text(response: axum::response::Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}

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

// ── Async streaming ──────────────────────────────────────────
pub use runtime::{
    async_response_combined, async_value_response, AsyncHtml, AsyncHtmlPatch, AsyncValue,
    AsyncValuePatch, LiveProp, LiveTarget, __live_props_response,
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
pub use runtime::{__async_html_patch_stream, __async_value_patch_stream, __serialize_async_value};
#[doc(hidden)]
pub use runtime::{__serialize_page_props, __streaming_props_response};
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
