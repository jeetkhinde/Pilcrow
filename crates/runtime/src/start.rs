use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::BoxError;
use axum::Router;
use pilcrow_core::config::config::CacheProvider;
use pilcrow_core::PilcrowConfig;
use tower::timeout::TimeoutLayer;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::adapter::{PilcrowAdapter, TokioAdapter};
use crate::dev::{dev_inject_layer, dev_reload_handler, spawn_css_watcher, DevState};
use crate::i18n::{locale_middleware_impl, I18nBundles};
use crate::image::handler::{image_handler, ImageState};
use crate::isr::{IsrCache, IsrHandle};
use crate::sw::{sw_handler, sw_inject_layer};

const REQUEST_TIMEOUT_SECS: u64 = 30;

pub async fn start(app: Router) {
    start_with_prerender(app, |_cache| async {}).await;
}

/// Start the Pilcrow web server, calling `prerender_fn` with the shared `IsrCache`
/// before accepting connections. Used by SSG apps to pre-warm the cache at startup.
pub async fn start_with_prerender<F, Fut>(app: Router, prerender_fn: F)
where
    F: FnOnce(Arc<IsrCache>) -> Fut,
    Fut: Future<Output = ()>,
{
    start_with_adapter(app, prerender_fn, TokioAdapter).await;
}

/// Start the Pilcrow web server with a custom deployment [`PilcrowAdapter`].
///
/// The adapter receives the fully-wired `Router` and the bind address from
/// `Pilcrow.toml`. Use this when targeting a non-standard runtime (Lambda, etc.).
///
/// ```rust,ignore
/// pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, MyAdapter).await;
/// ```
pub async fn start_with_adapter<F, Fut, A>(app: Router, prerender_fn: F, adapter: A)
where
    F: FnOnce(Arc<IsrCache>) -> Fut,
    Fut: Future<Output = ()>,
    A: PilcrowAdapter,
{
    let config = Arc::new(PilcrowConfig::load_from_current_dir().expect("load Pilcrow.toml"));
    let bind_addr = config.web_bind_addr();
    let http = reqwest::Client::new();

    // Load i18n bundles when [i18n] is configured in Pilcrow.toml.
    let i18n_bundles: Option<I18nBundles> = if !config.i18n.locales.is_empty() {
        let locales_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("src")
            .join(&config.i18n.locales_dir);
        let bundles = I18nBundles::load(
            &locales_dir,
            &config.i18n.locales,
            &config.i18n.default_locale,
        );
        tracing::info!(
            "i18n: {} locale(s) loaded (default: {})",
            config.i18n.locales.len(),
            config.i18n.default_locale
        );
        Some(bundles)
    } else {
        None
    };

    let isr_cache = Arc::new(match &config.cache.provider {
        CacheProvider::Filesystem => {
            let dir = config.cache.dir.as_deref().unwrap_or(".pilcrow-cache");
            tracing::info!("ISR cache: filesystem backend at {dir}");
            IsrCache::with_persistence(dir)
        }
        _ => IsrCache::new(),
    });

    let dev_mode = std::env::var("PILCROW_DEV").is_ok();
    let sw_enabled = config.service_worker.enabled && !dev_mode;
    let sw_strategy_dbg = format!("{:?}", config.service_worker.strategy);

    prerender_fn(Arc::clone(&isr_cache)).await;
    let isr_handle = IsrHandle::new(Arc::clone(&isr_cache));

    let mut app = app.route("/__pilcrow/isr", axum::routing::get(isr_inspect_handler));

    if sw_enabled {
        app = app.route("/sw.js", axum::routing::get(sw_handler));
    }

    if config.images.enabled {
        let image_state = ImageState {
            config: Arc::new(config.images.clone()),
            semaphore: Arc::new(tokio::sync::Semaphore::new(config.images.concurrency)),
        };
        app = app.route(
            "/_image",
            axum::routing::get(image_handler).with_state(image_state),
        );
        tracing::info!(
            "image optimization: enabled (cache: {})",
            config.images.cache_dir
        );
    }

    let dev_state = if dev_mode {
        let state = DevState::new();
        let src_dir = std::env::current_dir().unwrap_or_default().join("src");
        spawn_css_watcher(state.sender(), src_dir);
        app = app.route(
            "/__pilcrow/dev-reload",
            axum::routing::get(dev_reload_handler),
        );
        Some(state)
    } else {
        None
    };

    let mut app = app
        .layer(axum::Extension(config))
        .layer(axum::Extension(http))
        .layer(axum::Extension(isr_handle))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    if err.is::<tower::timeout::error::Elapsed>() {
                        StatusCode::REQUEST_TIMEOUT
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                }))
                .layer(TimeoutLayer::new(Duration::from_secs(REQUEST_TIMEOUT_SECS))),
        )
        .layer(TraceLayer::new_for_http());

    // i18n: inject bundles as extension (for handler access via Req.i18n) and add the
    // locale-prefix rewrite middleware (outermost, so it runs before routing).
    if let Some(bundles) = i18n_bundles {
        let b = bundles.clone();
        app = app
            .layer(axum::Extension(bundles))
            .layer(axum::middleware::from_fn(move |req, next| {
                locale_middleware_impl(req, next, b.clone())
            }));
    }

    let app = app;

    let app = if let Some(state) = dev_state {
        tracing::info!("dev mode: live reload + CSS hot swap active");
        app.layer(axum::Extension(state))
            .layer(axum::middleware::from_fn(dev_inject_layer))
    } else {
        app
    };

    let app = if sw_enabled {
        tracing::info!("service worker: enabled (strategy: {sw_strategy_dbg})");
        app.layer(axum::middleware::from_fn(sw_inject_layer))
    } else {
        app
    };

    adapter.serve(&bind_addr, app).await;
}

/// Export all pre-rendered pages as static HTML files to `dir`.
///
/// Runs `prerender_fn` to fill the ISR cache (same as `start_with_prerender`),
/// then writes each cached entry to `<dir><key>/index.html`.
///
/// Call this from a generated `pilcrow_export(dir)` function in the `pilcrow_app!()`
/// macro, or invoke it directly for custom export flows.
///
/// Dynamic routes must declare `pub async fn entries()` for pages with `PRERENDER = true`.
pub async fn export<F, Fut>(dir: &str, prerender_fn: F)
where
    F: FnOnce(Arc<IsrCache>) -> Fut,
    Fut: Future<Output = ()>,
{
    let isr_cache = Arc::new(IsrCache::new());
    prerender_fn(Arc::clone(&isr_cache)).await;

    let entries = isr_cache.export_entries();
    let total = entries.len();
    for (key, html) in entries {
        let rel = key.trim_start_matches('/');
        let out_path = if rel.is_empty() {
            std::path::Path::new(dir).join("index.html")
        } else {
            std::path::Path::new(dir).join(rel).join("index.html")
        };
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&out_path, html) {
            Ok(()) => println!("  exported {key} → {}", out_path.display()),
            Err(e) => eprintln!("  failed to write {}: {e}", out_path.display()),
        }
    }
    println!("export complete: {total} pages → {dir}");
}

async fn isr_inspect_handler(
    axum::Extension(handle): axum::Extension<IsrHandle>,
) -> axum::response::Response {
    match handle.__arc() {
        Some(cache) => axum::Json(cache.snapshot()).into_response(),
        None => (StatusCode::OK, axum::Json(serde_json::json!([]))).into_response(),
    }
}
