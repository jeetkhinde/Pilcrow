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
#[cfg(test)]
mod tests {
    use super::{script_tag, serve_silcrow_js, silcrow_js_path, SILCROW_JS};
    use axum::{
        body::to_bytes,
        http::{header, StatusCode},
    };

    #[test]
    fn script_tag_uses_fingerprinted_path() {
        let path = silcrow_js_path();
        assert!(path.starts_with("/_silcrow/silcrow."));
        assert!(path.ends_with(".js"));
        assert_eq!(
            script_tag(),
            format!(r#"<script src="{path}" defer></script>"#)
        );
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
