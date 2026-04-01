use std::net::SocketAddr;

use tonic::transport::Server;

pub async fn serve(addr: SocketAddr) -> Result<(), tonic::transport::Error> {
    let (_reporter, health_service) = tonic_health::server::health_reporter();

    Server::builder()
        .add_service(health_service)
        .serve(addr)
        .await
}
