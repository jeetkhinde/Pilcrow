use axum::response::Response;
use serde::{Deserialize, Serialize};

// ─── User ──────────────────────────────────────────────────────────

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

// ─── Profile ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateProfile {
    pub name: String,
}

// ─── Mock Database ─────────────────────────────────────────────────

pub struct Db;

impl Db {
    pub async fn update_profile(id: i64, payload: UpdateProfile) -> Result<Profile, Response> {
        Ok(Profile {
            id,
            name: payload.name,
        })
    }
}

// ─── Task Manager ──────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

#[derive(Deserialize)]
pub struct CreateTask {
    pub title: String,
}

#[derive(Clone)]
pub struct AppState {
    pub tasks: std::sync::Arc<std::sync::Mutex<Vec<Task>>>,
    pub next_id: std::sync::Arc<std::sync::Mutex<i64>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tasks: std::sync::Arc::new(std::sync::Mutex::new(vec![
                Task {
                    id: 1,
                    title: "Learn Axum".into(),
                    completed: true,
                },
                Task {
                    id: 2,
                    title: "Build Pilcrow".into(),
                    completed: false,
                },
            ])),
            next_id: std::sync::Arc::new(std::sync::Mutex::new(3)),
        }
    }
}
