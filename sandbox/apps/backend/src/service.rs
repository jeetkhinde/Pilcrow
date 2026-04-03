use pilcrow_core::AppResult;

use crate::{api::contracts::TodoDto, repository::TodoRepository};

#[derive(Clone)]
pub struct TodoService {
    repo: TodoRepository,
}

impl TodoService {
    pub fn new(repo: TodoRepository) -> Self {
        Self { repo }
    }

    pub async fn list_todos(&self) -> AppResult<Vec<TodoDto>> {
        let todos = self
            .repo
            .list()
            .await
            .into_iter()
            .map(|todo| TodoDto {
                id: todo.id,
                title: todo.title,
                done: todo.done,
            })
            .collect::<Vec<_>>();
        Ok(todos)
    }

    pub async fn create_todo(&self, title: String) -> AppResult<TodoDto> {
        let todo = self.repo.create(title).await;
        Ok(TodoDto {
            id: todo.id,
            title: todo.title,
            done: todo.done,
        })
    }
}
