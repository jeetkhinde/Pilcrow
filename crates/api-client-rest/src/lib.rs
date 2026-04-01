use async_trait::async_trait;
use pilcrow_contracts::{CreateTodoRequest, ListTodosResponse, TodoDto};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RestClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}

#[async_trait]
pub trait TodosApi: Send + Sync {
    async fn list_todos(&self) -> Result<Vec<TodoDto>, RestClientError>;
    async fn create_todo(&self, title: String) -> Result<TodoDto, RestClientError>;
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

#[async_trait]
impl TodosApi for RestTodosClient {
    async fn list_todos(&self) -> Result<Vec<TodoDto>, RestClientError> {
        let response = self
            .client
            .get(format!("{}/api/todos", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<ListTodosResponse>()
            .await?;
        Ok(response.items)
    }

    async fn create_todo(&self, title: String) -> Result<TodoDto, RestClientError> {
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
    }
}
