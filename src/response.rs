use crate::headers::*;
use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration;
use headers::HeaderMapExt;
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
    pub cookies: CookieJar,
    pub toasts: Vec<Toast>, // Future-proof: multiple toasts
}

impl BaseResponse {
    pub fn apply_to_response(&self, response: &mut Response) {
        // 1. Apply standard headers
        self.headers.iter().for_each(|(name, value)| {
            response.headers_mut().insert(name.clone(), value.clone());
        });

        // 2. Prepare all cookies, including toasts
        let mut final_jar = self.cookies.clone();

        if !self.toasts.is_empty() {
            if let Ok(json_string) = serde_json::to_string(&self.toasts) {
                let encoded = urlencoding::encode(&json_string).into_owned();
                let toast_cookie = Cookie::build(("silcrow_toasts", encoded))
                    .path("/")
                    .same_site(SameSite::Lax)
                    .max_age(Duration::seconds(5))
                    .build();
                final_jar = final_jar.add(toast_cookie);
            }
        }

        // 3. Apply cookies
        for cookie in final_jar.iter() {
            if let Ok(header_value) = HeaderValue::from_str(&cookie.to_string()) {
                response
                    .headers_mut()
                    .append(axum::http::header::SET_COOKIE, header_value);
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

    fn no_cache(mut self) -> Self {
        self.base_mut()
            .headers
            .typed_insert(SilcrowCache("no-cache".to_string()));
        self
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
        self.base_mut()
            .headers
            .typed_insert(SilcrowTrigger(map.to_string()));
        self
    }
    //  Override the target DOM element for the swap
    fn retarget(mut self, selector: &str) -> Self {
        self.base_mut()
            .headers
            .typed_insert(SilcrowRetarget(selector.to_string()));
        self
    }

    // Force the browser history URL, or prevent it entirely with "false"
    fn push_history(mut self, url: &str) -> Self {
        self.base_mut()
            .headers
            .typed_insert(SilcrowPush(url.to_string()));
        self
    }

    /// Server-driven patch: tells Silcrow.js to patch JSON data into a specific root element.
    fn patch_target(mut self, selector: &str, data: &impl serde::Serialize) -> Self {
        let payload = serde_json::json!({ "target": selector, "data": data });
        self.base_mut()
            .headers
            .typed_insert(SilcrowPatch(payload.to_string()));
        self
    }

    /// Server-driven invalidation: tells Silcrow.js to rebuild binding maps for a root.
    fn invalidate_target(mut self, selector: &str) -> Self {
        self.base_mut()
            .headers
            .typed_insert(SilcrowInvalidate(selector.to_string()));
        self
    }

    /// Server-driven navigation: tells Silcrow.js to perform a client-side navigation.
    fn client_navigate(mut self, path: &str) -> Self {
        self.base_mut()
            .headers
            .typed_insert(SilcrowNavigate(path.to_string()));
        self
    }

    /// Server-driven SSE: tells Silcrow.js to open an SSE connection to the given path.
    fn sse(mut self, path: impl AsRef<str>) -> Self {
        self.base_mut()
            .headers
            .typed_insert(SilcrowSse(path.as_ref().to_string()));
        self
    }
    fn ws(mut self, path: impl AsRef<str>) -> Self
    where
        Self: Sized,
    {
        self.base_mut()
            .headers
            .typed_insert(SilcrowWs(path.as_ref().to_string()));
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
        serde_json::to_value(&self.data)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
            .map(|json_payload| {
                if self.base.toasts.is_empty() {
                    json_payload
                } else {
                    let toasts_json = serde_json::json!(self.base.toasts);
                    match json_payload {
                        serde_json::Value::Object(mut map) => {
                            map.insert("_toasts".to_string(), toasts_json);
                            serde_json::Value::Object(map)
                        }
                        other => serde_json::json!({
                            "data": other,
                            "_toasts": toasts_json
                        }),
                    }
                }
            })
            .map(|final_payload| {
                let mut response = Json(final_payload).into_response();
                self.base.apply_to_response(&mut response);
                response
            })
            .unwrap_or_else(std::convert::identity)
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
