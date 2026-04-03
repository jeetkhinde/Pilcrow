use thiserror::Error;

#[derive(Debug, Error)]
pub enum RestClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}
