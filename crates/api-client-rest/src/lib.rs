use std::future::Future;
use std::pin::Pin;

use pilcrow_contracts::{CreateTodoRequest, ListTodosResponse, TodoDto};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RestClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}

pub trait TodosApi: Send + Sync {
    fn list_todos(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, RestClientError>> + Send + '_>>;
    fn create_todo(
        &self,
        title: String,
    ) -> Pin<Box<dyn Future<Output = Result<TodoDto, RestClientError>> + Send + '_>>;
}

#[derive(Clone)]
pub struct RestTodosClient {
    base_url: String,
    client: reqwest::Client,
}

impl RestTodosClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

impl TodosApi for RestTodosClient {
    fn list_todos(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, RestClientError>> + Send + '_>> {
        Box::pin(async {
            let response = self
                .client
                .get(format!("{}/api/todos", self.base_url))
                .send()
                .await?
                .error_for_status()?
                .json::<ListTodosResponse>()
                .await?;
            Ok(response.items)
        })
    }

    fn create_todo(
        &self,
        title: String,
    ) -> Pin<Box<dyn Future<Output = Result<TodoDto, RestClientError>> + Send + '_>> {
        Box::pin(async move {
            let response = self
                .client
                .post(format!("{}/api/todos", self.base_url))
                .json(&CreateTodoRequest { title })
                .send()
                .await?
                .error_for_status()?
                .json::<TodoDto>()
                .await?;
            Ok(response)
        })
    }
}
