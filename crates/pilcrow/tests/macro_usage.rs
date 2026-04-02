// ./crates/pilcrow/tests/macro_usage.rs
//
// Compile-time verification of all respond! macro arms.
// respond! is JSON-only — no request parameter, no content negotiation.

use axum::http::StatusCode;
use pilcrow::{Response, ToastLevel, json, respond, response::ResponseExt};

// ════════════════════════════════════════════════════════════
// Pre-wrapped JSON
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_plain() {
    let response: Result<Response, Response> = respond!(
        json => json(serde_json::json!({"ok": true})),
    );
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn json_with_status() {
    let response: Result<Response, Response> = respond!(
        json => json(serde_json::json!({"id": 1})),
        status => StatusCode::CREATED,
    );
    assert_eq!(response.unwrap().status(), StatusCode::CREATED);
}

#[tokio::test]
async fn json_with_toast() {
    let response: Result<Response, Response> = respond!(
        json => json(serde_json::json!({"ok": true})),
        toast => ("Saved!", ToastLevel::Success),
    );
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn json_with_status_and_toast() {
    let response: Result<Response, Response> = respond!(
        json => json(serde_json::json!({"id": 1})),
        status => StatusCode::CREATED,
        toast => ("Created!", ToastLevel::Success),
    );
    assert_eq!(response.unwrap().status(), StatusCode::CREATED);
}

// ════════════════════════════════════════════════════════════
// Raw JSON (auto-wraps in json())
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn raw_json_plain() {
    let response: Result<Response, Response> = respond!(
        json => raw serde_json::json!({"bare": true}),
    );
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn raw_json_with_status() {
    let response: Result<Response, Response> = respond!(
        json => raw serde_json::json!({"id": 1}),
        status => StatusCode::CREATED,
    );
    assert_eq!(response.unwrap().status(), StatusCode::CREATED);
}

#[tokio::test]
async fn raw_json_with_toast() {
    let response: Result<Response, Response> = respond!(
        json => raw serde_json::json!({"ok": true}),
        toast => ("Done", ToastLevel::Info),
    );
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn raw_json_with_status_and_toast() {
    let response: Result<Response, Response> = respond!(
        json => raw serde_json::json!({"id": 1}),
        status => StatusCode::CREATED,
        toast => ("Created!", ToastLevel::Success),
    );
    assert_eq!(response.unwrap().status(), StatusCode::CREATED);
}

// ════════════════════════════════════════════════════════════
// Inline modifiers (chaining on pre-wrapped json)
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_with_chained_modifiers() {
    let response: Result<Response, Response> = respond!(
        json => json(serde_json::json!({"ok": true}))
            .with_toast("Done", ToastLevel::Info)
            .no_cache(),
    );
    let res = response.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
