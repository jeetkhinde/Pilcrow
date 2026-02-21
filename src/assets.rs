// ./crates/pilcrow/src/assets.rs

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// The unified Silcrow client runtime, embedded at compile time.
pub const SILCROW_JS: &str = include_str!("../public/silcrow.js");

/// Canonical URL path for serving the Silcrow JS bundle.
pub const SILCROW_JS_PATH: &str = "/_silcrow/silcrow.js";

/// Axum handler that serves the embedded Silcrow JS with aggressive caching.
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

/// Returns a raw HTML `<script>` tag pointing to the Silcrow JS bundle.
pub fn script_tag() -> &'static str {
    "<script src=\"/_silcrow/silcrow.js\" defer></script>"
}

#[cfg(test)]
mod tests {
    use super::{script_tag, serve_silcrow_js, SILCROW_JS, SILCROW_JS_PATH};
    use axum::{
        body::to_bytes,
        http::{header, StatusCode},
    };

    #[test]
    fn script_tag_uses_canonical_path() {
        assert_eq!(
            script_tag(),
            r#"<script src="/_silcrow/silcrow.js" defer></script>"#
        );
        assert_eq!(SILCROW_JS_PATH, "/_silcrow/silcrow.js");
    }

    #[tokio::test]
    async fn serve_silcrow_js_returns_expected_headers_and_body() {
        let response = serve_silcrow_js().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let body_text = String::from_utf8(body.to_vec()).expect("js payload should be utf8");

        assert_eq!(body_text, SILCROW_JS);
    }
}
