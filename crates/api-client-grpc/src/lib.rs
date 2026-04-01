use async_trait::async_trait;
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

#[async_trait]
pub trait TodosGrpcApi: Send + Sync {
    async fn list_todos(&self) -> Result<Vec<TodoDto>, GrpcClientError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnimplementedTodosGrpcClient;

#[async_trait]
impl TodosGrpcApi for UnimplementedTodosGrpcClient {
    async fn list_todos(&self) -> Result<Vec<TodoDto>, GrpcClientError> {
        Err(GrpcClientError::Unavailable)
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
