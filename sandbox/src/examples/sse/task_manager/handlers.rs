use axum::{Form, extract::Extension, extract::Path, response::IntoResponse};
use pilcrow::*;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;

use super::EVENTS;
use super::model::{AppState, CreateTask, Task};
use super::templates::render_dashboard;
use crate::templates::layout;
// ── HTTP handlers ─────────────────────────────────────────────

pub async fn list_tasks(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
) -> Result<Response, ErrorResponse> {
    let tasks = state.tasks.lock().unwrap().clone();
    respond!(req, {
        html => html(layout(render_dashboard(&tasks, EVENTS.path()))).sse(EVENTS),
        json => json(&tasks),
    })
}

pub async fn create_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Form(form): Form<CreateTask>,
) -> Result<Response, ErrorResponse> {
    let title = form.title.trim().to_owned();
    if title.is_empty() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Title cannot be empty").into_response());
    }

    // Allocate ID before mutate() to avoid nesting next_id lock inside tasks lock.
    let id = {
        let mut next_id = state.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        id
    };

    state.mutate(|tasks| {
        tasks.push(Task {
            id,
            title,
            completed: false,
        });
    });

    respond!(req, {
        json => json(serde_json::json!({ "title": "" })),
        status => StatusCode::CREATED,
    })
}

fn toggle_task_in_list(tasks: &mut Vec<Task>, id: i64) -> Option<bool> {
    tasks.iter().position(|t| t.id == id).map(|idx| {
        let completed = !tasks[idx].completed;
        tasks[idx] = Task {
            completed,
            ..tasks[idx].clone()
        };
        completed
    })
}

pub async fn toggle_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let new_completed = state
        .mutate(|tasks| toggle_task_in_list(tasks, id))
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Task not found").into_response())?;

    respond!(req, {
        json => json(serde_json::json!({ "task": { "id": id, "completed": new_completed } })),
        status => StatusCode::OK,
    })
}
pub async fn delete_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let mut deleted = false;

    state.mutate(|tasks| {
        let before = tasks.len();
        tasks.retain(|t| t.id != id);
        deleted = tasks.len() < before;
    });

    if !deleted {
        return Err((StatusCode::NOT_FOUND, "Task not found").into_response());
    }

    respond!(req, {
        json => json(serde_json::json!({ "tasks": { "id": id, "_remove": true } })).with_status(StatusCode::NO_CONTENT),
    })
}

// ── SSE stream handler ────────────────────────────────────────

/// Single unified SSE endpoint.
///
/// Both `#live-stats` and `#task-list` point to this same URL via `s-live`.
/// The Silcrow SSE hub multiplexes them over a single `EventSource` — regardless
/// of how many DOM elements share the URL, the browser opens exactly one connection.
///
/// Pattern:
///   1. Seed: client gets current state immediately on connect. No blank panels.
///   2. `combine!` drives two independent streams concurrently:
///      - `watch`     → stats     (always delivers latest, never misses an update)
///      - `broadcast` → task list (full snapshot after every mutation, drops Lagged)
///   3. `with_id` sets `Last-Event-ID` — reconnecting clients resume from a known
///      point rather than re-requesting full state from the server.
///   4. Either stream returning `Err` disconnects the client cleanly via `?`.
pub async fn events(Extension(state): Extension<AppState>) -> impl IntoResponse {
    // Capture seeds before the async boundary.
    let seed_stats = state.stats_rx.borrow().clone();
    let seed_tasks = state.tasks.lock().unwrap().clone();

    // Subscribe to channels before entering the closure. This closes the race
    // between seed delivery and the first mutation: any event fired between
    // subscribe() and the seed send will be re-delivered by the watch stream.
    let stats_stream = pilcrow::watch(state.stats_rx.clone());
    let list_stream =
        BroadcastStream::new(state.list_tx.subscribe()).filter_map(|result| result.ok()); // Lagged: drop stale snapshots, client has latest via seed

    sse_stream(|emit| async move {
        // ── Seed: immediate state on connect ─────────────────────
        emit.send(SilcrowEvent::patch(&seed_stats, "#live-stats").with_id("seed-stats"))
            .await?;

        emit.send(
            SilcrowEvent::patch(serde_json::json!({ "tasks": seed_tasks }), "#task-list")
                .with_id("seed-list"),
        )
        .await?;

        // ── Live updates: two streams, one connection, two targets ──
        //
        // Other SilcrowEvent variants available for richer flows:
        //
        //   emit.send(SilcrowEvent::invalidate("#sidebar")).await?;
        //   emit.send(SilcrowEvent::navigate("/dashboard")).await?;
        //   emit.send(SilcrowEvent::custom("task:milestone", &json!({"count": 10}))).await?;
        pilcrow::combine!(
            stats_stream.json("#live-stats", &emit),
            list_stream.json("#task-list", &emit),
        )
        .await
    })
}
