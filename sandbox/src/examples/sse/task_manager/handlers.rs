use axum::{
    Form,
    extract::{Extension, Path},
    response::{IntoResponse, Response},
};
use pilcrow::*;

use super::model::{AppState, AppStateSse, CreateTask, Task, TaskListEvent, TaskStats};
use super::templates::render_task_dashboard;
use tokio_stream::StreamExt;

pub async fn list_tasks(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
) -> Result<Response, ErrorResponse> {
    let tasks = state.tasks.lock().unwrap().clone();
    respond!(req, {
        html => html(crate::templates::layout(render_task_dashboard(&tasks))),
        json => json(&tasks),
    })
}

pub async fn create_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Extension(sse_state): Extension<AppStateSse>,
    Form(payload): Form<CreateTask>,
) -> Result<Response, ErrorResponse> {
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Ok(navigate("/examples/sse/tasks")
            .with_toast("Title cannot be empty", "error")
            .into_response());
    }
    let task = {
        let mut next_id = state.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;

        Task {
            id,
            title,
            completed: false,
        }
    };

    let stats_data = {
        let mut tasks = state.tasks.lock().unwrap();
        tasks.push(task.clone());
        tasks.clone()
    };
    let stats = TaskStats::from(&stats_data);
    let _ = sse_state.tx.send(stats.clone());
    let _ = sse_state.list_tx.send(TaskListEvent::Added(task.clone()));
    respond!(req, {
        json => Ok::<_, ErrorResponse>(axum::http::StatusCode::NO_CONTENT.into_response()),
    })
}

pub async fn toggle_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Extension(sse_state): Extension<AppStateSse>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let mut tasks = state.tasks.lock().unwrap();

    let task = tasks.iter_mut().find(|t| t.id == id);

    let Some(task) = task else {
        return Err((StatusCode::NOT_FOUND, "Task not found").into_response());
    };

    task.completed = !task.completed;
    let toggle_event = TaskListEvent::Toggled {
        id: task.id,
        completed: task.completed,
    };
    let stats = TaskStats::from(&tasks);
    let _ = sse_state.tx.send(stats.clone());
    let _ = sse_state.list_tx.send(toggle_event);

    drop(tasks); // release the lock before responding

    respond!(req, {
        json => Ok::<_, ErrorResponse>(axum::http::StatusCode::NO_CONTENT.into_response()),
    })
}

pub async fn delete_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Extension(sse_state): Extension<AppStateSse>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let mut tasks = state.tasks.lock().unwrap();
    let len_before = tasks.len();
    tasks.retain(|t| t.id != id);

    if tasks.len() == len_before {
        return Err((StatusCode::NOT_FOUND, "Task not found").into_response());
    }

    let stats = TaskStats::from(&tasks);
    let _ = sse_state.tx.send(stats.clone());
    let _ = sse_state
        .list_tx
        .send(TaskListEvent::Removed { id, _remove: true });
    drop(tasks);

    respond!(req, {
        json => Ok::<_, ErrorResponse>(axum::http::StatusCode::NO_CONTENT.into_response()),
    })
}

// SSE version events
use async_stream::stream;
use pilcrow::{PilcrowStreamExt, sse_stream};
use std::convert::Infallible;
use tokio::sync::broadcast::error::RecvError::{Closed, Lagged};

use tokio_stream::wrappers::BroadcastStream;

pub async fn task_events(
    Extension(state): Extension<AppState>,
    Extension(sse_state): Extension<AppStateSse>,
) -> impl IntoResponse {
    let mut rx = sse_state.tx.subscribe();
    let stream = stream! {
        let current_stats = {
            let tasks = state.tasks.lock().unwrap();
            TaskStats::from(&tasks)
        };
        yield Ok::<_, Infallible>(
            SilcrowEvent::patch(&current_stats, "#live-stats").into()
        );
        loop {
            match rx.recv().await {
                Ok(stats) => {
                    yield Ok::<_, Infallible>(
                        SilcrowEvent::patch(&stats, "#live-stats").into()
                    );
                }
                Err(Lagged(_)) => {
                }
                Err(Closed) => {
                    break;
                }
            }
        }
    };
    pilcrow::sse_raw(stream)
}

pub async fn task_list_events(
    Extension(state): Extension<AppState>,
    Extension(sse_state): Extension<AppStateSse>,
) -> impl IntoResponse {
    let mut rx = sse_state.list_tx.subscribe();
    let stream = stream! {
        // On connect, send the full current task list so the client is in sync.
        let current_tasks = {
            let tasks = state.tasks.lock().unwrap();
            tasks.clone()
        };
        yield Ok::<_, Infallible>(
            SilcrowEvent::patch(
                serde_json::json!({ "tasks": current_tasks }),
                "#task-list",
            ).into()
        );
        // Then relay individual mutation events as they arrive.
        while let Ok(event) = rx.recv().await {
            let payload = serde_json::json!({ "tasks": event });
            yield Ok::<_, Infallible>(
                SilcrowEvent::patch(payload, "#task-list").into()
            );
        }
    };
    pilcrow::sse_raw(stream)
}

#[allow(dead_code)]
pub async fn sse_handler(Extension(state): Extension<AppStateSse>) -> impl IntoResponse {
    let stats_rx = BroadcastStream::new(state.tx.subscribe()).filter_map(|r| r.ok()); // ignore lagged errors

    let list_rx = BroadcastStream::new(state.list_tx.subscribe()).filter_map(|r| r.ok());

    sse_stream(|emit| async move {
        pilcrow::combine!(
            stats_rx.json("#live-stats", &emit),
            list_rx.json("#task-list", &emit),
        )
        .await
    })
}
