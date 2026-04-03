use tonic::transport::Endpoint;
use tonic_health::pb::{HealthCheckRequest, health_client::HealthClient};

use crate::GrpcClientError;

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
