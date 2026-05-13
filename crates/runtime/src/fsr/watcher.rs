use super::baking::inject_fsr_slots;
use super::store::{FsrStore, StaleSlot};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

/// Configuration for the embedded FSR watcher.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// How often to poll for stale rows.
    pub poll_interval_ms: u64,
    /// Framework default promote_after_hits (per-field can override via pilcrow_fsr).
    pub promote_after_hits: u32,
    /// Framework default patch_debounce_secs.
    pub patch_debounce_secs: u32,
    /// Seconds before a route's baked artefacts are purged.
    pub purge_after_seconds: u64,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 500,
            promote_after_hits: 100,
            patch_debounce_secs: 30,
            purge_after_seconds: 2_592_000,
        }
    }
}

/// A channel sender for pushing FSR slot-patch events to the SSE hub.
///
/// Type alias — the channel is created by the SSE hub (Phase 8); the watcher
/// receives a sender to call after patching. If None, pushes are skipped.
pub type WatcherEventTx = tokio::sync::broadcast::Sender<SlotPatch>;

/// A single slot-patch event pushed by the watcher.
#[derive(Debug, Clone)]
pub struct SlotPatch {
    pub route: String,
    pub slot: String,
    pub value: serde_json::Value,
}

/// Tick function — usable both in embedded mode (Tokio task) and external mode (caller-driven).
///
/// - Fetches stale rows from `pilcrow_fsr`
/// - Re-executes stored queries via sqlx
/// - Patches baked HTML/JSON files on disk (promoted routes)
/// - Broadcasts `SlotPatch` events to connected SSE clients
/// - Marks rows fresh
pub async fn watcher_tick(
    store: &FsrStore,
    event_tx: Option<&WatcherEventTx>,
) -> Result<(), sqlx::Error> {
    let stale = store.fetch_stale_slots().await?;

    for slot_row in stale {
        // Re-execute the stored query.
        let value = match re_execute_query(store, &slot_row).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    route = %slot_row.route,
                    slot = %slot_row.slot,
                    error = %e,
                    "FSR watcher: failed to re-execute query"
                );
                continue;
            }
        };

        // Patch baked HTML on disk if html_path is set and promoted.
        if slot_row.promoted {
            if let Some(ref html_path) = slot_row.html_path {
                patch_html_file(html_path, &slot_row.slot, &value).await;
            }
            if let Some(ref json_path) = slot_row.json_path {
                patch_json_file(json_path, &slot_row.slot, &value).await;
            }
        }

        // Push SSE event.
        if let Some(tx) = event_tx {
            let _ = tx.send(SlotPatch {
                route: slot_row.route.clone(),
                slot: slot_row.slot.clone(),
                value: value.clone(),
            });
        }

        // Mark fresh.
        if let Err(e) = store.mark_fresh(&slot_row.route, &slot_row.slot).await {
            tracing::warn!(
                route = %slot_row.route,
                slot = %slot_row.slot,
                error = %e,
                "FSR watcher: failed to mark slot fresh"
            );
        }
    }

    Ok(())
}

/// Re-execute the stored query for a stale slot, returning the new value.
async fn re_execute_query(store: &FsrStore, slot: &StaleSlot) -> sqlx::Result<serde_json::Value> {
    let Some(ref sql) = slot.query else {
        return Ok(serde_json::Value::Null);
    };

    // Build a query with positional params from slot.query_params JSON array.
    let params: Vec<serde_json::Value> = slot
        .query_params
        .as_ref()
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    // Execute via raw SQL with positional params.
    // For simplicity, bind all as text; Postgres will cast via implicit coercion.
    let row = execute_with_params(store.pool(), sql, &params).await?;

    // Extract the column matching the slot name.
    let value = row
        .as_ref()
        .and_then(|m| m.get(&slot.slot))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    Ok(value)
}

/// Execute a parameterised query and return the first row as a JSON map.
/// Parameters are bound as text strings (Postgres will coerce to the column type).
async fn execute_with_params(
    pool: &sqlx::PgPool,
    sql: &str,
    params: &[serde_json::Value],
) -> sqlx::Result<Option<serde_json::Map<String, serde_json::Value>>> {
    // Wrap in row_to_json so we get a typed map back regardless of column names.
    let json_sql = format!("SELECT row_to_json(t) as __row FROM ({sql}) t");

    let mut q = sqlx::query_scalar::<_, serde_json::Value>(&json_sql);
    for p in params {
        let s = match p {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        q = q.bind(s);
    }

    let result = q.fetch_optional(pool).await?;
    Ok(result.and_then(|v| v.as_object().cloned()))
}

/// Patch an `s-live` slot in a baked HTML file on disk.
async fn patch_html_file(html_path: &str, slot: &str, value: &serde_json::Value) {
    match tokio::fs::read_to_string(html_path).await {
        Ok(html) => {
            let slots = vec![(slot.to_string(), value.clone())];
            let patched = inject_fsr_slots(&html, &slots);
            if let Err(e) = tokio::fs::write(html_path, patched).await {
                tracing::warn!(
                    path = html_path,
                    error = %e,
                    "FSR watcher: failed to write patched HTML"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                path = html_path,
                error = %e,
                "FSR watcher: failed to read HTML for patching"
            );
        }
    }
}

/// Patch a JSON field in a baked JSON file on disk.
async fn patch_json_file(json_path: &str, slot: &str, value: &serde_json::Value) {
    match tokio::fs::read_to_string(json_path).await {
        Ok(content) => {
            let mut obj: serde_json::Value =
                serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            if let serde_json::Value::Object(ref mut map) = obj {
                map.insert(slot.to_string(), value.clone());
            }
            match serde_json::to_string(&obj) {
                Ok(json) => {
                    if let Err(e) = tokio::fs::write(json_path, json).await {
                        tracing::warn!(
                            path = json_path,
                            error = %e,
                            "FSR watcher: failed to write patched JSON"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        path = json_path,
                        error = %e,
                        "FSR watcher: failed to serialise patched JSON"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                path = json_path,
                error = %e,
                "FSR watcher: failed to read JSON for patching"
            );
        }
    }
}

/// Start the embedded watcher as a background Tokio task.
///
/// The task runs indefinitely until the process exits. Call this once from
/// `pilcrow_start()` when FSR is enabled.
pub fn spawn_embedded_watcher(
    store: Arc<FsrStore>,
    config: WatcherConfig,
    event_tx: Option<WatcherEventTx>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = Duration::from_millis(config.poll_interval_ms);
        let mut ticker = time::interval(interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            if let Err(e) = watcher_tick(&store, event_tx.as_ref()).await {
                tracing::error!(error = %e, "FSR watcher tick failed");
            }
        }
    })
}

/// Execute a single watcher tick from an external process.
///
/// Use this when `watcher = "external"` in `[fsr]` config.
pub async fn pilcrow_fsr_watcher_tick(pool: sqlx::PgPool) -> Result<(), sqlx::Error> {
    let store = FsrStore::new(pool);
    watcher_tick(&store, None).await
}
