use axum::{extract::Extension, response::{IntoResponse, Response}, routing::get, Form, Router};
use pilcrow::*;
use serde::{Deserialize, Serialize};

// ─── 1. Mock Models & State ────────────────────────────────────────

#[derive(Clone)]
pub struct User {
    pub id: i64,
    pub role: String,
}

impl User {
    pub fn can_edit(&self) -> bool {
        self.role == "admin"
    }
}

#[derive(Deserialize)]
pub struct UpdateProfile {
    pub name: String,
}

#[derive(Serialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
}

struct Db;
impl Db {
    // Pure compute mock
    async fn update_profile(id: i64, payload: UpdateProfile) -> Result<Profile, Response> {
        Ok(Profile { id, name: payload.name })
    }
}

// ─── 2. Pure Render Functions ──────────────────────────────────────

fn render_profile(profile: &Profile) -> String {
    format!(
        r#"
        <div id="header" s-bind="name">Current Name: {name}</div>
        <div id="sidebar">Sidebar loaded for ID: {id}</div>
        <hr/>
        <form s-action="/profile" method="POST">
            <input type="text" name="name" value="{name}" />
            <button type="submit">Update Profile</button>
        </form>
        "#,
        name = profile.name,
        id = profile.id
    )
}

fn layout(content: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head>{}</head><body s-debug>{}</body></html>",
        pilcrow::assets::script_tag(), // Injects <script src="/_silcrow/silcrow...js">
        content
    )
}

// ─── 3. Pilcrow Handlers ───────────────────────────────────────────

async fn view_profile(req: SilcrowRequest) -> Result<Response, Response> {
    let profile = Profile { id: 1, name: "Jagjeet".into() };
    respond!(req, {
        html => html(layout(&render_profile(&profile))),
        json => json(&profile),
    })
}

pub async fn update_profile(
    req: SilcrowRequest,
    Extension(user): Extension<User>,
    Form(payload): Form<UpdateProfile>,
) -> Result<Response, Response> {
    // 1. Pure Auth Guard
    if !user.can_edit() {
        return Ok(navigate("/login")
            .with_toast("Unauthorized", "error")
            .into_response());
    }

    // 2. Pure Compute
    let updated = Db::update_profile(user.id, payload).await?;
    let header_patch = serde_json::json!({"name": updated.name});

    // 3. Declarative Pipeline
    respond!(req, {
        html => html(render_profile(&updated))
            .patch_target("#header", &header_patch) 
            .invalidate_target("#sidebar"),         
        json => json(&updated),
        toast => ("Saved!", "success"),             
    })
}

// ─── 4. Axum Boundary ──────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let mock_user = User { id: 1, role: "admin".into() };

    let app = Router::new()
        .route(&pilcrow::assets::silcrow_js_path(), get(pilcrow::assets::serve_silcrow_js))
        .route("/profile", get(view_profile).post(update_profile))
        .layer(axum::Extension(mock_user));

    println!("Listening on http://127.0.0.1:3000/profile");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}