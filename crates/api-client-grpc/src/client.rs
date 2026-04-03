use std::future::Future;
use std::pin::Pin;

use crate::GrpcClientError;
use crate::TodosGrpcApi;
use pilcrow_contracts::TodoDto;

#[derive(Debug, Default, Clone, Copy)]
pub struct UnimplementedTodosGrpcClient;

impl TodosGrpcApi for UnimplementedTodosGrpcClient {
    fn list_todos(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, GrpcClientError>> + Send + '_>> {
        Box::pin(async { Err(GrpcClientError::Unavailable) })
    }
}
