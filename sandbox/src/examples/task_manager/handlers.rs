use axum::{
    extract::{Extension, Path},
    response::{IntoResponse, Response},
    Form,
};
use pilcrow::*;

use super::models::{AppState, CreateTask, Task};
use super::templates::render_task_dashboard;

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
    Form(payload): Form<CreateTask>,
) -> Result<Response, ErrorResponse> {
    if payload.title.trim().is_empty() {
        return Ok(navigate("/examples/tasks")
            .with_toast("Title cannot be empty", "error")
            .into_response());
    }

    let mut next_id = state.next_id.lock().unwrap();
    let task = Task {
        id: *next_id,
        title: payload.title.clone(),
        completed: true,
    };
    *next_id += 1;

    state.tasks.lock().unwrap().push(task.clone());
    respond!(req, {
        json => json(serde_json::json!({ "tasks": task })).with_header("silcrow-trigger", "task:created"),
    })
}

pub async fn toggle_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let mut tasks = state.tasks.lock().unwrap();
    let mut modified = false;
    for t in tasks.iter_mut() {
        if t.id == id {
            t.completed = !t.completed;
            modified = true;
            break;
        }
    }

    if !modified {
        return Err((StatusCode::NOT_FOUND, "Task not found").into_response());
    }

    let cloned_tasks = tasks.clone();

    respond!(req, {
        json => json(&serde_json::json!({"tasks": cloned_tasks})),
        toast => ("Task toggled.", "success"),
    })
}

pub async fn delete_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let mut tasks = state.tasks.lock().unwrap();
    let len_before = tasks.len();
    tasks.retain(|t| t.id != id);

    if tasks.len() == len_before {
        return Err((StatusCode::NOT_FOUND, "Task not found").into_response());
    }

    let cloned_tasks = tasks.clone();

    respond!(req, {
        json => json(&serde_json::json!({"tasks": cloned_tasks})),
        toast => ("Task deleted.", "info"),
    })
}
