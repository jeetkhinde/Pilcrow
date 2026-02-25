use axum::{
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};

// ════════════════════════════════════════════════════════════
// 1. Shared State & Modifiers
// ════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toast {
    pub message: String,
    pub level: String,
}

#[derive(Default)]
pub struct BaseResponse {
    pub headers: HeaderMap,
    pub cookies: Vec<Cookie<'static>>,
    pub toasts: Vec<Toast>, // Future-proof: multiple toasts
}

impl BaseResponse {
    /// Applies all headers and standard cookies to the Axum response.
    /// (Fix #4: Centralized emission logic)
    pub fn apply_to_response(&self, response: &mut Response) {
        // 1. Apply standard headers
        for (name, value) in &self.headers {
            response.headers_mut().insert(name.clone(), value.clone());
        }

        // 2. Apply standard cookies
        for cookie in &self.cookies {
            if let Ok(header_value) = HeaderValue::from_str(&cookie.to_string()) {
                response.headers_mut().append(SET_COOKIE, header_value);
            }
        }
    }

    /// Safely formats toasts as URL-encoded cookies for HTML/Navigate responses.
    /// (Fix #3: Safe Cookie formatting)
    pub fn apply_toast_cookies(&self, response: &mut Response) {
        // If we have multiple toasts, we serialize the array to JSON, then URL-encode it
        if !self.toasts.is_empty() {
            if let Ok(json_string) = serde_json::to_string(&self.toasts) {
                let encoded = urlencoding::encode(&json_string);

                let cookie = Cookie::build(("silcrow_toasts", encoded.into_owned()))
                    .path("/")
                    .same_site(SameSite::Lax)
                    .max_age(cookie::time::Duration::seconds(5))
                    .build();

                if let Ok(header_value) = HeaderValue::from_str(&cookie.to_string()) {
                    response.headers_mut().append(SET_COOKIE, header_value);
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════
// 2. The Modifier Trait
// ════════════════════════════════════════════════════════════

// In src/response.rs, update the ResponseExt trait:

pub trait ResponseExt: Sized {
    fn base_mut(&mut self) -> &mut BaseResponse;

    fn with_header(mut self, key: &'static str, value: &'static str) -> Self {
        if let Ok(val) = HeaderValue::from_str(value) {
            self.base_mut().headers.insert(key, val);
        }
        self
    }

    fn no_cache(self) -> Self {
        self.with_header("silcrow-cache", "no-cache")
    }

    fn with_toast(mut self, message: impl Into<String>, level: impl Into<String>) -> Self {
        self.base_mut().toasts.push(Toast {
            message: message.into(),
            level: level.into(),
        });
        self
    }

    // Trigger a custom DOM event on the client
    fn trigger_event(mut self, event_name: &str) -> Self {
        let map = serde_json::json!({ event_name: {} });
        if let Ok(val) = HeaderValue::from_str(&map.to_string()) {
            self.base_mut().headers.insert("silcrow-trigger", val);
        }
        self
    }
    //  Override the target DOM element for the swap
    fn retarget(mut self, selector: &str) -> Self {
        if let Ok(val) = HeaderValue::from_str(selector) {
            self.base_mut().headers.insert("silcrow-retarget", val);
        }
        self
    }

    // Force the browser history URL, or prevent it entirely with "false"
    fn push_history(mut self, url: &str) -> Self {
        if let Ok(val) = HeaderValue::from_str(url) {
            self.base_mut().headers.insert("silcrow-push", val);
        }
        self
    }

    /// Server-driven patch: tells Silcrow.js to patch JSON data into a specific root element.
    fn patch_target(mut self, selector: &str, data: &impl serde::Serialize) -> Self {
        let payload = serde_json::json!({ "target": selector, "data": data });
        if let Ok(val) = HeaderValue::from_str(&payload.to_string()) {
            self.base_mut().headers.insert("silcrow-patch", val);
        }
        self
    }

    /// Server-driven invalidation: tells Silcrow.js to rebuild binding maps for a root.
    fn invalidate_target(mut self, selector: &str) -> Self {
        if let Ok(val) = HeaderValue::from_str(selector) {
            self.base_mut().headers.insert("silcrow-invalidate", val);
        }
        self
    }

    /// Server-driven navigation: tells Silcrow.js to perform a client-side navigation.
    fn client_navigate(mut self, path: &str) -> Self {
        if let Ok(val) = HeaderValue::from_str(path) {
            self.base_mut().headers.insert("silcrow-navigate", val);
        }
        self
    }

    /// Server-driven SSE: tells Silcrow.js to open an SSE connection to the given path.
    fn sse(mut self, path: impl AsRef<str>) -> Self {
        if let Ok(val) = HeaderValue::from_str(path.as_ref()) {
            self.base_mut().headers.insert("silcrow-sse", val);
        }
        self
    }
}
// ════════════════════════════════════════════════════════════
// 3. Response Wrappers & Transport Logic
// ════════════════════════════════════════════════════════════

// --- HTML ---
pub struct HtmlResponse {
    pub data: String,
    pub base: BaseResponse,
}
impl From<String> for HtmlResponse {
    fn from(s: String) -> Self {
        html(s)
    }
}

impl From<&str> for HtmlResponse {
    fn from(s: &str) -> Self {
        html(s.to_owned())
    }
}
impl IntoResponse for HtmlResponse {
    fn into_response(self) -> Response {
        let mut response = axum::response::Html(self.data).into_response();
        self.base.apply_to_response(&mut response);
        self.base.apply_toast_cookies(&mut response);
        response
    }
}

// --- JSON ---
pub struct JsonResponse<T> {
    pub data: T,
    pub base: BaseResponse,
}

impl<T: serde::Serialize> IntoResponse for JsonResponse<T> {
    fn into_response(self) -> Response {
        // Fix #1: Never unwrap serialization. Return 500 if it fails.
        let mut json_payload = match serde_json::to_value(&self.data) {
            Ok(val) => val,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };

        // Fix #2: Handle toasts safely, even if the root isn't an Object
        if !self.base.toasts.is_empty() {
            let toasts_json = serde_json::json!(self.base.toasts);

            if let serde_json::Value::Object(mut map) = json_payload {
                map.insert("_toasts".to_string(), toasts_json);
                json_payload = serde_json::Value::Object(map);
            } else {
                // Option B Safe Wrap: If the user returned an array `json(vec![1, 2])`
                json_payload = serde_json::json!({
                    "data": json_payload,
                    "_toasts": toasts_json
                });
            }
        }

        let mut response = Json(json_payload).into_response();
        self.base.apply_to_response(&mut response); // Apply headers/cookies (but NOT toast cookies)
        response
    }
}

// --- NAVIGATE ---
pub struct NavigateResponse {
    pub path: String,
    pub base: BaseResponse,
}

impl IntoResponse for NavigateResponse {
    fn into_response(self) -> Response {
        // Fix #5: Explicitly using 303 See Other, which is best practice for client-side routers
        let mut response = Redirect::to(&self.path).into_response();

        // Ensure the status is explicitly 303 (Axum defaults to 303 for Redirect::to, but this guarantees it)
        *response.status_mut() = StatusCode::SEE_OTHER;

        self.base.apply_to_response(&mut response);
        self.base.apply_toast_cookies(&mut response);
        response
    }
}

// ════════════════════════════════════════════════════════════
// 4. Constructors & Trait Impls
// ════════════════════════════════════════════════════════════

pub fn html(data: impl Into<String>) -> HtmlResponse {
    HtmlResponse {
        data: data.into(),
        base: BaseResponse::default(),
    }
}

pub fn json<T>(data: T) -> JsonResponse<T> {
    JsonResponse {
        data,
        base: BaseResponse::default(),
    }
}

pub fn navigate(path: impl Into<String>) -> NavigateResponse {
    NavigateResponse {
        path: path.into(),
        base: BaseResponse::default(),
    }
}

impl ResponseExt for HtmlResponse {
    fn base_mut(&mut self) -> &mut BaseResponse {
        &mut self.base
    }
}
impl<T> ResponseExt for JsonResponse<T> {
    fn base_mut(&mut self) -> &mut BaseResponse {
        &mut self.base
    }
}
impl ResponseExt for NavigateResponse {
    fn base_mut(&mut self) -> &mut BaseResponse {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::{html, json, navigate, ResponseExt};
    use axum::{
        body::to_bytes,
        http::{header, StatusCode},
        response::IntoResponse,
    };
    use serde::Serialize;

    #[tokio::test]
    async fn html_response_sets_toast_cookie_and_headers() {
        let response = html("<h1>Hello</h1>")
            .with_toast("Saved", "success")
            .retarget("#sidebar")
            .trigger_event("refresh")
            .no_cache()
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["silcrow-retarget"], "#sidebar");
        assert_eq!(response.headers()["silcrow-trigger"], r#"{"refresh":{}}"#);
        assert_eq!(response.headers()["silcrow-cache"], "no-cache");

        let set_cookie_values: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().expect("set-cookie should be utf8"))
            .collect();

        assert!(set_cookie_values
            .iter()
            .any(|cookie| cookie.starts_with("silcrow_toasts=")));
    }

    #[tokio::test]
    async fn json_response_injects_toasts_into_object_payload() {
        let response = json(serde_json::json!({"ok": true}))
            .with_toast("Saved", "success")
            .into_response();

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("json body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert_eq!(payload["ok"], serde_json::json!(true));
        assert_eq!(payload["_toasts"][0]["message"], "Saved");
        assert_eq!(payload["_toasts"][0]["level"], "success");
    }

    #[tokio::test]
    async fn json_response_wraps_non_object_payload_when_toasts_exist() {
        let response = json(vec![1, 2, 3])
            .with_toast("Done", "info")
            .into_response();

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("json body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert_eq!(payload["data"], serde_json::json!([1, 2, 3]));
        assert_eq!(payload["_toasts"][0]["message"], "Done");
    }

    #[derive(Clone)]
    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("expected serialization failure"))
        }
    }

    #[test]
    fn json_response_returns_500_on_serialization_error() {
        let response = json(FailingSerialize).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn navigate_response_is_303_with_location_and_toast_cookie() {
        let response = navigate("/dashboard")
            .with_toast("Redirected", "info")
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], "/dashboard");

        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().expect("set-cookie should be utf8"))
            .collect();
        assert!(cookies
            .iter()
            .any(|cookie| cookie.starts_with("silcrow_toasts=")));
    }

    // ════════════════════════════════════════════════════════════
    // New: Server-driven header methods
    // ════════════════════════════════════════════════════════════

    #[test]
    fn patch_target_sets_json_header_with_selector_and_data() {
        let response = html("<h1>Updated</h1>")
            .patch_target("#sidebar", &serde_json::json!({"count": 42}))
            .into_response();

        let header = response.headers()["silcrow-patch"]
            .to_str()
            .expect("header should be utf8");
        let parsed: serde_json::Value =
            serde_json::from_str(header).expect("header should be valid json");

        assert_eq!(parsed["target"], "#sidebar");
        assert_eq!(parsed["data"]["count"], 42);
    }

    #[test]
    fn patch_target_works_on_json_response() {
        let response = json(serde_json::json!({"ok": true}))
            .patch_target("#notifications", &serde_json::json!({"unread": 5}))
            .into_response();

        let header = response.headers()["silcrow-patch"]
            .to_str()
            .expect("header should be utf8");
        let parsed: serde_json::Value =
            serde_json::from_str(header).expect("header should be valid json");

        assert_eq!(parsed["target"], "#notifications");
        assert_eq!(parsed["data"]["unread"], 5);
    }

    #[test]
    fn invalidate_target_sets_header() {
        let response = html("<h1>Reloaded</h1>")
            .invalidate_target("#app")
            .into_response();

        assert_eq!(response.headers()["silcrow-invalidate"], "#app");
    }

    #[test]
    fn client_navigate_sets_header() {
        let response = json(serde_json::json!({"saved": true}))
            .client_navigate("/dashboard")
            .into_response();

        assert_eq!(response.headers()["silcrow-navigate"], "/dashboard");
    }

    #[test]
    fn sse_sets_header() {
        let response = html("<div id='feed'></div>")
            .sse("/events/feed")
            .into_response();

        assert_eq!(response.headers()["silcrow-sse"], "/events/feed");
    }

    #[test]
    fn all_new_headers_chain_together() {
        let response = html("<h1>Full</h1>")
            .patch_target("#stats", &serde_json::json!({"online": 100}))
            .invalidate_target("#sidebar")
            .client_navigate("/home")
            .sse("/events/live")
            .into_response();

        assert!(response.headers().contains_key("silcrow-patch"));
        assert_eq!(response.headers()["silcrow-invalidate"], "#sidebar");
        assert_eq!(response.headers()["silcrow-navigate"], "/home");
        assert_eq!(response.headers()["silcrow-sse"], "/events/live");
    }

    #[test]
    fn navigate_response_supports_new_headers() {
        let response = navigate("/login")
            .client_navigate("/auth/callback")
            .invalidate_target("#session")
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()["silcrow-navigate"], "/auth/callback");
        assert_eq!(response.headers()["silcrow-invalidate"], "#session");
    }
}
