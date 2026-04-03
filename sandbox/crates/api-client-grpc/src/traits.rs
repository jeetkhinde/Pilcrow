use std::future::Future;
use std::pin::Pin;

use crate::GrpcClientError;
use pilcrow_contracts::TodoDto;

pub trait TodosGrpcApi: Send + Sync {
    fn list_todos(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, GrpcClientError>> + Send + '_>>;
}
