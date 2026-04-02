// ./crates/pilcrow/src/macros.rs

/// Convenience macro for JSON API responses.
///
/// Used in `api/` directory handlers. Templates handle HTML rendering.
/// No request parameter needed — these endpoints always return JSON.
///
/// # Usage
///
/// ```ignore
/// // Pre-wrapped
/// respond!(json => json(&user))
///
/// // Raw (auto-wraps in json())
/// respond!(json => raw &user)
///
/// // With toast
/// respond!(json => json(&user), toast => ("Saved!", ToastLevel::Success))
///
/// // With status
/// respond!(json => json(&user), status => StatusCode::CREATED)
///
/// // All modifiers
/// respond!(json => raw &user, status => StatusCode::CREATED, toast => ("Created!", ToastLevel::Success))
/// ```
#[macro_export]
macro_rules! respond {

    // ── Raw (auto-wraps in json()) ───────────────────────────

    // status + toast
    (json => raw $json:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response(
                $crate::ResponseExt::with_status(
                    $crate::ResponseExt::with_toast($crate::json($json), $msg, $lvl),
                    $code,
                )
            )
        )
    };
    // status only
    (json => raw $json:expr, status => $code:expr $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response(
                $crate::ResponseExt::with_status($crate::json($json), $code)
            )
        )
    };
    // toast only
    (json => raw $json:expr, toast => ($msg:expr, $lvl:expr) $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response(
                $crate::ResponseExt::with_toast($crate::json($json), $msg, $lvl)
            )
        )
    };
    // plain
    (json => raw $json:expr $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response($crate::json($json))
        )
    };

    // ── Pre-wrapped ──────────────────────────────────────────

    // status + toast
    (json => $json:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response(
                $crate::ResponseExt::with_status(
                    $crate::ResponseExt::with_toast($json, $msg, $lvl),
                    $code,
                )
            )
        )
    };
    // status only
    (json => $json:expr, status => $code:expr $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response(
                $crate::ResponseExt::with_status($json, $code)
            )
        )
    };
    // toast only
    (json => $json:expr, toast => ($msg:expr, $lvl:expr) $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response(
                $crate::ResponseExt::with_toast($json, $msg, $lvl)
            )
        )
    };
    // plain
    (json => $json:expr $(,)?) => {
        Ok::<_, $crate::Response>(
            axum::response::IntoResponse::into_response($json)
        )
    };
}
