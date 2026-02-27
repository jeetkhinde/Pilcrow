// ./crates/pilcrow/src/select.rs
use crate::extract::{RequestMode, SilcrowRequest};
use crate::response::{html, json, HtmlResponse, JsonResponse};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::future::Future;
use std::pin::Pin;
// ════════════════════════════════════════════════════════════
// 1. The Polymorphic Conversion Traits
// ════════════════════════════════════════════════════════════

pub trait IntoPilcrowHtml<E> {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E>;
}
impl<E> IntoPilcrowHtml<E> for String {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        Ok(html(self))
    }
}
impl<E> IntoPilcrowHtml<E> for HtmlResponse {
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        Ok(self)
    }
}
impl<R, E> IntoPilcrowHtml<E> for Result<R, E>
where
    R: IntoPilcrowHtml<E>,
{
    fn into_pilcrow_html(self) -> Result<HtmlResponse, E> {
        self.and_then(IntoPilcrowHtml::into_pilcrow_html)
    }
}

pub trait IntoPilcrowJson<E> {
    fn into_pilcrow_json(self) -> Result<Response, E>;
}

impl<T, E> IntoPilcrowJson<E> for JsonResponse<T>
where
    T: serde::Serialize,
{
    fn into_pilcrow_json(self) -> Result<Response, E> {
        Ok(self.into_response())
    }
}

impl<E> IntoPilcrowJson<E> for serde_json::Value {
    fn into_pilcrow_json(self) -> Result<Response, E> {
        Ok(json(self).into_response())
    }
}

impl<T, E> IntoPilcrowJson<E> for Result<T, E>
where
    T: IntoPilcrowJson<E>,
{
    fn into_pilcrow_json(self) -> Result<Response, E> {
        self.and_then(IntoPilcrowJson::into_pilcrow_json)
    }
}

// ════════════════════════════════════════════════════════════
// 2. The Type-Erased Responses Builder
// ════════════════════════════════════════════════════════════
type AsyncResponseFn<E> =
    Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<Response, E>> + Send>> + Send>;
pub struct Responses<E> {
    html: Option<AsyncResponseFn<E>>,
    json: Option<AsyncResponseFn<E>>,
}

impl<E> Default for Responses<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Responses<E> {
    pub fn new() -> Self {
        Self {
            html: None,
            json: None,
        }
    }

    /// Registers the HTML response generator.
    pub fn html<F, Fut, T>(mut self, f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
        T: IntoPilcrowHtml<E> + 'static,
        E: 'static,
    {
        self.html = Some(Box::new(|| {
            Box::pin(async move { f().await.into_pilcrow_html().map(|res| res.into_response()) })
        }));
        self
    }
    /// Registers the JSON response generator.
    pub fn json<F, Fut, T>(mut self, f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
        T: IntoPilcrowJson<E> + 'static,
        E: 'static,
    {
        self.json = Some(Box::new(|| {
            Box::pin(async move { f().await.into_pilcrow_json() })
        }));
        self
    }
}
// ════════════════════════════════════════════════════════════
// 3. The Core Selector Implementation
// ════════════════════════════════════════════════════════════
/// Evaluates the preferred mode (HTML or JSON) and executes *only* the matching closure
/// from the provided `Responses` builder.
/// `E` represents the application's custom error type, which must be convertible to an Axum `Response`.
impl SilcrowRequest {
    pub async fn select<E>(&self, responses: Responses<E>) -> Result<Response, E> {
        match self.preferred_mode() {
            RequestMode::Html => {
                if let Some(f) = responses.html {
                    f().await
                } else {
                    Ok((StatusCode::NOT_ACCEPTABLE, "HTML not provided").into_response())
                }
            }
            RequestMode::Json => {
                if let Some(f) = responses.json {
                    f().await
                } else {
                    Ok((StatusCode::NOT_ACCEPTABLE, "JSON not provided").into_response())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Responses;
    use crate::extract::SilcrowRequest;
    use axum::{body::to_bytes, http::StatusCode, response::Response};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn select_executes_only_html_branch_for_html_request() {
        let req = SilcrowRequest {
            is_silcrow: false,
            accepts_html: true,
            accepts_json: true,
        };

        let html_calls = Arc::new(AtomicUsize::new(0));
        let json_calls = Arc::new(AtomicUsize::new(0));

        let html_calls_clone = Arc::clone(&html_calls);
        let json_calls_clone = Arc::clone(&json_calls);

        let response = req
            .select::<Response>(
                Responses::new()
                    .html(move || async move {
                        html_calls_clone.fetch_add(1, Ordering::SeqCst);
                        "<p>html</p>".to_string()
                    })
                    .json(move || async move {
                        json_calls_clone.fetch_add(1, Ordering::SeqCst);
                        serde_json::json!({"mode": "json"})
                    }),
            )
            .await
            .expect("selection should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(html_calls.load(Ordering::SeqCst), 1);
        assert_eq!(json_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn select_returns_406_when_requested_format_is_missing() {
        let req = SilcrowRequest {
            is_silcrow: false,
            accepts_html: true,
            accepts_json: false,
        };

        let response = req
            .select::<Response>(Responses::new().json(|| async { serde_json::json!({"ok": true}) }))
            .await
            .expect("selection should return fallback response");

        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
    }

    #[tokio::test]
    async fn select_supports_json_result_closures() {
        let req = SilcrowRequest {
            is_silcrow: true,
            accepts_html: false,
            accepts_json: true,
        };

        let response = req
            .select::<Response>(
                Responses::new()
                    .json(|| async { Ok::<_, Response>(serde_json::json!({"ok": true})) }),
            )
            .await
            .expect("selection should succeed");

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json should parse");
        assert_eq!(payload["ok"], serde_json::json!(true));
    }

    #[tokio::test]
    async fn select_propagates_custom_errors() {
        let req = SilcrowRequest {
            is_silcrow: true,
            accepts_html: true,
            accepts_json: false,
        };

        let err = req
            .select::<StatusCode>(
                Responses::new().html(|| async { Err::<String, _>(StatusCode::BAD_REQUEST) }),
            )
            .await
            .expect_err("error should propagate from closure");

        assert_eq!(err, StatusCode::BAD_REQUEST);
    }
}
