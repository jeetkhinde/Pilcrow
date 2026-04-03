use thiserror::Error;

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
