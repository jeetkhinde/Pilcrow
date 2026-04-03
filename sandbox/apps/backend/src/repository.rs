use std::sync::Arc;

use tokio::sync::RwLock;

use crate::models::Todo;

#[derive(Clone, Default)]
pub struct TodoRepository {
    inner: Arc<RwLock<Vec<Todo>>>,
}

impl TodoRepository {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn list(&self) -> Vec<Todo> {
        self.inner.read().await.clone()
    }

    pub async fn create(&self, title: String) -> Todo {
        let mut guard = self.inner.write().await;
        let id = guard.len() as i64 + 1;
        let todo = Todo {
            id,
            title,
            done: false,
        };
        guard.push(todo.clone());
        todo
    }
}
