// ./crates/pilcrow/src/macros.rs

/// Ergonomic macro for content negotiation responses.
///
/// Expands to `req.select(Responses::new()...).await` with automatic
/// closure wrapping. Supports shared toasts applied to whichever branch runs.
///
/// # Usage
///
/// ```ignore
/// // Both arms
/// pilcrow::respond!(req, {
///     html => html(markup).with_toast("Loaded", "info"),
///     json => json(user),
/// })
///
/// // Auto-wrap JSON (raw)
/// pilcrow::respond!(req, {
///     html => html(markup),
///     json => raw user,
/// })
///
/// // Shared toast
/// pilcrow::respond!(req, {
///     html => html(markup),
///     json => json(user),
///     toast => ("Saved!", "success"),
/// })
///
/// // HTML-only (JSON returns 406)
/// pilcrow::respond!(req, {
///     html => html(markup),
/// })
///
/// // JSON-only (HTML returns 406)
/// pilcrow::respond!(req, {
///     json => json(user),
/// })
/// ```
#[macro_export]
macro_rules! respond {
    ($req:expr, { html => $html:expr, json => raw $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($html, $msg, $lvl),
                ))
            }
            $crate::extract::RequestMode::Json => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($crate::json($json), $msg, $lvl),
                ))
            }
        }
    };
    ($req:expr, { html => $html:expr, json => $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($html, $msg, $lvl),
                ))
            }
            $crate::extract::RequestMode::Json => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($json, $msg, $lvl),
                ))
            }
        }
    };

    // ── Both arms, no shared toast ───────────────────────────
    ($req:expr, { html => $html:expr, json => raw $json:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($html),
            ),
            $crate::extract::RequestMode::Json => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($crate::json($json)),
            ),
        }
    };
    ($req:expr, { html => $html:expr, json => $json:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($html),
            ),
            $crate::extract::RequestMode::Json => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($json),
            ),
        }
    };

    // ── HTML-only + shared toast ─────────────────────────────
    ($req:expr, { html => $html:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($html, $msg, $lvl),
                ))
            }
            _ => Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response((
                axum::http::StatusCode::NOT_ACCEPTABLE,
                "HTML required",
            ))),
        }
    };

    // ── JSON-only + shared toast ─────────────────────────────
    ($req:expr, { json => raw $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($crate::json($json), $msg, $lvl),
                ))
            }
            _ => Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response((
                axum::http::StatusCode::NOT_ACCEPTABLE,
                "JSON required",
            ))),
        }
    };
    ($req:expr, { json => $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response(
                    $crate::ResponseExt::with_toast($json, $msg, $lvl),
                ))
            }
            _ => Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response((
                axum::http::StatusCode::NOT_ACCEPTABLE,
                "JSON required",
            ))),
        }
    };

    // ── HTML-only, no toast ──────────────────────────────────
    ($req:expr, { html => $html:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($html),
            ),
            _ => Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response((
                axum::http::StatusCode::NOT_ACCEPTABLE,
                "HTML required",
            ))),
        }
    };

    // ── JSON-only, no toast ──────────────────────────────────
    ($req:expr, { json => raw $json:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($crate::json($json)),
            ),
            _ => Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response((
                axum::http::StatusCode::NOT_ACCEPTABLE,
                "JSON required",
            ))),
        }
    };
    ($req:expr, { json => $json:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => Ok::<_, axum::response::Response>(
                axum::response::IntoResponse::into_response($json),
            ),
            _ => Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response((
                axum::http::StatusCode::NOT_ACCEPTABLE,
                "JSON required",
            ))),
        }
    };
}
