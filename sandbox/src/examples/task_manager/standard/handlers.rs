use axum::{
    Form,
    extract::{Extension, Path},
    response::{IntoResponse, Response},
};
use pilcrow::*;

use super::model::{AppState, CreateTask, Task, TaskStats};
use crate::examples::task_manager::sse::model::AppStateSse;
use crate::examples::task_manager::templates::render_task_dashboard;

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
    if payload.title.trim().is_empty() {
        return Ok(navigate("/examples/tasks")
            .with_toast("Title cannot be empty", "error")
            .into_response());
    }

    let task = {
        let mut next_id = state.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;

        Task {
            id,
            title: payload.title,
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
    respond!(req, {
        json => json(serde_json::json!({ "tasks": task }))
            .patch_target("#stats", &stats)
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
    let payload = serde_json::json!({ "tasks": { "id": task.id, "completed": task.completed } });
    let stats = TaskStats::from(&tasks);
    let _ = sse_state.tx.send(stats.clone());

    drop(tasks); // release the lock before responding

    respond!(req, {
        json => json(&payload)
            .patch_target("#stats", &stats),
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
    drop(tasks);

    respond!(req, {
        json => json(&serde_json::json!({ "tasks": { "id": id, "_remove": true } }))
                        .patch_target("#stats", &stats),
    })
}
