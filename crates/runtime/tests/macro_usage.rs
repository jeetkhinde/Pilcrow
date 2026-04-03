// ./crates/pilcrow/tests/macro_usage.rs
//
// Verifies json() + ResponseExt chaining produces correct responses.
// No macros needed — json() returns JsonResponse<T> which implements
// both ResponseExt (for chaining) and IntoResponse (for Axum).

use axum::http::StatusCode;
use axum::response::IntoResponse;
use runtime::{ToastLevel, json, response::ResponseExt};

// ════════════════════════════════════════════════════════════
// Plain JSON
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_plain() {
    let response = json(serde_json::json!({"ok": true})).into_response();
    assert_eq!(response.status(), StatusCode::OK);
}

// ════════════════════════════════════════════════════════════
// With status
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_with_status() {
    let response = json(serde_json::json!({"id": 1}))
        .with_status(StatusCode::CREATED)
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
}

// ════════════════════════════════════════════════════════════
// With toast
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_with_toast() {
    let response = json(serde_json::json!({"ok": true}))
        .with_toast("Saved!", ToastLevel::Success)
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
}

// ════════════════════════════════════════════════════════════
// With status + toast
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_with_status_and_toast() {
    let response = json(serde_json::json!({"id": 1}))
        .with_status(StatusCode::CREATED)
        .with_toast("Created!", ToastLevel::Success)
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
}

// ════════════════════════════════════════════════════════════
// Chained modifiers
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn json_with_chained_modifiers() {
    let response = json(serde_json::json!({"ok": true}))
        .with_toast("Done", ToastLevel::Info)
        .no_cache()
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn json_with_all_modifiers() {
    let response = json(serde_json::json!({"id": 42}))
        .with_status(StatusCode::CREATED)
        .with_toast("Created!", ToastLevel::Success)
        .no_cache()
        .with_header("x-custom", "value")
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response
            .headers()
            .get("x-custom")
            .unwrap()
            .to_str()
            .unwrap(),
        "value"
    );
}
