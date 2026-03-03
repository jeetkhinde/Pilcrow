use axum::response::Response;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateProfile {
    pub name: String,
}

pub struct Db;

impl Db {
    pub async fn update_profile(id: i64, payload: UpdateProfile) -> Result<Profile, Response> {
        Ok(Profile {
            id,
            name: payload.name,
        })
    }
}
