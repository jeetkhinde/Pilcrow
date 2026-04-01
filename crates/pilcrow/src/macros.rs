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
/// Internal: apply status code if provided, otherwise pass through
#[doc(hidden)]
#[macro_export]
macro_rules! __respond_with_status {
    ($expr:expr, $code:expr) => {
        $crate::ResponseExt::with_status($expr, $code)
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

    // ════════════════════════════════════════════════════════
    // Both arms — raw JSON
    // ════════════════════════════════════════════════════════

    // status + toast
    ($req:expr, { html => $html:expr, json => raw $json:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($html, ($msg, $lvl)), $code))
            }
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($crate::json($json), ($msg, $lvl)), $code))
            }
        }
    };
    // status only
    ($req:expr, { html => $html:expr, json => raw $json:expr, status => $code:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_status!($html, $code))
            }
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::json($json), $code))
            }
        }
    };
    // toast only + neither (existing)
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

    // ════════════════════════════════════════════════════════
    // Both arms — pre-wrapped JSON
    // ════════════════════════════════════════════════════════

    // status + toast
    ($req:expr, { html => $html:expr, json => $json:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($html, ($msg, $lvl)), $code))
            }
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($json, ($msg, $lvl)), $code))
            }
        }
    };
    // status only
    ($req:expr, { html => $html:expr, json => $json:expr, status => $code:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_status!($html, $code))
            }
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($json, $code))
            }
        }
    };
    // toast only + neither (existing)
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

    // ════════════════════════════════════════════════════════
    // HTML-only
    // ════════════════════════════════════════════════════════

    // status + toast
    ($req:expr, { html => $html:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($html, ($msg, $lvl)), $code))
            }
            _ => $crate::__respond_406!("HTML required"),
        }
    };
    // status only
    ($req:expr, { html => $html:expr, status => $code:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_status!($html, $code))
            }
            _ => $crate::__respond_406!("HTML required"),
        }
    };
    // toast only + neither (existing)
    ($req:expr, { html => $html:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Html => {
                $crate::__respond_ok!($crate::__respond_with_toast!($html, $( ($msg, $lvl) )?))
            }
            _ => $crate::__respond_406!("HTML required"),
        }
    };

    // ════════════════════════════════════════════════════════
    // JSON-only — raw
    // ════════════════════════════════════════════════════════

    // status + toast
    ($req:expr, { json => raw $json:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($crate::json($json), ($msg, $lvl)), $code))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };
    // status only
    ($req:expr, { json => raw $json:expr, status => $code:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::json($json), $code))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };
    // toast only + neither (existing)
    ($req:expr, { json => raw $json:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_toast!($crate::json($json), $( ($msg, $lvl) )?))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };

    // ════════════════════════════════════════════════════════
    // JSON-only — pre-wrapped
    // ════════════════════════════════════════════════════════

    // status + toast
    ($req:expr, { json => $json:expr, status => $code:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($crate::__respond_with_toast!($json, ($msg, $lvl)), $code))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };
    // status only
    ($req:expr, { json => $json:expr, status => $code:expr $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_status!($json, $code))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };
    // toast only + neither (existing)
    ($req:expr, { json => $json:expr $(, toast => ($msg:expr, $lvl:expr))? $(,)? }) => {
        match $req.preferred_mode() {
            $crate::extract::RequestMode::Json => {
                $crate::__respond_ok!($crate::__respond_with_toast!($json, $( ($msg, $lvl) )?))
            }
            _ => $crate::__respond_406!("JSON required"),
        }
    };
}
