// ./crates/pilcrow/src/assets.rs

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

pub const SILCROW_JS: &str = include_str!("../assets/silcrow.js");

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
    let hash = crc32fast::hash(SILCROW_JS.as_bytes());
    format!("/_silcrow/silcrow.{hash:08x}.js")
}
pub fn script_tag() -> String {
    format!(r#"<script src="{}" defer></script>"#, silcrow_js_path())
}
