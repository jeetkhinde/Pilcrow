use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
}