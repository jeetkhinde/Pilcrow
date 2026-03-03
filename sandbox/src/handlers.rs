use axum::{
    extract::Extension,
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
