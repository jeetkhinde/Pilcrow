use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use axum::{
    async_trait,
    extract::{Form, FromRequest, FromRequestParts, Path, Request},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    http::request::Parts,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use cookie::time::Duration;
use headers::HeaderMapExt;
use pilcrow_core::AppError;
use serde::Serialize;

use crate::response::headers::*;
use crate::response::response::{ActionResult, BaseResponse, FormErrors, Toast, ToastLevel};

// ── Locals ────────────────────────────────────────────────────

/// Per-request typed store shared across all `load()` calls in the same request.
///
/// `Locals` uses `Arc<RwLock<...>>` so that cloning `Req` (which the framework
/// does when passing it to layout and page loads) still refers to the same underlying
/// map.  A layout's `load()` can write to `req.locals`, and the page's `load()` will
/// see it without re-fetching.
///
/// ```rust,ignore
/// // _layout.rs
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let user = auth::verify(&req.cookies).await?;
///     req.locals.set(user);
///     Ok(Props { ... })
/// }
///
/// // products.rs
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let user = req.locals.require::<User>()?;
///     Ok(Props { ... })
/// }
/// ```
#[derive(Clone, Default)]
pub struct Locals(Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>);

impl std::fmt::Debug for Locals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locals").finish_non_exhaustive()
    }
}

impl Locals {
    /// Store a value of type `T`. Overwrites any previous value of the same type.
    pub fn set<T: Send + Sync + 'static>(&self, value: T) {
        self.0.write().unwrap().insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieve a clone of the stored value of type `T`, or `None` if not set.
    pub fn get<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        self.0
            .read()
            .unwrap()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Like `get`, but returns `Err(AppError::Unauthorized)` when absent.
    ///
    /// ```rust,ignore
    /// let user = req.locals.require::<User>()?;
    /// ```
    pub fn require<T: Clone + Send + Sync + 'static>(&self) -> Result<T, AppError> {
        self.get::<T>().ok_or(AppError::Unauthorized)
    }

    /// `true` if a value of type `T` has been set.
    pub fn has<T: 'static>(&self) -> bool {
        self.0.read().unwrap().contains_key(&TypeId::of::<T>())
    }
}

// ── Res ──────────────────────────────────────────────────────

/// Per-request response modifier available inside `load()` and `actions()`.
///
/// Lets handlers set response headers, cookies, and toasts without changing the
/// return type.  The framework applies accumulated modifications to the final
/// rendered `Response` after all handlers complete.
///
/// ```rust,ignore
/// pub async fn load(req: Req) -> AppResult<Props> {
///     req.res.no_cache();
///     req.res.with_toast("Welcome back!", ToastLevel::Info);
///     Ok(Props { ... })
/// }
/// ```
#[derive(Clone, Default)]
pub struct Res(Arc<Mutex<BaseResponse>>);

impl std::fmt::Debug for Res {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Res").finish_non_exhaustive()
    }
}

impl Res {
    /// Set an explicit HTTP status code on the rendered page response.
    pub fn with_status(&self, status: StatusCode) -> &Self {
        self.0.lock().unwrap().status = Some(status);
        self
    }

    /// Append a raw response header.
    pub fn with_header(&self, key: &'static str, value: impl Into<String>) -> &Self {
        if let Ok(val) = HeaderValue::from_str(&value.into()) {
            self.0.lock().unwrap().headers.insert(key, val);
        }
        self
    }

    /// Add `silcrow-cache: no-cache` so silcrow.js skips the response cache.
    pub fn no_cache(&self) -> &Self {
        self.0.lock().unwrap().headers.typed_insert(SilcrowCache("no-cache".to_string()));
        self
    }

    /// Add a `Set-Cookie` header to the response.
    pub fn with_cookie(&self, cookie: Cookie<'static>) -> &Self {
        let mut base = self.0.lock().unwrap();
        base.cookies = std::mem::take(&mut base.cookies).add(cookie);
        self
    }

    /// Queue a toast notification. The client reads this from the `silcrow_toasts` cookie.
    pub fn with_toast(&self, message: impl Into<String>, level: ToastLevel) -> &Self {
        self.0.lock().unwrap().toasts.push(Toast { message: message.into(), level });
        self
    }

    /// Fire a custom DOM event on the client via `silcrow-trigger`.
    /// Multiple calls accumulate — all named events are sent in a single header.
    pub fn trigger_event(&self, event_name: &str) -> &Self {
        let mut base = self.0.lock().unwrap();
        let mut map = base
            .headers
            .typed_get::<SilcrowTrigger>()
            .and_then(|h| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&h.0).ok())
            .unwrap_or_default();
        map.insert(event_name.to_string(), serde_json::json!({}));
        base.headers.typed_insert(SilcrowTrigger(serde_json::Value::Object(map).to_string()));
        self
    }

    /// Override the swap target selector via `silcrow-retarget`.
    pub fn retarget(&self, selector: &str) -> &Self {
        self.0.lock().unwrap().headers.typed_insert(SilcrowRetarget(selector.to_string()));
        self
    }

    /// Push a URL to the browser history via `silcrow-push`.
    pub fn push_history(&self, url: &str) -> &Self {
        self.0.lock().unwrap().headers.typed_insert(SilcrowPush(url.to_string()));
        self
    }

    /// Patch a secondary DOM target via `silcrow-patch`.
    /// Multiple calls accumulate — each `{target, data}` entry is carried in
    /// a single JSON-array header and applied in call order on the client.
    pub fn patch_target(&self, selector: &str, data: &impl Serialize) -> &Self {
        let mut base = self.0.lock().unwrap();
        let mut list = base
            .headers
            .typed_get::<SilcrowPatch>()
            .and_then(|h| serde_json::from_str::<Vec<serde_json::Value>>(&h.0).ok())
            .unwrap_or_default();
        list.push(serde_json::json!({ "data": data, "target": selector }));
        base.headers.typed_insert(SilcrowPatch(serde_json::Value::Array(list).to_string()));
        self
    }

    /// Invalidate a DOM target's binding cache via `silcrow-invalidate`.
    /// Multiple calls accumulate — all selectors are carried in a single
    /// JSON-array header and invalidated in call order on the client.
    pub fn invalidate_target(&self, selector: &str) -> &Self {
        let mut base = self.0.lock().unwrap();
        let mut list = base
            .headers
            .typed_get::<SilcrowInvalidate>()
            .and_then(|h| serde_json::from_str::<Vec<String>>(&h.0).ok())
            .unwrap_or_default();
        list.push(selector.to_string());
        base.headers.typed_insert(SilcrowInvalidate(serde_json::to_string(&list).unwrap_or_default()));
        self
    }

    /// Trigger a client-side navigation via `silcrow-navigate`.
    pub fn client_navigate(&self, path: &str) -> &Self {
        self.0.lock().unwrap().headers.typed_insert(SilcrowNavigate(path.to_string()));
        self
    }

    /// Open an SSE connection on the client via `silcrow-sse`.
    pub fn sse(&self, path: impl AsRef<str>) -> &Self {
        self.0.lock().unwrap().headers.typed_insert(SilcrowSse(path.as_ref().to_string()));
        self
    }

    /// Open a WebSocket connection on the client via `silcrow-ws`.
    pub fn ws(&self, path: impl AsRef<str>) -> &Self {
        self.0.lock().unwrap().headers.typed_insert(SilcrowWs(path.as_ref().to_string()));
        self
    }

    /// Apply all accumulated modifications to an existing response.
    ///
    /// Called by generated handler code after rendering is complete.
    pub fn apply_to(&self, response: &mut Response) {
        self.0.lock().unwrap().apply_to_response(response);
    }
}

// ── Action parsing ───────────────────────────────────────────

/// Extract the named action from a raw URL query string (`?/<name>`).
///
/// Scans the query string for the first key that starts with `/` and returns
/// the URL-decoded remainder as the action name. Returns `None` when no such
/// key is present.
fn extract_action_from_query(raw_query: Option<&str>) -> Option<String> {
    let query = raw_query?;
    for pair in query.split('&') {
        let key = pair.split('=').next()?;
        if let Some(stripped) = key.strip_prefix('/') {
            return match urlencoding::decode(stripped) {
                Ok(decoded) => Some(decoded.into_owned()),
                Err(_) => Some(stripped.to_string()),
            };
        }
    }
    None
}

// ── FormMap ───────────────────────────────────────────────────

/// Multi-value URL-encoded map. Used for both the form body (`req.form`) and
/// the query string (`req.query`); both share the same repeated-key semantics
/// (e.g. `?tag=a&tag=b` or `<input name="tag" …>` twice).
#[derive(Debug, Default, Clone)]
pub struct FormMap(pub HashMap<String, Vec<String>>);

impl FormMap {
    /// First value for a key, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.first().map(String::as_str)
    }

    /// All values for a key (e.g. multiple checkboxes with the same name, or
    /// repeated query params like `?tag=a&tag=b`).
    pub fn get_all(&self, key: &str) -> &[String] {
        self.0.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// `true` if the key appears at least once.
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Iterator over distinct keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

/// Parse a raw query string into a multi-value [`FormMap`], stripping any
/// `?/<name>` action markers (those are read via [`Req::action`]).
///
/// Handles standard URL-encoded form semantics: `+` is decoded as space and
/// `%XX` sequences are percent-decoded. Empty pairs are ignored.
fn parse_query_multi(raw_query: Option<&str>) -> FormMap {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let Some(q) = raw_query else { return FormMap(out) };
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_val) = pair.split_once('=').unwrap_or((pair, ""));
        // `?/<name>` keys are action markers; readable via req.action().
        if raw_key.starts_with('/') {
            continue;
        }
        out.entry(decode_form_component(raw_key))
            .or_default()
            .push(decode_form_component(raw_val));
    }
    FormMap(out)
}

fn decode_form_component(s: &str) -> String {
    // `+` → space is form-urlencoded-specific (not covered by percent-decoding).
    let with_spaces = s.replace('+', " ");
    urlencoding::decode(&with_spaces)
        .map(|c| c.into_owned())
        .unwrap_or(with_spaces)
}

// ── Req ──────────────────────────────────────────────────────

/// Unified request context for both `load()` and named action handlers.
///
/// Replaces the old `PageContext` / `ActionContext` split — one type, same mental
/// model everywhere.  `req.form` is empty on GET requests; `req.res` accumulates
/// response side-effects (headers, cookies, toasts) that the framework applies
/// after the handler returns.
///
/// ```rust,ignore
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let user = req.locals.require::<User>()?;
///     req.res.no_cache();
///     Ok(Props { user })
/// }
///
/// // Named action — invoked when the client POSTs `?/create`.
/// pub async fn create(req: Req) -> ActionResult {
///     let name = req.form.get("name").unwrap_or("");
///     redirect("/items")
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Req {
    /// Named path capture groups. `/posts/[id]` → `{"id": "42"}`.
    pub params: HashMap<String, String>,
    /// Query string as a multi-value map (`?category=shoes&tag=a&tag=b` →
    /// `{"category": ["shoes"], "tag": ["a", "b"]}`). Use `.get(key)` for the
    /// first value, `.get_all(key)` for every value.
    ///
    /// The `?/name` action marker is stripped out of this map — read it via
    /// [`Req::action`] instead.
    pub query: FormMap,
    /// Parsed URL-encoded form body. Empty on GET requests.
    pub form: FormMap,
    /// Request cookies.
    pub cookies: CookieJar,
    /// Raw request headers.
    pub headers: HeaderMap,
    /// The current request path, e.g. `/products` or `/posts/42`.
    pub path: String,
    /// `true` when the request was sent by silcrow.js (has a `silcrow-target` header).
    ///
    /// Use `req.fail(form_errors)` instead of checking this directly — it handles
    /// the JSON-vs-flash decision automatically.
    pub is_enhanced: bool,
    /// Per-request typed store shared across all loads in this request.
    pub locals: Locals,
    /// Response modifier: set headers, cookies, toasts from inside any handler.
    pub res: Res,
    /// The named action, parsed from `?/<name>`. `None` when the URL has no
    /// action marker (default POST / GET). See [`Req::action`].
    action: Option<String>,
}

impl Req {
    /// Return the named action from the current request URL.
    ///
    /// The server parses `?/<name>` from the query string — matching the
    /// SvelteKit convention. Returns `""` when no action marker is present.
    ///
    /// ```rust,ignore
    /// // HTML: <form s-post="?/create">…</form>
    /// // Code-behind:
    /// pub async fn create(req: Req) -> ActionResult {
    ///     let name = req.form.get("name").unwrap_or("");
    ///     redirect("/items")
    /// }
    /// ```
    pub fn action(&self) -> &str {
        self.action.as_deref().unwrap_or("")
    }

    /// Return the smart form-error response for the current request context.
    ///
    /// - **Enhanced** (`req.is_enhanced == true`, i.e. silcrow.js sent this): returns
    ///   JSON that silcrow.js patches into the form via `:text`/`:show`/`:value` bindings.
    /// - **Plain browser POST**: serialises the errors into a short-lived
    ///   `silcrow_form_flash` cookie and issues a `303 → req.path` redirect.
    ///   The page's `load()` reads the flash with [`Req::take_form_flash`].
    ///
    /// ```rust,ignore
    /// pub async fn signup(req: Req) -> ActionResult {
    ///     let email = req.form.get("email").unwrap_or("");
    ///     if email.is_empty() {
    ///         return req.fail(form_errors()
    ///             .error("email", "Email is required")
    ///             .value("email", email));
    ///     }
    ///     redirect("/dashboard")
    /// }
    /// ```
    pub fn fail(&self, errors: FormErrors) -> ActionResult {
        if self.is_enhanced {
            Ok(axum::Json(errors).into_response())
        } else {
            let json = serde_json::to_string(&errors).unwrap_or_default();
            let encoded = urlencoding::encode(&json).into_owned();
            let flash_cookie = Cookie::build(("silcrow_form_flash", encoded))
                .path("/")
                .same_site(SameSite::Lax)
                .max_age(Duration::seconds(30))
                .build();
            let mut response = Redirect::to(&self.path).into_response();
            *response.status_mut() = StatusCode::SEE_OTHER;
            if let Ok(header_value) = HeaderValue::from_str(&flash_cookie.to_string()) {
                response.headers_mut().append(header::SET_COOKIE, header_value);
            }
            Ok(response)
        }
    }

    /// Read and clear the form flash cookie set by [`Req::fail`] on a
    /// non-enhanced (native browser) POST that failed validation.
    ///
    /// Call this at the top of your page `load()` to repopulate the form UI:
    ///
    /// ```rust,ignore
    /// pub async fn load(req: Req) -> AppResult<Props> {
    ///     let flash = req.take_form_flash();
    ///     Ok(Props { errors: flash, ..Default::default() })
    /// }
    /// ```
    ///
    /// The cookie is automatically cleared so the errors don't persist on refresh.
    pub fn take_form_flash(&self) -> Option<FormErrors> {
        let cookie = self.cookies.get("silcrow_form_flash")?;
        let decoded = urlencoding::decode(cookie.value()).ok()?;
        let flash: FormErrors = serde_json::from_str(&decoded).ok()?;
        let removal = Cookie::build(("silcrow_form_flash", ""))
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(Duration::seconds(0))
            .build();
        self.res.with_cookie(removal);
        Some(flash)
    }

    /// Extract a `Req` from request `Parts` only — no body consumed.
    ///
    /// Used by the generated middleware glue so the original `Request` can be
    /// reconstructed (with body intact) and forwarded through `Next`.
    ///
    /// - `form` is always empty (body not consumed)
    /// - `Locals` and `Res` are inserted into `parts.extensions` so they are
    ///   shared with the downstream page/action handlers
    ///
    /// This is `#[doc(hidden)]` — it is not part of the public API.
    #[doc(hidden)]
    pub async fn __from_middleware_parts<S: Send + Sync>(parts: &mut Parts, state: &S) -> Self {
        let params = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map(|p| p.0)
            .unwrap_or_default();

        let action = extract_action_from_query(parts.uri.query());

        let query = parse_query_multi(parts.uri.query());

        let cookies = CookieJar::from_request_parts(parts, state)
            .await
            .unwrap_or_default();

        let headers = parts.headers.clone();
        let path = parts.uri.path().to_owned();
        let is_enhanced = parts.headers.typed_get::<SilcrowTarget>().is_some();

        // Insert Locals and Res into extensions so downstream handlers see the same instances.
        let locals = parts
            .extensions
            .get::<Locals>()
            .cloned()
            .unwrap_or_else(|| {
                let l = Locals::default();
                parts.extensions.insert(l.clone());
                l
            });

        let res = parts
            .extensions
            .get::<Res>()
            .cloned()
            .unwrap_or_else(|| {
                let r = Res::default();
                parts.extensions.insert(r.clone());
                r
            });

        Req {
            params,
            query,
            form: FormMap::default(),
            cookies,
            headers,
            path,
            is_enhanced,
            locals,
            res,
            action,
        }
    }
}

#[async_trait]
impl<S: Send + Sync> FromRequest<S> for Req {
    type Rejection = (StatusCode, &'static str);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();

        let params = Path::<HashMap<String, String>>::from_request_parts(&mut parts, state)
            .await
            .map(|p| p.0)
            .unwrap_or_default();

        let action = extract_action_from_query(parts.uri.query());

        let query = parse_query_multi(parts.uri.query());

        let cookies = CookieJar::from_request_parts(&mut parts, state)
            .await
            .unwrap_or_default();

        let headers = parts.headers.clone();
        let path = parts.uri.path().to_owned();
        let is_enhanced = parts.headers.typed_get::<SilcrowTarget>().is_some();

        // Shared per-request Locals: first extraction creates and inserts; subsequent ones share.
        let locals = parts
            .extensions
            .get::<Locals>()
            .cloned()
            .unwrap_or_else(|| {
                let l = Locals::default();
                parts.extensions.insert(l.clone());
                l
            });

        // Shared per-request Res: same pattern.
        let res = parts
            .extensions
            .get::<Res>()
            .cloned()
            .unwrap_or_else(|| {
                let r = Res::default();
                parts.extensions.insert(r.clone());
                r
            });

        // Reconstruct the request so Form can consume the body.
        let req = Request::from_parts(parts, body);
        let pairs: Vec<(String, String)> =
            Form::<Vec<(String, String)>>::from_request(req, state)
                .await
                .map(|f| f.0)
                .unwrap_or_default();

        let mut raw_map: HashMap<String, Vec<String>> = HashMap::new();
        for (k, v) in pairs {
            raw_map.entry(k).or_default().push(v);
        }
        let form = FormMap(raw_map);

        Ok(Req { params, query, form, cookies, headers, path, is_enhanced, locals, res, action })
    }
}
