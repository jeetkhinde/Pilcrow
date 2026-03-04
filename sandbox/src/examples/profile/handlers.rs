use axum::{
    extract::Extension,
    response::{IntoResponse, Response},
    Form,
};
use pilcrow::*;

use super::models::{Db, Profile, UpdateProfile, User};
use super::templates::render_profile;
use crate::templates::layout;

pub async fn view_profile(req: SilcrowRequest) -> Result<Response, ErrorResponse> {
    let profile = Profile {
        id: 1,
        name: "Jagjeet".into(),
    };
    respond!(req, {
        html => html(layout(render_profile(&profile))),
        json => json(&profile),
    })
}

pub async fn update_profile(
    req: SilcrowRequest,
    Extension(user): Extension<User>,
    Form(payload): Form<UpdateProfile>,
) -> Result<Response, Response> {
    if !user.can_edit() {
        return Ok(navigate("/login")
            .with_toast("Unauthorized", "error")
            .into_response());
    }

    let updated = Db::update_profile(user.id, payload).await?;
    let header_patch = serde_json::json!({"name": updated.name});

    respond!(req, {
        html => html(render_profile(&updated).into_string())
            .patch_target("#header", &header_patch)
            .invalidate_target("#sidebar"),
        json => json(&updated),
        toast => ("Saved!", "success"),
    })
}
