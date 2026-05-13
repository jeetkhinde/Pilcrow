use super::PilcrowLive;
use axum::http::request::Parts;
use pilcrow_core::AppError;
use serde_json::Value;
use std::collections::HashMap;

/// Runtime helper called from the generated `FromRequestParts` impl for each `Live` type.
///
/// Extracts the sqlx pool from extensions, extracts route params,
/// executes the `Live::query()`, and calls `Live::from_row()`.
///
/// For now this returns a default-populated `Live` struct; full DB wiring is done in
/// Phase 7 (watcher) when the pool is wired into extensions.
pub async fn extract_live_from_parts<T: PilcrowLive>(
    _parts: &mut Parts,
) -> Result<T, AppError> {
    // Stub: return a row-less Live struct.  Full implementation in Phase 7.
    let row: HashMap<String, Value> = HashMap::new();
    Ok(T::from_row(&row))
}
