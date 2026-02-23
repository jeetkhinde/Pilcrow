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
    // ── Both arms + shared toast ─────────────────────────────
    ($req:expr, { html => $html:expr, json => raw $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        $req.select(
            $crate::Responses::new()
                .html(move || async move { $crate::ResponseExt::with_toast($html, $msg, $lvl) })
                .json(move || async move {
                    $crate::ResponseExt::with_toast($crate::json($json), $msg, $lvl)
                }),
        )
        .await
    };
    ($req:expr, { html => $html:expr, json => $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        $req.select(
            $crate::Responses::new()
                .html(move || async move { $crate::ResponseExt::with_toast($html, $msg, $lvl) })
                .json(move || async move { $crate::ResponseExt::with_toast($json, $msg, $lvl) }),
        )
        .await
    };

    // ── Both arms, no shared toast ───────────────────────────
    ($req:expr, { html => $html:expr, json => raw $json:expr $(,)? }) => {
        $req.select(
            $crate::Responses::new()
                .html(move || async move { $html })
                .json(move || async move { $crate::json($json) }),
        )
        .await
    };
    ($req:expr, { html => $html:expr, json => $json:expr $(,)? }) => {
        $req.select(
            $crate::Responses::new()
                .html(move || async move { $html })
                .json(move || async move { $json }),
        )
        .await
    };

    // ── HTML-only + shared toast ─────────────────────────────
    ($req:expr, { html => $html:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        $req.select(
            $crate::Responses::new()
                .html(move || async move { $crate::ResponseExt::with_toast($html, $msg, $lvl) }),
        )
        .await
    };

    // ── JSON-only + shared toast ─────────────────────────────
    ($req:expr, { json => raw $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        $req.select($crate::Responses::new().json(move || async move {
            $crate::ResponseExt::with_toast($crate::json($json), $msg, $lvl)
        }))
        .await
    };
    ($req:expr, { json => $json:expr, toast => ($msg:expr, $lvl:expr) $(,)? }) => {
        $req.select(
            $crate::Responses::new()
                .json(move || async move { $crate::ResponseExt::with_toast($json, $msg, $lvl) }),
        )
        .await
    };

    // ── HTML-only, no toast ──────────────────────────────────
    ($req:expr, { html => $html:expr $(,)? }) => {
        $req.select($crate::Responses::new().html(move || async move { $html }))
            .await
    };

    // ── JSON-only, no toast ──────────────────────────────────
    ($req:expr, { json => raw $json:expr $(,)? }) => {
        $req.select($crate::Responses::new().json(move || async move { $crate::json($json) }))
            .await
    };
    ($req:expr, { json => $json:expr $(,)? }) => {
        $req.select($crate::Responses::new().json(move || async move { $json }))
            .await
    };
}
