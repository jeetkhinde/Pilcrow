// ./crates/pilcrow/tests/macro_usage.rs
//
// Compile-time verification of all respond! macro arms.
// These tests ensure the macro expands correctly and type-checks
// against the real Responses builder.

use axum::{http::StatusCode, response::Response};
use pilcrow::{html, json, respond, response::ResponseExt, SilcrowRequest};

// ── Helper: simulate a browser HTML request ──────────────────
fn html_request() -> SilcrowRequest {
    SilcrowRequest {
        is_silcrow: false,
        accepts_html: true,
        accepts_json: false,
    }
}

fn json_request() -> SilcrowRequest {
    SilcrowRequest {
        is_silcrow: true,
        accepts_html: false,
        accepts_json: true,
    }
}

// ════════════════════════════════════════════════════════════
// Both arms
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn both_arms_html_selected() {
    let req = html_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
        json => json(serde_json::json!({"ok": true})),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn both_arms_json_selected() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
        json => json(serde_json::json!({"ok": true})),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

// ════════════════════════════════════════════════════════════
// Raw JSON
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn raw_json_auto_wraps() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
        json => raw serde_json::json!({"auto": true}),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

// ════════════════════════════════════════════════════════════
// Shared toast
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn shared_toast_applied_to_html() {
    let req = html_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
        json => json(serde_json::json!({"ok": true})),
        toast => ("Saved!", "success"),
    });
    let res = response.unwrap();
    let cookies: Vec<_> = res
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert!(cookies.iter().any(|c| c.starts_with("silcrow_toasts=")));
}

#[tokio::test]
async fn shared_toast_with_raw_json() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
        json => raw serde_json::json!({"ok": true}),
        toast => ("Saved!", "success"),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

// ════════════════════════════════════════════════════════════
// Single-arm variants
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn html_only_returns_406_for_json_request() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
    });
    assert_eq!(response.unwrap().status(), StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn json_only_returns_406_for_html_request() {
    let req = html_request();
    let response: Result<Response, Response> = respond!(req, {
        json => json(serde_json::json!({"ok": true})),
    });
    assert_eq!(response.unwrap().status(), StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn html_only_with_toast() {
    let req = html_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>"),
        toast => ("Done", "info"),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn json_only_with_toast() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        json => json(serde_json::json!({"ok": true})),
        toast => ("Done", "info"),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn raw_json_only_with_toast() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        json => raw serde_json::json!({"ok": true}),
        toast => ("Done", "info"),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

// ════════════════════════════════════════════════════════════
// Inline modifiers (per-arm toast, chaining)
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn per_arm_modifiers_chain() {
    let req = html_request();
    let response: Result<Response, Response> = respond!(req, {
        html => html("<h1>Hello</h1>")
            .with_toast("HTML toast", "info")
            .no_cache()
            .retarget("#main"),
        json => json(serde_json::json!({"ok": true}))
            .with_toast("JSON toast", "success"),
    });
    let res = response.unwrap();
    assert_eq!(res.headers()["silcrow-retarget"], "#main");
    assert_eq!(res.headers()["silcrow-cache"], "no-cache");
}

// ════════════════════════════════════════════════════════════
// The get_profile example from our discussion
// ════════════════════════════════════════════════════════════

#[tokio::test]
async fn get_profile_example() {
    // Simulating the handler pattern
    let req = html_request();
    let user_name = "Jagjeet";
    let user_bio = "Rust architect";

    let markup = format!(
        r#"<div class="profile"><h1>{}</h1><p>{}</p></div>"#,
        user_name, user_bio
    );

    let response: Result<Response, Response> = respond!(req, {
        html => html(markup).with_toast("Loaded", "info"),
        json => json(serde_json::json!({
            "name": user_name,
            "bio": user_bio
        })),
    });

    assert_eq!(response.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn raw_json_only_no_toast() {
    let req = json_request();
    let response: Result<Response, Response> = respond!(req, {
        json => raw serde_json::json!({"bare": true}),
    });
    assert_eq!(response.unwrap().status(), StatusCode::OK);
}
