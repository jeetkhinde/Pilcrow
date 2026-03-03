use axum::{
    extract::{Extension, Path},
    response::{IntoResponse, Response},
    Form,
};
use pilcrow::*;

use crate::models::{Db, UpdateProfile, User};
use crate::templates::{layout, render_profile};

// ─── GET /profile ──────────────────────────────────────────────────

pub async fn view_profile(req: SilcrowRequest) -> Result<Response, ErrorResponse> {
    let profile = crate::models::Profile {
        id: 1,
        name: "Jagjeet".into(),
    };
    respond!(req, {
        html => html(layout(render_profile(&profile))),
        json => json(&profile),
    })
}

// ─── POST /profile ─────────────────────────────────────────────────

pub async fn update_profile(
    req: SilcrowRequest,
    Extension(user): Extension<User>,
    Form(payload): Form<UpdateProfile>,
) -> Result<Response, Response> {
    // 1. Auth guard
    if !user.can_edit() {
        return Ok(navigate("/login")
            .with_toast("Unauthorized", "error")
            .into_response());
    }

    // 2. Compute
    let updated = Db::update_profile(user.id, payload).await?;
    let header_patch = serde_json::json!({"name": updated.name});

    // 3. Respond
    respond!(req, {
        html => html(render_profile(&updated).into_string())
            .patch_target("#header", &header_patch)
            .invalidate_target("#sidebar"),
        json => json(&updated),
        toast => ("Saved!", "success"),
    })
}

// ─── Task Manager ──────────────────────────────────────────────────

pub async fn list_tasks(
    req: SilcrowRequest,
    Extension(state): Extension<crate::models::AppState>,
) -> Result<Response, ErrorResponse> {
    let tasks = state.tasks.lock().unwrap().clone();
    respond!(req, {
        html => html(crate::templates::layout(crate::templates::render_task_dashboard(&tasks))),
        json => json(&tasks),
    })
}

pub async fn create_task(
    req: SilcrowRequest,
    Extension(state): Extension<crate::models::AppState>,
    Form(payload): Form<crate::models::CreateTask>,
) -> Result<Response, Response> {
    if payload.title.trim().is_empty() {
        return Ok(navigate("/tasks")
            .with_toast("Title cannot be empty", "error")
            .into_response());
    }

    let mut next_id = state.next_id.lock().unwrap();
    let task = crate::models::Task {
        id: *next_id,
        title: payload.title.clone(),
        completed: false,
    };
    *next_id += 1;

    state.tasks.lock().unwrap().push(task.clone());

    let tasks = state.tasks.lock().unwrap().clone();

    respond!(req, {
        html => html(crate::templates::render_task_list(&tasks).into_string())
            .trigger_event("task:created")
            .with_header("silcrow-trigger", r#"{"toast": {"msg": "Task created!", "level": "success"}, "task:created": {}}"#)
            .retarget("#task-list"),
        json => json(&task),
    })
}

pub async fn toggle_task(
    req: SilcrowRequest,
    Extension(state): Extension<crate::models::AppState>,
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
        html => html(crate::templates::render_task_list(&cloned_tasks).into_string())
            .with_header("silcrow-trigger", r#"{"toast": {"msg": "Task toggled.", "level": "success"}}"#)
            .retarget("#task-list"),
        json => json(&updated_task),
    })
}

pub async fn delete_task(
    req: SilcrowRequest,
    Extension(state): Extension<crate::models::AppState>,
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
        html => html(crate::templates::render_task_list(&cloned_tasks).into_string())
            .with_header("silcrow-trigger", r#"{"toast": {"msg": "Task deleted.", "level": "info"}}"#)
            .retarget("#task-list"),
        json => json(&serde_json::json!({"deleted": true})),
    })
}
