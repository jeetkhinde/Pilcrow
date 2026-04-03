use std::future::Future;
use std::pin::Pin;

use crate::RestClientError;
use pilcrow_contracts::TodoDto;

pub trait TodosApi: Send + Sync {
    fn list_todos(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, RestClientError>> + Send + '_>>;
    fn create_todo(
        &self,
        title: String,
    ) -> Pin<Box<dyn Future<Output = Result<TodoDto, RestClientError>> + Send + '_>>;
}
