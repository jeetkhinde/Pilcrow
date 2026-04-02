use std::future::Future;
use std::pin::Pin;

use pilcrow_contracts::TodoDto;
use thiserror::Error;
use tonic::transport::Endpoint;
use tonic_health::pb::{HealthCheckRequest, health_client::HealthClient};

#[derive(Debug, Error)]
pub enum GrpcClientError {
    #[error("grpc transport unavailable in current build")]
    Unavailable,
    #[error("grpc transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("grpc uri error: {0}")]
    InvalidUri(#[from] tonic::codegen::http::uri::InvalidUri),
    #[error("grpc status error: {0}")]
    Status(#[from] tonic::Status),
}

pub trait TodosGrpcApi: Send + Sync {
    fn list_todos(&self) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, GrpcClientError>> + Send + '_>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnimplementedTodosGrpcClient;

impl TodosGrpcApi for UnimplementedTodosGrpcClient {
    fn list_todos(&self) -> Pin<Box<dyn Future<Output = Result<Vec<TodoDto>, GrpcClientError>> + Send + '_>> {
        Box::pin(async { Err(GrpcClientError::Unavailable) })
    }
}

pub async fn check_health(endpoint: impl AsRef<str>) -> Result<bool, GrpcClientError> {
    let channel = Endpoint::from_shared(endpoint.as_ref().to_string())?
        .connect()
        .await?;
    let mut client = HealthClient::new(channel);
    let response = client
        .check(HealthCheckRequest {
            service: String::new(),
        })
        .await?;

    let status = response.into_inner().status;
    Ok(status == tonic_health::ServingStatus::Serving as i32)
}
