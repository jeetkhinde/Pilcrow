// ./crates/pilcrow/src/macros.rs

/// Ergonomic macro for content negotiation responses.
///
/// Supports shared toasts applied to whichever branch runs.
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
/// Internal: wrap an expression in Ok(IntoResponse::into_response(...))
#[doc(hidden)]
#[macro_export]
macro_rules! __respond_ok {
    ($expr:expr) => {
        Ok::<_, axum::response::Response>(axum::response::IntoResponse::into_response($expr))
    };
}

/// Internal: apply toast if provided, otherwise pass through
#[doc(hidden)]
#[macro_export]
macro_rules! __respond_with_toast {
    ($expr:expr, ($msg:expr, $lvl:expr)) => {
        $crate::ResponseExt::with_toast($expr, $msg, $lvl)
    };
    ($expr:expr,) => {
        $expr
    };
}

/// Internal: 406 response for unsupported content type
#[doc(hidden)]
#[macro_export]
macro_rules! __respond_406 {
    ($label:expr) => {
        $crate::__respond_ok!((axum::http::StatusCode::NOT_ACCEPTABLE, $label))
    };
}

#[macro_export]
macro_rules! respond {
    // ── Both arms (html + json) ──────────────────────────────
    // With raw JSON
    ($req:expr, { html => $html:expr, json => raw $json:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_toast!($html, $( ($msg, $lvl) )?))
            }
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_toast!($crate::json($json), $( ($msg, $lvl) )?))
            }
        }
    };
    // With pre-wrapped JSON
    ($req:expr, { html => $html:expr, json => $json:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_toast!($html, $( ($msg, $lvl) )?))
            }
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_toast!($json, $( ($msg, $lvl) )?))
            }
        }
    };

    // ── HTML-only ────────────────────────────────────────────
    ($req:expr, { html => $html:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_toast!($html, $( ($msg, $lvl) )?))
            }
            _ => $crate::__respond_406!("HTML required"),
        }
    };

    // ── JSON-only (raw) ──────────────────────────────────────
    ($req:expr, { json => raw $json:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_toast!($crate::json($json), $( ($msg, $lvl) )?))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };

    // ── JSON-only (pre-wrapped) ──────────────────────────────
    ($req:expr, { json => $json:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_toast!($json, $( ($msg, $lvl) )?))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };
}
