// ./crates/pilcrow/src/assets.rs

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// The unified Silcrow client runtime, embedded at compile time.
pub const SILCROW_JS: &str = include_str!("../public/silcrow.js");

/// Canonical URL path for serving the Silcrow JS bundle.
const SILCROW_JS_HASH: &str = env!("SILCROW_JS_HASH");

pub async fn serve_silcrow_js() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        SILCROW_JS,
    )
        .into_response()
}

pub fn silcrow_js_path() -> String {
    format!("/_silcrow/silcrow.{SILCROW_JS_HASH}.js")
}
pub fn script_tag() -> String {
    format!(r#"<script src="{}" defer></script>"#, silcrow_js_path())
}
