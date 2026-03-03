use axum::{
    extract::{Extension, Path},
    response::{IntoResponse, Response},
    Form,
};
use pilcrow::*;

use super::models::{AppState, CreateTask};
use super::templates::{render_task_dashboard, render_task_list};

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
) -> Result<Response, Response> {
    if payload.title.trim().is_empty() {
        return Ok(navigate("/examples/tasks")
            .with_toast("Title cannot be empty", "error")
            .into_response());
    }

    let mut next_id = state.next_id.lock().unwrap();
    let task = super::models::Task {
        id: *next_id,
        title: payload.title.clone(),
        completed: false,
    };
    *next_id += 1;

    state.tasks.lock().unwrap().push(task.clone());

    let tasks = state.tasks.lock().unwrap().clone();

    respond!(req, {
        html => html(render_task_list(&tasks).into_string())
            .trigger_event("task:created")
            .with_header(
                "silcrow-trigger",
                r#"{"toast": {"msg": "Task created!", "level": "success"}, "task:created": {}}"#,
            )
            .retarget("#task-list"),
        json => json(&task),
    })
}

pub async fn toggle_task(
    req: SilcrowRequest,
    Extension(state): Extension<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ErrorResponse> {
    let mut tasks = state.tasks.lock().unwrap();
    let mut modified = false;
    let mut updated_task = None;
    for t in tasks.iter_mut() {
        if t.id == id {
            t.completed = !t.completed;
            modified = true;
            updated_task = Some(t.clone());
            break;
        }
    }

    if !modified {
        return Err((axum::http::StatusCode::NOT_FOUND, "Task not found").into_response());
    }

    let cloned_tasks = tasks.clone();
    let updated_task = updated_task.unwrap();

    respond!(req, {
        html => html(render_task_list(&cloned_tasks).into_string())
            .with_header(
                "silcrow-trigger",
                r#"{"toast": {"msg": "Task toggled.", "level": "success"}}"#,
            )
            .retarget("#task-list"),
        json => json(&updated_task),
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
        return Err((axum::http::StatusCode::NOT_FOUND, "Task not found").into_response());
    }

    let cloned_tasks = tasks.clone();

    respond!(req, {
        html => html(render_task_list(&cloned_tasks).into_string())
            .with_header(
                "silcrow-trigger",
                r#"{"toast": {"msg": "Task deleted.", "level": "info"}}"#,
            )
            .retarget("#task-list"),
        json => json(&serde_json::json!({"deleted": true})),
    })
}
