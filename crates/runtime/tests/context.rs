/// Integration and unit tests for `Locals`, `FormMap`, and `Req`.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use http_body_util::BodyExt;
use pilcrow_core::AppError;
use runtime::{FormMap, Locals, Req, form_errors};
use tower::ServiceExt;

// ── Helpers ──────────────────────────────────────────────────

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn action_echo_app() -> Router {
    Router::new()
        .route("/", get(|req: Req| async move { req.action().to_owned() }))
        .route("/", post(|req: Req| async move { req.action().to_owned() }))
}

fn enhanced_echo_app() -> Router {
    Router::new().route(
        "/",
        get(|req: Req| async move { req.is_enhanced.to_string() }),
    )
}

fn fail_app() -> Router {
    Router::new().route(
        "/",
        // req.fail() always returns Ok(_) — safe to unwrap.
        post(|req: Req| async move {
            req.fail(form_errors().error("email", "Required")).unwrap()
        }),
    )
}

// ── Locals ───────────────────────────────────────────────────

#[test]
fn locals_set_and_get() {
    let locals = Locals::default();
    locals.set(42u32);
    assert_eq!(locals.get::<u32>(), Some(42));
}

#[test]
fn locals_get_absent_is_none() {
    let locals = Locals::default();
    assert_eq!(locals.get::<u32>(), None);
}

#[test]
fn locals_require_absent_is_unauthorized() {
    let locals = Locals::default();
    assert!(matches!(locals.require::<u32>(), Err(AppError::Unauthorized)));
}

#[test]
fn locals_require_present_returns_value() {
    let locals = Locals::default();
    locals.set(7u32);
    assert_eq!(locals.require::<u32>().unwrap(), 7);
}

#[test]
fn locals_has() {
    let locals = Locals::default();
    assert!(!locals.has::<u32>());
    locals.set(1u32);
    assert!(locals.has::<u32>());
}

#[test]
fn locals_set_overwrites_previous() {
    let locals = Locals::default();
    locals.set(1u32);
    locals.set(2u32);
    assert_eq!(locals.get::<u32>(), Some(2));
}

#[test]
fn locals_clone_shares_state() {
    let locals = Locals::default();
    let cloned = locals.clone();
    locals.set(99u32);
    // Both handles should see the value since they share the same inner Arc.
    assert_eq!(cloned.get::<u32>(), Some(99));
}

#[test]
fn locals_different_types_are_independent() {
    let locals = Locals::default();
    locals.set(1u32);
    locals.set("hello");
    assert_eq!(locals.get::<u32>(), Some(1));
    assert_eq!(locals.get::<&'static str>(), Some("hello"));
}

// ── FormMap ──────────────────────────────────────────────────

fn make_formmap(pairs: &[(&str, &str)]) -> FormMap {
    let mut map = std::collections::HashMap::new();
    for (k, v) in pairs {
        map.entry(k.to_string())
            .or_insert_with(Vec::new)
            .push(v.to_string());
    }
    FormMap(map)
}

#[test]
fn formmap_get_returns_first_value() {
    let fm = make_formmap(&[("name", "alice"), ("name", "bob")]);
    assert_eq!(fm.get("name"), Some("alice"));
}

#[test]
fn formmap_get_absent_is_none() {
    let fm = FormMap::default();
    assert_eq!(fm.get("missing"), None);
}

#[test]
fn formmap_get_all_returns_all() {
    let fm = make_formmap(&[("tag", "a"), ("tag", "b"), ("tag", "c")]);
    assert_eq!(fm.get_all("tag"), &["a", "b", "c"]);
}

#[test]
fn formmap_get_all_absent_is_empty() {
    let fm = FormMap::default();
    assert_eq!(fm.get_all("missing"), &[] as &[String]);
}

#[test]
fn formmap_contains() {
    let fm = make_formmap(&[("x", "1")]);
    assert!(fm.contains("x"));
    assert!(!fm.contains("y"));
}

// ── req.action() ─────────────────────────────────────────────

#[tokio::test]
async fn req_action_from_query_param() {
    let resp = action_echo_app()
        .oneshot(
            Request::builder()
                .uri("/?action=create")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "create");
}

#[tokio::test]
async fn req_action_from_underscore_query() {
    let resp = action_echo_app()
        .oneshot(
            Request::builder()
                .uri("/?_action=delete")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "delete");
}

#[tokio::test]
async fn req_action_from_form_body() {
    let resp = action_echo_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("_action=update"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "update");
}

#[tokio::test]
async fn req_action_defaults_to_empty() {
    let resp = action_echo_app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "");
}

#[tokio::test]
async fn req_action_query_wins_over_form_body() {
    // ?action= in query wins over _action in form body.
    let resp = action_echo_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/?action=from_query")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("_action=from_body"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "from_query");
}

// ── req.is_enhanced ──────────────────────────────────────────

#[tokio::test]
async fn req_is_enhanced_true_with_silcrow_target_header() {
    let resp = enhanced_echo_app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("silcrow-target", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "true");
}

#[tokio::test]
async fn req_is_enhanced_false_without_header() {
    let resp = enhanced_echo_app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(body_string(resp).await, "false");
}

// ── req.fail() ───────────────────────────────────────────────

#[tokio::test]
async fn req_fail_enhanced_returns_json_errors() {
    let resp = fail_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("silcrow-target", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/json"), "expected json, got: {ct}");

    let body: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body["has_errors"], true);
    assert_eq!(body["errors"]["email"], "Required");
}

#[tokio::test]
async fn req_fail_plain_returns_redirect_with_flash_cookie() {
    let resp = fail_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/x-www-form-urlencoded")
                // No silcrow-target header → plain browser POST
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(location, "/");

    let cookie_header = resp
        .headers()
        .get("set-cookie")
        .expect("set-cookie header must be present")
        .to_str()
        .unwrap();
    assert!(
        cookie_header.contains("silcrow_form_flash="),
        "expected silcrow_form_flash cookie, got: {cookie_header}"
    );
}

// ── req.take_form_flash() ────────────────────────────────────

#[tokio::test]
async fn req_take_form_flash_reads_and_clears_cookie() {
    use runtime::form_errors;

    // Encode a FormErrors value the same way req.fail() does.
    let errors = form_errors().error("name", "Required").value("name", "alice");
    let json = serde_json::to_string(&errors).unwrap();
    let encoded = urlencoding::encode(&json).into_owned();
    let cookie_header = format!("silcrow_form_flash={encoded}");

    let app = Router::new().route(
        "/",
        get(|req: Req| async move {
            match req.take_form_flash() {
                Some(fe) => fe.errors.get("name").cloned().unwrap_or_default(),
                None => "no flash".to_owned(),
            }
        }),
    );

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("cookie", cookie_header)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_string(resp).await, "Required");
}

#[tokio::test]
async fn req_take_form_flash_returns_none_without_cookie() {
    let app = Router::new().route(
        "/",
        get(|req: Req| async move {
            req.take_form_flash()
                .map(|_| "some")
                .unwrap_or("none")
                .to_owned()
        }),
    );

    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(body_string(resp).await, "none");
}
